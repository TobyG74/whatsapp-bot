use super::{Admission, JobSummary, Peer};
use crate::{app::config::Settings, infrastructure::security::Crypto};
use anyhow::{Context, Result, ensure};
use chrono::Utc;
use mongodb::{
    Client, Collection, IndexModel,
    bson::{Document, doc},
    error::{ErrorKind, WriteFailure},
    options::{ClientOptions, ReturnDocument},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

pub(super) struct MongoDatabase {
    database: mongodb::Database,
    crypto: Arc<Crypto>,
    admission: Mutex<()>,
}

fn duplicate_key(error: &mongodb::error::Error) -> bool {
    matches!(error.kind.as_ref(), ErrorKind::Write(WriteFailure::WriteError(write)) if write.code == 11000)
}

impl MongoDatabase {
    pub async fn connect(url: &str, crypto: Arc<Crypto>) -> Result<Self> {
        let mut options = ClientOptions::parse(url)
            .await
            .context("Invalid MongoDB connection URL")?;
        options.server_selection_timeout = Some(Duration::from_secs(10));
        options.connect_timeout = Some(Duration::from_secs(10));
        let client = Client::with_options(options)?;
        let database = client
            .default_database()
            .context("MongoDB URL must include a database name")?;
        database
            .run_command(doc! { "ping": 1 })
            .await
            .context("MongoDB connection failed")?;
        let db = Self {
            database,
            crypto,
            admission: Mutex::new(()),
        };
        for (collection, keys) in [
            ("peers", doc! { "kind": 1 }),
            ("seen_messages", doc! { "seen_at": 1 }),
            ("command_usage", doc! { "day": 1 }),
            (
                "outbox",
                doc! { "status": 1, "send_at": 1, "created_at": 1, "_id": 1 },
            ),
            ("outbox", doc! { "created_at": -1, "_id": -1 }),
        ] {
            db.collection(collection)
                .create_index(IndexModel::builder().keys(keys).build())
                .await?;
        }
        // A crash after transmission cannot safely be retried without a delivery receipt.
        db.collection("outbox").update_many(
            doc! { "status": "sending" },
            doc! { "$set": { "status": "failed", "failure_code": "delivery_unknown_after_restart", "payload": "" } },
        ).await?;
        Ok(db)
    }

    fn collection(&self, name: &str) -> Collection<Document> {
        self.database.collection(name)
    }

    fn encode<T: Serialize>(&self, context: &str, value: &T) -> Result<String> {
        let payload = self.crypto.seal(context, &serde_json::to_vec(value)?)?;
        ensure!(
            payload.len() <= 15 * 1024 * 1024,
            "Encrypted payload exceeds MongoDB document budget"
        );
        Ok(payload)
    }

    fn decode<T: DeserializeOwned>(&self, context: &str, value: &str) -> Result<T> {
        Ok(serde_json::from_slice(&self.crypto.open(context, value)?)?)
    }

    pub async fn settings(&self, defaults: &Settings) -> Result<Settings> {
        let collection = self.collection("bot_settings");
        let result = collection
            .update_one(
                doc! { "_id": 1 },
                doc! { "$setOnInsert": { "payload": self.encode("settings", defaults)? } },
            )
            .upsert(true)
            .await;
        if let Err(error) = result
            && !duplicate_key(&error)
        {
            return Err(error.into());
        }
        let row = collection
            .find_one(doc! { "_id": 1 })
            .await?
            .context("Settings missing")?;
        let settings: Settings = self.decode("settings", row.get_str("payload")?)?;
        settings.validate()?;
        Ok(settings)
    }

    pub async fn save_settings(&self, settings: &Settings) -> Result<()> {
        settings.validate()?;
        self.collection("bot_settings")
            .update_one(
                doc! { "_id": 1 },
                doc! { "$set": { "payload": self.encode("settings", settings)? } },
            )
            .upsert(true)
            .await?;
        Ok(())
    }

    pub async fn remember_peer(&self, kind: &str, jid: &str, name: &str) -> Result<()> {
        ensure!(matches!(kind, "contact" | "group"), "Invalid peer kind");
        let id = self.crypto.index(kind, jid);
        let peer = Peer {
            jid: jid.to_owned(),
            name: name.chars().take(128).collect(),
        };
        self.collection("peers").update_one(
            doc! { "_id": &id },
            doc! { "$set": { "kind": kind, "payload": self.encode(&id, &peer)?, "updated_at": Utc::now().timestamp() } },
        ).upsert(true).await?;
        Ok(())
    }

    pub async fn user_language(&self, actor: &str) -> Result<Option<String>> {
        let id = self.crypto.index("user_preferences", actor);
        let row = self
            .collection("user_preferences")
            .find_one(doc! { "_id": &id })
            .await?;
        row.map(|row| self.decode(&id, row.get_str("payload")?))
            .transpose()
    }

    pub async fn save_user_language(&self, actor: &str, language: &str) -> Result<()> {
        let id = self.crypto.index("user_preferences", actor);
        self.collection("user_preferences")
            .update_one(
                doc! { "_id": &id },
                doc! { "$set": { "payload": self.encode(&id, &language)? } },
            )
            .upsert(true)
            .await?;
        Ok(())
    }

    pub async fn peer_counts(&self) -> Result<(i64, i64)> {
        let collection = self.collection("peers");
        let (contacts, groups) = tokio::try_join!(
            async { collection.count_documents(doc! { "kind": "contact" }).await },
            async { collection.count_documents(doc! { "kind": "group" }).await },
        )?;
        Ok((contacts.try_into()?, groups.try_into()?))
    }

    pub async fn first_message(&self, chat: &str, sender: &str, message_id: &str) -> Result<bool> {
        let id = self
            .crypto
            .index("message", &format!("{chat}\0{sender}\0{message_id}"));
        match self
            .collection("seen_messages")
            .update_one(
                doc! { "_id": id },
                doc! { "$setOnInsert": { "seen_at": Utc::now().timestamp() } },
            )
            .upsert(true)
            .await
        {
            Ok(result) => Ok(result.upserted_id.is_some()),
            Err(error) if duplicate_key(&error) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn admit(
        &self,
        actor: &str,
        command: &str,
        cooldown: u64,
        cost: u32,
        limit: u32,
        now: i64,
    ) -> Result<Admission> {
        let _guard = self.admission.lock().await;
        let day = chrono::DateTime::from_timestamp(now, 0)
            .context("Invalid timestamp")?
            .format("%Y-%m-%d")
            .to_string();
        let id = format!("{}:{day}", self.crypto.index("actor", actor));
        let last_key = format!("last.{}", self.crypto.index("command", command));
        let cooldown = i64::try_from(cooldown).context("Cooldown is too large")?;
        let before = now
            .checked_sub(cooldown)
            .context("Invalid cooldown timestamp")?;
        let collection = self.collection("command_usage");
        let result = collection
            .update_one(
                doc! { "_id": &id },
                doc! { "$setOnInsert": { "day": day, "used": 0_i64, "last": {} } },
            )
            .upsert(true)
            .await;
        if let Err(error) = result
            && !duplicate_key(&error)
        {
            return Err(error.into());
        }
        let accepted = collection
            .find_one_and_update(
                doc! {
                    "_id": &id,
                    "used": { "$lte": if cost == 0 { i64::MAX } else { i64::from(limit) - i64::from(cost) } },
                    "$or": [
                        { &last_key: { "$lte": before } },
                        { &last_key: { "$exists": false } },
                    ],
                },
                doc! { "$inc": { "used": i64::from(cost) }, "$set": { &last_key: now } },
            )
            .return_document(ReturnDocument::After)
            .await?;
        if accepted.is_some() {
            return Ok(Admission::Accepted);
        }
        let row = collection
            .find_one(doc! { "_id": id })
            .await?
            .context("Usage record missing")?;
        if let Ok(last) = row
            .get_document("last")?
            .get_i64(last_key.trim_start_matches("last."))
        {
            let remaining = last.saturating_add(cooldown).saturating_sub(now);
            if remaining > 0 {
                return Ok(Admission::Cooldown(remaining as u64));
            }
        }
        Ok(Admission::Limit)
    }

    pub async fn enqueue<T: Serialize>(&self, value: &T, send_at: i64) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.collection("outbox")
            .insert_one(doc! {
                "_id": &id, "payload": self.encode(&id, value)?, "send_at": send_at,
                "status": "pending", "failure_code": "", "created_at": Utc::now().timestamp(),
            })
            .await?;
        Ok(id)
    }

    pub async fn claim_due<T: DeserializeOwned>(&self) -> Result<Option<(String, T)>> {
        let row = self
            .collection("outbox")
            .find_one_and_update(
                doc! { "status": "pending", "send_at": { "$lte": Utc::now().timestamp() } },
                doc! { "$set": { "status": "sending" } },
            )
            .sort(doc! { "send_at": 1, "created_at": 1, "_id": 1 })
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let id = row.get_str("_id")?.to_owned();
        match self.decode(&id, row.get_str("payload")?) {
            Ok(value) => Ok(Some((id, value))),
            Err(error) => {
                self.finish(&id, false, "decryption_failed").await?;
                Err(error)
            }
        }
    }

    pub async fn finish(&self, id: &str, success: bool, code: &str) -> Result<()> {
        self.collection("outbox").update_one(
            doc! { "_id": id },
            doc! { "$set": { "status": if success { "sent" } else { "failed" }, "failure_code": code, "payload": "" } },
        ).await?;
        Ok(())
    }

    pub async fn cancel(&self, id: &str) -> Result<bool> {
        Ok(self
            .collection("outbox")
            .update_one(
                doc! { "_id": id, "status": "pending" },
                doc! { "$set": { "status": "cancelled", "payload": "" } },
            )
            .await?
            .modified_count
            == 1)
    }

    pub async fn jobs(&self) -> Result<Vec<JobSummary>> {
        let mut cursor = self
            .collection("outbox")
            .find(doc! {})
            .sort(doc! { "created_at": -1, "_id": -1 })
            .limit(100)
            .await?;
        let mut jobs = Vec::new();
        while cursor.advance().await? {
            let row = cursor.deserialize_current()?;
            jobs.push(JobSummary {
                id: row.get_str("_id")?.to_owned(),
                send_at: row.get_i64("send_at")?,
                status: row.get_str("status")?.to_owned(),
                failure_code: row.get_str("failure_code")?.to_owned(),
            });
        }
        Ok(jobs)
    }

    pub async fn maintenance(&self) -> Result<()> {
        let cutoff = Utc::now().timestamp() - 7 * 86400;
        self.collection("seen_messages")
            .delete_many(doc! { "seen_at": { "$lt": cutoff } })
            .await?;
        self.collection("command_usage").delete_many(doc! { "day": { "$lt": (Utc::now() - chrono::Duration::days(2)).format("%Y-%m-%d").to_string() } }).await?;
        self.collection("outbox").delete_many(doc! { "status": { "$in": ["sent", "failed", "cancelled"] }, "created_at": { "$lt": cutoff } }).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Backend, Database};

    #[tokio::test]
    #[ignore = "Requires MONGODB_TEST_URL with a database name starting with bot_test"]
    async fn mongodb_storage_contract_and_concurrency() {
        let url = std::env::var("MONGODB_TEST_URL")
            .expect("Set MONGODB_TEST_URL to a dedicated test database");
        let options = ClientOptions::parse(&url).await.unwrap();
        let name = options.default_database.unwrap();
        assert!(
            name.starts_with("bot_test"),
            "Use a dedicated bot_test database"
        );
        let isolated = format!("bot_test_{}", uuid::Uuid::new_v4().simple());
        let url = url.replacen(&format!("/{name}"), &format!("/{isolated}"), 1);
        let crypto = Arc::new(Crypto::new(&"11".repeat(32)).unwrap());
        let db = Database::connect(&url, crypto.clone()).await.unwrap();
        let Backend::Mongo(mongo) = &db.backend else {
            panic!("MongoDB backend expected");
        };
        let mut settings =
            toml::from_str::<crate::app::config::Config>(include_str!("../../config.example.toml"))
                .unwrap()
                .bot;
        db.settings(&settings).await.unwrap();
        settings.prefix = "#".into();
        db.save_settings(&settings).await.unwrap();
        assert_eq!(db.settings(&settings).await.unwrap().prefix, "#");
        db.save_user_language("alice", "en").await.unwrap();
        db.save_user_language("bob", "id").await.unwrap();
        assert_eq!(
            db.user_language("alice").await.unwrap().as_deref(),
            Some("en")
        );
        assert_eq!(
            db.user_language("bob").await.unwrap().as_deref(),
            Some("id")
        );
        assert_eq!(db.user_language("other").await.unwrap(), None);
        assert!(db.save_user_language("alice", "../bad").await.is_err());
        let raw = mongo
            .collection("user_preferences")
            .find_one(doc! { "_id": crypto.index("user_preferences", "alice") })
            .await
            .unwrap()
            .unwrap();
        assert!(!raw.get_str("payload").unwrap().contains("\"en\""));
        db.remember_peer("contact", "123@s.whatsapp.net", "private-contact")
            .await
            .unwrap();
        db.remember_peer("contact", "123@s.whatsapp.net", "private-contact")
            .await
            .unwrap();
        db.remember_peer("group", "123@g.us", "private-group")
            .await
            .unwrap();
        assert_eq!(db.peer_counts().await.unwrap(), (1, 1));
        assert!(db.remember_peer("invalid", "jid", "name").await.is_err());
        let peer = mongo
            .collection("peers")
            .find_one(doc! { "kind": "contact" })
            .await
            .unwrap()
            .unwrap();
        assert!(!peer.to_string().contains("private-contact"));
        assert!(!peer.to_string().contains("123@s.whatsapp.net"));
        let (first, second) = tokio::join!(
            db.first_message("chat", "sender", "message"),
            db.first_message("chat", "sender", "message")
        );
        assert_ne!(first.unwrap(), second.unwrap());

        let now = Utc::now().timestamp();
        assert_eq!(
            db.admit("alice", "sticker", 5, 1, 2, now).await.unwrap(),
            Admission::Accepted
        );
        assert_eq!(
            db.admit("alice", "sticker", 5, 1, 2, now + 1)
                .await
                .unwrap(),
            Admission::Cooldown(4)
        );
        assert_eq!(
            db.admit("alice", "sticker", 5, 1, 2, now + 5)
                .await
                .unwrap(),
            Admission::Accepted
        );
        assert_eq!(
            db.admit("alice", "ping", 2, 1, 2, now + 10).await.unwrap(),
            Admission::Limit
        );
        assert_eq!(
            db.admit("alice", "sticker", 5, 1, 2, now + 86400)
                .await
                .unwrap(),
            Admission::Accepted
        );
        assert_eq!(
            db.admit("owner", "ping", 2, 0, 1, now).await.unwrap(),
            Admission::Accepted
        );
        assert_eq!(
            db.admit("owner", "ping", 2, 0, 1, now + 1).await.unwrap(),
            Admission::Cooldown(1)
        );
        let other = Database::connect(&url, crypto.clone()).await.unwrap();
        let (one, two) = tokio::join!(
            db.admit("parallel", "sticker", 0, 2, 2, now),
            other.admit("parallel", "ping", 0, 2, 2, now)
        );
        let results = [one.unwrap(), two.unwrap()];
        assert!(results.contains(&Admission::Accepted));
        assert!(results.contains(&Admission::Limit));

        let id = db
            .enqueue(&serde_json::json!({ "text": "private-message" }), 0)
            .await
            .unwrap();
        let raw = mongo
            .collection("outbox")
            .find_one(doc! { "_id": &id })
            .await
            .unwrap()
            .unwrap();
        assert!(!raw.get_str("payload").unwrap().contains("private-message"));
        let (one, two) = tokio::join!(
            db.claim_due::<serde_json::Value>(),
            other.claim_due::<serde_json::Value>()
        );
        let claimed: Vec<_> = [one.unwrap(), two.unwrap()].into_iter().flatten().collect();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].0, id);
        assert_eq!(claimed[0].1["text"], "private-message");
        db.finish(&id, true, "").await.unwrap();
        assert!(!db.cancel(&id).await.unwrap());
        let pending = db.enqueue(&42, i64::MAX).await.unwrap();
        assert!(db.cancel(&pending).await.unwrap());
        assert!(!db.cancel(&pending).await.unwrap());

        let interrupted = db.enqueue(&7, 0).await.unwrap();
        db.claim_due::<i32>().await.unwrap().unwrap();
        let reopened = Database::connect(&url, crypto).await.unwrap();
        assert_eq!(
            reopened.user_language("alice").await.unwrap().as_deref(),
            Some("en")
        );
        assert_eq!(
            reopened
                .admit("alice", "setlang", 0, 0, 1, now + 11)
                .await
                .unwrap(),
            Admission::Accepted
        );
        assert_eq!(reopened.settings(&settings).await.unwrap().prefix, "#");
        assert_eq!(
            reopened
                .admit("alice", "ping", 2, 1, 2, now + 10)
                .await
                .unwrap(),
            Admission::Limit
        );
        let job = reopened
            .jobs()
            .await
            .unwrap()
            .into_iter()
            .find(|job| job.id == interrupted)
            .unwrap();
        assert_eq!(job.status, "failed");
        assert_eq!(job.failure_code, "delivery_unknown_after_restart");
        assert!(reopened.claim_due::<i32>().await.unwrap().is_none());
        let raw = mongo
            .collection("outbox")
            .find_one(doc! { "_id": &interrupted })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(raw.get_str("payload").unwrap(), "");

        let corrupted = db.enqueue(&1, 0).await.unwrap();
        mongo
            .collection("outbox")
            .update_one(
                doc! { "_id": &corrupted },
                doc! { "$set": { "payload": "invalid" } },
            )
            .await
            .unwrap();
        assert!(db.claim_due::<i32>().await.is_err());
        assert_eq!(
            db.jobs()
                .await
                .unwrap()
                .into_iter()
                .find(|job| job.id == corrupted)
                .unwrap()
                .failure_code,
            "decryption_failed"
        );
        let media = serde_json::json!({
            "data_base64": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, vec![0_u8; 8 * 1024 * 1024]),
            "text": "maximum-size media",
        });
        let large = db.enqueue(&media, 0).await.unwrap();
        let claimed = db.claim_due::<serde_json::Value>().await.unwrap().unwrap();
        assert_eq!(claimed.0, large);
        assert_eq!(claimed.1, media);
        db.finish(&large, true, "").await.unwrap();
        assert!(db.enqueue(&"x".repeat(12 * 1024 * 1024), 0).await.is_err());
        let ancient = Utc::now().timestamp() - 8 * 86400;
        mongo
            .collection("seen_messages")
            .insert_one(doc! { "_id": "old", "seen_at": ancient })
            .await
            .unwrap();
        mongo
            .collection("command_usage")
            .insert_one(doc! { "_id": "old", "day": "2000-01-01", "used": 0_i64, "last": {} })
            .await
            .unwrap();
        mongo
            .collection("outbox")
            .update_one(
                doc! { "_id": &pending },
                doc! { "$set": { "created_at": ancient } },
            )
            .await
            .unwrap();
        db.maintenance().await.unwrap();
        assert!(
            mongo
                .collection("seen_messages")
                .find_one(doc! { "_id": "old" })
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            mongo
                .collection("command_usage")
                .find_one(doc! { "_id": "old" })
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            mongo
                .collection("outbox")
                .find_one(doc! { "_id": &pending })
                .await
                .unwrap()
                .is_none()
        );
        mongo.database.drop().await.unwrap();
    }
}
