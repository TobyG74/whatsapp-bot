use crate::{
    app::config::{Config, Settings},
    infrastructure::{i18n::Translator, logging::LogBuffer},
    storage::Database,
};
use anyhow::Result;
use serde::Serialize;
use std::{
    sync::{Arc, atomic::AtomicU64},
    time::{Duration, Instant},
};
use tokio::sync::{RwLock, Semaphore};
use whatsapp_rust::Client;

pub type Shared = Arc<AppState>;
pub struct AppState {
    pub config: Config,
    pub settings: RwLock<Settings>,
    pub database: Database,
    pub translations: Translator,
    pub token: String,
    pub client: RwLock<Option<Arc<Client>>>,
    pub connection: RwLock<ConnectionStatus>,
    pub logs: LogBuffer,
    pub started: Instant,
    pub received: AtomicU64,
    pub commands: AtomicU64,
    pub sent: AtomicU64,
    pub failures: AtomicU64,
    pub media_slots: Semaphore,
}

#[derive(Clone, Serialize)]
pub struct ConnectionStatus {
    pub state: String,
    pub qr_data_url: Option<String>,
    pub qr_expires_at: Option<i64>,
}

impl ConnectionStatus {
    pub fn new(state: &str) -> Self {
        Self {
            state: state.into(),
            qr_data_url: None,
            qr_expires_at: None,
        }
    }
}

impl AppState {
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.config.connection.operation_timeout_secs)
    }

    pub async fn change_settings(&self, change: impl FnOnce(&mut Settings)) -> Result<()> {
        let mut guard = self.settings.write().await;
        let mut next = guard.clone();
        change(&mut next);
        next.validate()?;
        self.database.save_settings(&next).await?;
        *guard = next;
        tracing::info!("Bot settings saved");
        Ok(())
    }

    pub async fn connected_client(&self) -> Option<Arc<Client>> {
        self.client
            .read()
            .await
            .as_ref()
            .filter(|c| c.is_connected() && c.is_logged_in())
            .cloned()
    }
}
