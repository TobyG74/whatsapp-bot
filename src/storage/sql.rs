use super::{Admission, JobSummary, Peer};
use crate::{app::config::Settings, infrastructure::security::Crypto};
use anyhow::{Context, Result, ensure};
use chrono::Utc;
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{AnyPool, Row, any::AnyPoolOptions};
use std::sync::Arc;
use tokio::sync::Mutex;

pub(super) struct SqlDatabase {
    pool: AnyPool,
    crypto: Arc<Crypto>,
    admission: Mutex<()>,
}

impl SqlDatabase {
    pub async fn connect(url: &str, crypto: Arc<Crypto>) -> Result<Self> {
        sqlx::any::install_default_drivers();
        let pool = AnyPoolOptions::new()
            .max_connections(if url.starts_with("sqlite:") { 1 } else { 5 })
            .connect(url)
            .await
            .context("Database connection failed")?;
        for statement in include_str!("../../migrations/001_init.sql").split(';') {
            if !statement.trim().is_empty() {
                sqlx::query(statement).execute(&pool).await?;
            }
        }
        if url.starts_with("sqlite:") {
            sqlx::query("PRAGMA journal_mode = WAL")
                .execute(&pool)
                .await?;
            sqlx::query("PRAGMA busy_timeout = 5000")
                .execute(&pool)
                .await?;
        }
        let db = Self {
            pool,
            crypto,
            admission: Mutex::new(()),
        };
        // A crash after transmission cannot safely be retried without a delivery receipt.
        sqlx::query("UPDATE outbox SET status = 'failed', failure_code = 'delivery_unknown_after_restart', payload = '' WHERE status = 'sending'")
            .execute(&db.pool).await?;
        Ok(db)
    }

    fn encode<T: Serialize>(&self, context: &str, value: &T) -> Result<String> {
        self.crypto.seal(context, &serde_json::to_vec(value)?)
    }
    fn decode<T: DeserializeOwned>(&self, context: &str, value: &str) -> Result<T> {
        Ok(serde_json::from_slice(&self.crypto.open(context, value)?)?)
    }

    pub async fn settings(&self, defaults: &Settings) -> Result<Settings> {
        let payload = self.encode("settings", defaults)?;
        sqlx::query(
            "INSERT INTO bot_settings (id, payload) VALUES (1, $1) ON CONFLICT (id) DO NOTHING",
        )
        .bind(payload)
        .execute(&self.pool)
        .await?;
        let row = sqlx::query("SELECT payload FROM bot_settings WHERE id = 1")
            .fetch_one(&self.pool)
            .await?;
        let settings: Settings = self.decode("settings", row.try_get("payload")?)?;
        settings.validate()?;
        Ok(settings)
    }

    pub async fn save_settings(&self, settings: &Settings) -> Result<()> {
        settings.validate()?;
        sqlx::query("UPDATE bot_settings SET payload = $1 WHERE id = 1")
            .bind(self.encode("settings", settings)?)
            .execute(&self.pool)
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
        sqlx::query("INSERT INTO peers (id, kind, payload, updated_at) VALUES ($1, $2, $3, $4) ON CONFLICT (id) DO UPDATE SET payload = excluded.payload, updated_at = excluded.updated_at")
            .bind(&id).bind(kind).bind(self.encode(&id, &peer)?).bind(Utc::now().timestamp()).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn user_language(&self, actor: &str) -> Result<Option<String>> {
        let id = self.crypto.index("user_preferences", actor);
        let row = sqlx::query("SELECT payload FROM user_preferences WHERE id = $1")
            .bind(&id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| self.decode(&id, row.try_get("payload")?))
            .transpose()
    }

    pub async fn save_user_language(&self, actor: &str, language: &str) -> Result<()> {
        let id = self.crypto.index("user_preferences", actor);
        sqlx::query("INSERT INTO user_preferences (id, payload) VALUES ($1, $2) ON CONFLICT (id) DO UPDATE SET payload = excluded.payload")
            .bind(&id).bind(self.encode(&id, &language)?).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn peer_counts(&self) -> Result<(i64, i64)> {
        let mut contacts = 0;
        let mut groups = 0;
        for row in sqlx::query("SELECT kind, COUNT(*) AS count FROM peers GROUP BY kind")
            .fetch_all(&self.pool)
            .await?
        {
            match row.try_get::<&str, _>("kind")? {
                "contact" => contacts = row.try_get("count")?,
                "group" => groups = row.try_get("count")?,
                _ => (),
            }
        }
        Ok((contacts, groups))
    }

    pub async fn first_message(&self, chat: &str, sender: &str, message_id: &str) -> Result<bool> {
        let id = self
            .crypto
            .index("message", &format!("{chat}\0{sender}\0{message_id}"));
        Ok(sqlx::query(
            "INSERT INTO seen_messages (id, seen_at) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING",
        )
        .bind(id)
        .bind(Utc::now().timestamp())
        .execute(&self.pool)
        .await?
        .rows_affected()
            == 1)
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
        let actor = self.crypto.index("actor", actor);
        let day = chrono::DateTime::from_timestamp(now, 0)
            .context("Invalid timestamp")?
            .format("%Y-%m-%d")
            .to_string();
        let mut tx = self.pool.begin().await?;
        if let Some(row) = sqlx::query(
            "SELECT last_at FROM command_usage WHERE actor = $1 AND day = $2 AND command = $3",
        )
        .bind(&actor)
        .bind(&day)
        .bind(command)
        .fetch_optional(&mut *tx)
        .await?
        {
            let remaining = row.try_get::<i64, _>("last_at")? + cooldown as i64 - now;
            if remaining > 0 {
                return Ok(Admission::Cooldown(remaining as u64));
            }
        }
        let row = sqlx::query("SELECT CAST(COALESCE(SUM(used), 0) AS BIGINT) AS used FROM command_usage WHERE actor = $1 AND day = $2")
            .bind(&actor).bind(&day).fetch_one(&mut *tx).await?;
        if cost > 0 && row.try_get::<i64, _>("used")? + i64::from(cost) > i64::from(limit) {
            return Ok(Admission::Limit);
        }
        sqlx::query("INSERT INTO command_usage (actor, day, command, used, last_at) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (actor, day, command) DO UPDATE SET used = command_usage.used + excluded.used, last_at = excluded.last_at")
            .bind(actor).bind(day).bind(command).bind(i64::from(cost)).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Admission::Accepted)
    }

    pub async fn enqueue<T: Serialize>(&self, value: &T, send_at: i64) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO outbox (id, payload, send_at, status, created_at) VALUES ($1, $2, $3, 'pending', $4)")
            .bind(&id).bind(self.encode(&id, value)?).bind(send_at).bind(Utc::now().timestamp()).execute(&self.pool).await?;
        Ok(id)
    }

    pub async fn claim_due<T: DeserializeOwned>(&self) -> Result<Option<(String, T)>> {
        let row = sqlx::query("SELECT id, payload FROM outbox WHERE status = 'pending' AND send_at <= $1 ORDER BY send_at, created_at LIMIT 1")
            .bind(Utc::now().timestamp()).fetch_optional(&self.pool).await?;
        let Some(row) = row else { return Ok(None) };
        let id: String = row.try_get("id")?;
        let payload = row.try_get::<String, _>("payload")?;
        if sqlx::query("UPDATE outbox SET status = 'sending' WHERE id = $1 AND status = 'pending'")
            .bind(&id)
            .execute(&self.pool)
            .await?
            .rows_affected()
            == 0
        {
            return Ok(None);
        }
        match self.decode(&id, &payload) {
            Ok(value) => Ok(Some((id, value))),
            Err(error) => {
                self.finish(&id, false, "decryption_failed").await?;
                Err(error)
            }
        }
    }

    pub async fn finish(&self, id: &str, success: bool, code: &str) -> Result<()> {
        sqlx::query("UPDATE outbox SET status = $1, failure_code = $2, payload = '' WHERE id = $3")
            .bind(if success { "sent" } else { "failed" })
            .bind(code)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn cancel(&self, id: &str) -> Result<bool> {
        Ok(sqlx::query("UPDATE outbox SET status = 'cancelled', payload = '' WHERE id = $1 AND status = 'pending'")
            .bind(id).execute(&self.pool).await?.rows_affected() == 1)
    }

    pub async fn jobs(&self) -> Result<Vec<JobSummary>> {
        sqlx::query("SELECT id, send_at, status, failure_code FROM outbox ORDER BY created_at DESC LIMIT 100")
            .fetch_all(&self.pool).await?.into_iter().map(|r| Ok(JobSummary {
                id: r.try_get("id")?, send_at: r.try_get("send_at")?, status: r.try_get("status")?, failure_code: r.try_get("failure_code")?,
            })).collect()
    }

    pub async fn maintenance(&self) -> Result<()> {
        let cutoff = Utc::now().timestamp() - 7 * 86400;
        sqlx::query("DELETE FROM seen_messages WHERE seen_at < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM command_usage WHERE day < $1")
            .bind(
                (Utc::now() - chrono::Duration::days(2))
                    .format("%Y-%m-%d")
                    .to_string(),
            )
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM outbox WHERE status IN ('sent', 'failed', 'cancelled') AND created_at < $1")
            .bind(cutoff).execute(&self.pool).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SqlDatabase as Database;
    use super::*;
    async fn db() -> Database {
        Database::connect(
            "sqlite::memory:",
            Arc::new(Crypto::new(&"11".repeat(32)).unwrap()),
        )
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn cooldown_and_daily_limit_survive_database_state() {
        let db = db().await;
        let now = 1791806400;
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
            db.admit("alice", "sticker", 5, 1, 2, now + 10)
                .await
                .unwrap(),
            Admission::Limit
        );
        assert_eq!(
            db.admit("alice", "sticker", 5, 1, 2, now + 86400)
                .await
                .unwrap(),
            Admission::Accepted
        );
        assert_eq!(
            db.admit("bob", "sticker", 5, 1, 2, now).await.unwrap(),
            Admission::Accepted
        );
    }
    #[tokio::test]
    async fn personal_languages_and_feature_settings_are_encrypted_and_survive_restart() {
        let directory = tempfile::tempdir().unwrap();
        let url = format!(
            "sqlite:{}?mode=rwc",
            directory
                .path()
                .join("preferences.db")
                .display()
                .to_string()
                .replace('\\', "/")
        );
        let crypto = Arc::new(Crypto::new(&"11".repeat(32)).unwrap());
        let db = Database::connect(&url, crypto.clone()).await.unwrap();
        let mut settings =
            toml::from_str::<crate::app::config::Config>(include_str!("../../config.example.toml"))
                .unwrap()
                .bot;
        db.settings(&settings).await.unwrap();
        assert_eq!(
            db.user_language("alice@s.whatsapp.net").await.unwrap(),
            None
        );
        db.save_user_language("alice@s.whatsapp.net", "en")
            .await
            .unwrap();
        db.save_user_language("bob@s.whatsapp.net", "id")
            .await
            .unwrap();
        let rows = sqlx::query("SELECT id, payload FROM user_preferences")
            .fetch_all(&db.pool)
            .await
            .unwrap();
        for row in rows {
            assert!(!row.get::<String, _>("id").contains("@s.whatsapp.net"));
            assert!(!row.get::<String, _>("payload").contains("\"en\""));
            assert!(!row.get::<String, _>("payload").contains("\"id\""));
        }
        settings.autocorrect = false;
        settings.use_limit = false;
        settings.use_cooldown = false;
        settings.autoreadmessage = true;
        settings.autoreadstatus = true;
        settings.cooldown_secs = Some(7);
        settings.daily_limit = 42;
        db.save_settings(&settings).await.unwrap();
        db.pool.close().await;
        let reopened = Database::connect(&url, crypto).await.unwrap();
        assert_eq!(
            reopened
                .user_language("alice@s.whatsapp.net")
                .await
                .unwrap()
                .as_deref(),
            Some("en")
        );
        assert_eq!(
            reopened
                .user_language("bob@s.whatsapp.net")
                .await
                .unwrap()
                .as_deref(),
            Some("id")
        );
        assert_eq!(
            reopened
                .user_language("other@s.whatsapp.net")
                .await
                .unwrap(),
            None
        );
        let saved = reopened.settings(&settings).await.unwrap();
        assert_eq!(saved.language, settings.language);
        assert!(!saved.autocorrect && !saved.use_limit && !saved.use_cooldown);
        assert!(saved.autoreadmessage && saved.autoreadstatus);
        assert_eq!(saved.cooldown_secs, Some(7));
        assert_eq!(saved.daily_limit, 42);
        reopened
            .save_user_language("alice@s.whatsapp.net", "id")
            .await
            .unwrap();
        assert_eq!(
            reopened
                .user_language("alice@s.whatsapp.net")
                .await
                .unwrap()
                .as_deref(),
            Some("id")
        );
    }
    #[tokio::test]
    async fn outbox_encrypts_claims_once_and_cancels() {
        let db = db().await;
        let id = db
            .enqueue(&serde_json::json!({"text":"private"}), 0)
            .await
            .unwrap();
        let raw: String = sqlx::query("SELECT payload FROM outbox WHERE id = $1")
            .bind(&id)
            .fetch_one(&db.pool)
            .await
            .unwrap()
            .get("payload");
        assert!(!raw.contains("private"));
        assert_eq!(
            db.claim_due::<serde_json::Value>()
                .await
                .unwrap()
                .unwrap()
                .0,
            id
        );
        assert!(db.claim_due::<serde_json::Value>().await.unwrap().is_none());
        db.finish(&id, true, "").await.unwrap();
        assert!(!db.cancel(&id).await.unwrap());
        let pending = db.enqueue(&42, i64::MAX).await.unwrap();
        assert!(db.cancel(&pending).await.unwrap());
        assert_eq!(db.jobs().await.unwrap().len(), 2);
    }
    #[tokio::test]
    async fn settings_peers_and_deduplication() {
        let db = db().await;
        let mut settings: Settings =
            toml::from_str::<crate::app::config::Config>(include_str!("../../config.example.toml"))
                .unwrap()
                .bot;
        db.settings(&settings).await.unwrap();
        settings.prefix = "#".into();
        db.save_settings(&settings).await.unwrap();
        assert_eq!(db.settings(&settings).await.unwrap().prefix, "#");
        db.remember_peer("contact", "123@s.whatsapp.net", "Alice")
            .await
            .unwrap();
        db.remember_peer("group", "123@g.us", "").await.unwrap();
        assert_eq!(db.peer_counts().await.unwrap(), (1, 1));
        assert!(db.first_message("chat", "sender", "id").await.unwrap());
        assert!(!db.first_message("chat", "sender", "id").await.unwrap());
        db.maintenance().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "Requires POSTGRES_TEST_URL pointing to a dedicated test database"]
    async fn postgres_settings_admission_and_outbox() {
        let url = std::env::var("POSTGRES_TEST_URL")
            .expect("Set POSTGRES_TEST_URL to a dedicated test database");
        assert!(url.starts_with("postgres://") || url.starts_with("postgresql://"));
        let db = Database::connect(&url, Arc::new(Crypto::new(&"11".repeat(32)).unwrap()))
            .await
            .unwrap();
        let settings =
            toml::from_str::<crate::app::config::Config>(include_str!("../../config.example.toml"))
                .unwrap()
                .bot;
        db.settings(&settings).await.unwrap();
        db.save_settings(&settings).await.unwrap();
        let actor = uuid::Uuid::new_v4().to_string();
        db.save_user_language(&actor, "en").await.unwrap();
        assert_eq!(
            db.user_language(&actor).await.unwrap().as_deref(),
            Some("en")
        );
        let now = Utc::now().timestamp();
        assert_eq!(
            db.admit(&actor, "sticker", 5, 2, 2, now).await.unwrap(),
            Admission::Accepted
        );
        assert_eq!(
            db.admit(&actor, "sticker", 5, 2, 2, now + 5).await.unwrap(),
            Admission::Limit
        );
        assert_eq!(
            db.admit(&actor, "setlang", 0, 0, 1, now + 6).await.unwrap(),
            Admission::Accepted
        );
        let id = db
            .enqueue(&serde_json::json!({"text":"pg-test"}), 0)
            .await
            .unwrap();
        let claimed = db.claim_due::<serde_json::Value>().await.unwrap().unwrap();
        assert_eq!(claimed.0, id);
        db.finish(&id, true, "").await.unwrap();
        assert!(!db.cancel(&id).await.unwrap());
        db.remember_peer("contact", &actor, "test").await.unwrap();
        assert!(db.peer_counts().await.unwrap().0 >= 1);
        db.jobs().await.unwrap();
        db.maintenance().await.unwrap();
    }
}
