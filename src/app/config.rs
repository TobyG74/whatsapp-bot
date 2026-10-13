use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, net::SocketAddr, path::Path};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub bot: Settings,
    pub database: DatabaseConfig,
    pub connection: ConnectionConfig,
    pub dashboard: DashboardConfig,
    pub media: MediaConfig,
    #[serde(default)]
    pub sticker: StickerConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub name: String,
    pub owners: Vec<String>,
    pub prefix: String,
    pub language: String,
    pub mode: Mode,
    pub daily_limit: u32,
    #[serde(default = "enabled")]
    pub autocorrect: bool,
    #[serde(default = "enabled")]
    pub use_limit: bool,
    #[serde(default = "enabled")]
    pub use_cooldown: bool,
    #[serde(default)]
    pub autoreadmessage: bool,
    #[serde(default)]
    pub autoreadstatus: bool,
    #[serde(default)]
    pub cooldown_secs: Option<u64>,
    #[serde(default)]
    pub command_overrides: BTreeMap<String, CommandSettings>,
    #[serde(default)]
    pub sticker: Option<StickerConfig>,
}

fn enabled() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Both,
    Group,
    Private,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSettings {
    pub name: String,
    pub aliases: Vec<String>,
    pub category: String,
    pub description: Option<String>,
    pub cooldown_secs: u64,
    pub limit_cost: u32,
    pub scope: Scope,
    pub owner_only: bool,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[serde(rename = "self")]
    SelfOnly,
    Public,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    pub driver: String,
    pub url: String,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionConfig {
    pub session_path: String,
    pub operation_timeout_secs: u64,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardConfig {
    pub bind: SocketAddr,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaConfig {
    pub max_bytes: usize,
    pub ffmpeg: String,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StickerConfig {
    #[serde(default)]
    pub packname: String,
    #[serde(default)]
    pub authorname: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let mut config: Self = toml::from_str(
            &std::fs::read_to_string(path)
                .context("Cannot read config; copy config.example.toml to config.toml")?,
        )
        .context("Invalid config TOML")?;
        if let Ok(url) = std::env::var("DATABASE_URL") {
            config.database.url = url;
        }
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        self.bot.validate()?;
        match self.database.driver.as_str() {
            "sqlite" => ensure!(
                self.database.url.starts_with("sqlite:"),
                "SQLite URL must start with sqlite:"
            ),
            "postgres" => ensure!(
                self.database.url.starts_with("postgres://")
                    || self.database.url.starts_with("postgresql://"),
                "Invalid PostgreSQL URL"
            ),
            "mongodb" => {
                // shortcut: MongoDB queues use one document per job; add GridFS for media above 8 MiB.
                ensure!(
                    self.media.max_bytes <= 8 * 1024 * 1024,
                    "MongoDB requires media.max_bytes <= 8388608 (8 MiB) for encrypted documents"
                );
                ensure!(
                    self.database.url.starts_with("mongodb://")
                        || self.database.url.starts_with("mongodb+srv://"),
                    "Invalid MongoDB URL"
                );
                ensure!(
                    self.database
                        .url
                        .split_once("://")
                        .and_then(|(_, rest)| rest.split_once('/'))
                        .map(|(_, path)| path.split('?').next().unwrap_or_default())
                        .is_some_and(|name| !name.is_empty()),
                    "MongoDB URL must include a database name"
                );
            }
            _ => bail!("database.driver must be sqlite, postgres or mongodb"),
        }
        ensure!(
            (5..=600).contains(&self.connection.operation_timeout_secs),
            "Timeout must be 5..600 seconds"
        );
        ensure!(
            !self.connection.session_path.is_empty(),
            "Session path is required"
        );
        ensure!(
            (1024..=64 * 1024 * 1024).contains(&self.media.max_bytes),
            "Media limit must be 1 KiB..64 MiB"
        );
        ensure!(
            !self.media.ffmpeg.is_empty(),
            "ffmpeg executable is required"
        );
        self.sticker.validate()?;
        Ok(())
    }
}

impl StickerConfig {
    pub fn validate(&self) -> Result<()> {
        for name in [&self.packname, &self.authorname] {
            ensure!(
                name.len() <= 256 && !name.chars().any(char::is_control),
                "Sticker packname/authorname must be at most 256 bytes without control characters"
            );
        }
        Ok(())
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.name.trim().is_empty() && self.name.len() <= 80,
            "Bot name must be 1..80 bytes"
        );
        ensure!(!self.owners.is_empty(), "At least one owner is required");
        ensure!(
            self.owners
                .iter()
                .all(|s| (7..=15).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())),
            "Owners must be international phone numbers containing 7..15 digits"
        );
        ensure!(
            (1..=8).contains(&self.prefix.chars().count())
                && !self.prefix.chars().any(char::is_whitespace)
                && !self.prefix.chars().any(char::is_control),
            "Prefix must be 1..8 visible characters without spaces"
        );
        ensure!(
            crate::infrastructure::i18n::valid_code(&self.language),
            "Language must be a valid language code"
        );
        ensure!(
            (1..=10000).contains(&self.daily_limit),
            "Daily limit must be 1..10000"
        );
        crate::bot::commands::validate_overrides(&self.command_overrides)?;
        ensure!(
            self.cooldown_secs.is_none_or(|seconds| seconds <= 86400),
            "Cooldown must be 0..86400 seconds"
        );
        if let Some(sticker) = &self.sticker {
            sticker.validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn sample() -> Config {
        toml::from_str(include_str!("../../config.example.toml")).unwrap()
    }
    #[test]
    fn sticker_configuration_is_optional_and_validated() {
        let legacy: Settings = serde_json::from_str(
            r#"{"name":"Bot","owners":["6281234567890"],"prefix":"!","language":"id","mode":"self","daily_limit":30}"#,
        )
        .unwrap();
        assert!(legacy.command_overrides.is_empty());
        assert!(legacy.sticker.is_none());
        assert!(legacy.autocorrect && legacy.use_limit && legacy.use_cooldown);
        assert!(!legacy.autoreadmessage && !legacy.autoreadstatus);
        assert_eq!(legacy.cooldown_secs, None);
        legacy.validate().unwrap();
        let example = include_str!("../../config.example.toml");
        let old = example.split("[sticker]").next().unwrap();
        let mut config: Config = toml::from_str(old).unwrap();
        assert!(config.sticker.packname.is_empty());
        assert!(config.sticker.authorname.is_empty());
        config.sticker.packname = "Stiker 😎".into();
        config.sticker.authorname = "名前".into();
        config.validate().unwrap();
        config.sticker.packname = "x".repeat(257);
        assert!(config.validate().is_err());
        config.sticker.packname.clear();
        config.sticker.authorname = "bad\nname".into();
        assert!(config.validate().is_err());
        assert!(toml::from_str::<Config>(&format!("{old}\n[update]\nauto = true")).is_err());
    }
    #[test]
    fn rejects_invalid_configuration() {
        let mut c = sample();
        c.validate().unwrap();
        c.bot.prefix = " ".into();
        assert!(c.validate().is_err());
        c = sample();
        c.bot.owners = vec!["123@lid".into()];
        assert!(c.validate().is_err());
        c = sample();
        c.database.driver = "mongodb".into();
        assert!(c.validate().is_err());
        c = sample();
        c.media.max_bytes = usize::MAX;
        assert!(c.validate().is_err());
    }
    #[test]
    fn validates_global_cooldown_and_quota_boundaries() {
        let mut config = sample();
        for seconds in [0, 86400] {
            config.bot.cooldown_secs = Some(seconds);
            config.validate().unwrap();
        }
        config.bot.cooldown_secs = Some(86401);
        assert!(config.validate().is_err());
        config.bot.cooldown_secs = None;
        for limit in [0, 10001] {
            config.bot.daily_limit = limit;
            assert!(config.validate().is_err());
        }
    }
    #[test]
    fn serializes_self_mode_as_self() {
        // WhatsApp command and configuration use the same spelling.
        assert_eq!(serde_json::to_string(&Mode::SelfOnly).unwrap(), "\"self\"");
    }
    #[test]
    fn validates_mongodb_scheme_and_database_name() {
        let mut config = sample();
        config.database.driver = "mongodb".into();
        config.media.max_bytes = 8 * 1024 * 1024;
        for url in [
            "mongodb://localhost:27017/whatsapp_bot",
            "mongodb+srv://user:password@cluster.example.com/whatsapp_bot?authSource=admin",
        ] {
            config.database.url = url.into();
            config.validate().unwrap();
        }
        for url in [
            "sqlite::memory:",
            "mongodb://localhost:27017",
            "mongodb://localhost:27017/?authSource=admin",
        ] {
            config.database.url = url.into();
            assert!(config.validate().is_err());
        }
        config.database.url = "mongodb://localhost:27017/whatsapp_bot".into();
        config.media.max_bytes += 1;
        assert!(config.validate().is_err());
    }
}
