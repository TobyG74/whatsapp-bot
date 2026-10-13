mod mongo;
mod sql;

use crate::{app::config::Settings, infrastructure::security::Crypto};
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::sync::Arc;

pub struct Database {
    backend: Backend,
}

enum Backend {
    Sql(sql::SqlDatabase),
    Mongo(mongo::MongoDatabase),
}

#[derive(Serialize)]
pub struct JobSummary {
    pub id: String,
    pub send_at: i64,
    pub status: String,
    pub failure_code: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Admission {
    Accepted,
    Cooldown(u64),
    Limit,
}

#[derive(Serialize, Deserialize)]
pub struct Peer {
    pub jid: String,
    pub name: String,
}

impl Database {
    pub async fn connect(url: &str, crypto: Arc<Crypto>) -> Result<Self> {
        let backend = if url.starts_with("mongodb://") || url.starts_with("mongodb+srv://") {
            Backend::Mongo(mongo::MongoDatabase::connect(url, crypto).await?)
        } else if url.starts_with("sqlite:")
            || url.starts_with("postgres://")
            || url.starts_with("postgresql://")
        {
            Backend::Sql(sql::SqlDatabase::connect(url, crypto).await?)
        } else {
            bail!("Unsupported database URL scheme");
        };
        Ok(Self { backend })
    }

    pub async fn settings(&self, defaults: &Settings) -> Result<Settings> {
        match &self.backend {
            Backend::Sql(db) => db.settings(defaults).await,
            Backend::Mongo(db) => db.settings(defaults).await,
        }
    }

    pub async fn save_settings(&self, settings: &Settings) -> Result<()> {
        match &self.backend {
            Backend::Sql(db) => db.save_settings(settings).await,
            Backend::Mongo(db) => db.save_settings(settings).await,
        }
    }

    pub async fn remember_peer(&self, kind: &str, jid: &str, name: &str) -> Result<()> {
        match &self.backend {
            Backend::Sql(db) => db.remember_peer(kind, jid, name).await,
            Backend::Mongo(db) => db.remember_peer(kind, jid, name).await,
        }
    }

    pub async fn user_language(&self, actor: &str) -> Result<Option<String>> {
        match &self.backend {
            Backend::Sql(db) => db.user_language(actor).await,
            Backend::Mongo(db) => db.user_language(actor).await,
        }
    }

    pub async fn save_user_language(&self, actor: &str, language: &str) -> Result<()> {
        ensure!(
            crate::infrastructure::i18n::valid_code(language),
            "Invalid language code"
        );
        match &self.backend {
            Backend::Sql(db) => db.save_user_language(actor, language).await,
            Backend::Mongo(db) => db.save_user_language(actor, language).await,
        }
    }

    pub async fn peer_counts(&self) -> Result<(i64, i64)> {
        match &self.backend {
            Backend::Sql(db) => db.peer_counts().await,
            Backend::Mongo(db) => db.peer_counts().await,
        }
    }

    pub async fn first_message(&self, chat: &str, sender: &str, message_id: &str) -> Result<bool> {
        match &self.backend {
            Backend::Sql(db) => db.first_message(chat, sender, message_id).await,
            Backend::Mongo(db) => db.first_message(chat, sender, message_id).await,
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
        match &self.backend {
            Backend::Sql(db) => db.admit(actor, command, cooldown, cost, limit, now).await,
            Backend::Mongo(db) => db.admit(actor, command, cooldown, cost, limit, now).await,
        }
    }

    pub async fn enqueue<T: Serialize>(&self, value: &T, send_at: i64) -> Result<String> {
        match &self.backend {
            Backend::Sql(db) => db.enqueue(value, send_at).await,
            Backend::Mongo(db) => db.enqueue(value, send_at).await,
        }
    }

    pub async fn claim_due<T: DeserializeOwned>(&self) -> Result<Option<(String, T)>> {
        match &self.backend {
            Backend::Sql(db) => db.claim_due().await,
            Backend::Mongo(db) => db.claim_due().await,
        }
    }

    pub async fn finish(&self, id: &str, success: bool, code: &str) -> Result<()> {
        match &self.backend {
            Backend::Sql(db) => db.finish(id, success, code).await,
            Backend::Mongo(db) => db.finish(id, success, code).await,
        }
    }

    pub async fn cancel(&self, id: &str) -> Result<bool> {
        match &self.backend {
            Backend::Sql(db) => db.cancel(id).await,
            Backend::Mongo(db) => db.cancel(id).await,
        }
    }

    pub async fn jobs(&self) -> Result<Vec<JobSummary>> {
        match &self.backend {
            Backend::Sql(db) => db.jobs().await,
            Backend::Mongo(db) => db.jobs().await,
        }
    }

    pub async fn maintenance(&self) -> Result<()> {
        match &self.backend {
            Backend::Sql(db) => db.maintenance().await,
            Backend::Mongo(db) => db.maintenance().await,
        }
    }
}
