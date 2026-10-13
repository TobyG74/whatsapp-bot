use super::{commands, messaging::command_text};
use crate::app::config::Settings;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use whatsapp_rust::{prelude::*, wacore_binary::JidExt};

#[derive(Default)]
pub struct MessageLogger {
    group_names: Mutex<HashMap<String, (Instant, String)>>,
}

impl MessageLogger {
    pub async fn invalidate(&self, jid: &Jid) {
        self.group_names.lock().await.remove(&jid.to_string());
    }

    async fn group_name(
        &self,
        jid: &Jid,
        fetch: impl std::future::Future<Output = Option<String>>,
    ) -> String {
        let key = jid.to_string();
        let requested_at = Instant::now();
        {
            let mut names = self.group_names.lock().await;
            if let Some((at, name)) = names.get(&key)
                && at.elapsed() < Duration::from_secs(300)
            {
                return name.clone();
            }
            if names.len() >= 512 {
                names.clear();
            }
            // Reserve misses so simultaneous messages do not repeat the metadata query.
            names.insert(key.clone(), (requested_at, String::new()));
        }
        let name = tokio::time::timeout(Duration::from_secs(2), fetch)
            .await
            .ok()
            .flatten()
            .map(|name| crate::infrastructure::logging::preview(&name))
            .unwrap_or_default();
        let mut names = self.group_names.lock().await;
        // A group update may have invalidated this query while it was in flight.
        if let Some((at, cached)) = names.get_mut(&key)
            && *at == requested_at
        {
            *cached = name.clone();
        }
        name
    }

    pub async fn record(&self, ctx: &MessageContext, settings: &Settings) {
        let group_name = if ctx.info.source.is_group {
            let jid = &ctx.info.source.chat;
            self.group_name(jid, async {
                ctx.client
                    .groups()
                    .get_metadata(jid)
                    .await
                    .ok()
                    .map(|metadata| metadata.subject)
            })
            .await
        } else {
            String::new()
        };
        record(ctx, settings, &group_name);
    }
}

fn command_name(text: &str, settings: &Settings) -> Option<String> {
    let name = text
        .strip_prefix(&settings.prefix)?
        .split_whitespace()
        .next()?;
    Some(
        commands::parse(text, settings)
            .map(|(_, options, _)| options.name)
            .unwrap_or_else(|| name.to_lowercase()),
    )
}

fn sender_id(sender: &Jid, alternative: Option<&Jid>) -> Jid {
    if sender.server == Server::Pn {
        sender.to_non_ad()
    } else {
        alternative
            .filter(|jid| jid.server == Server::Pn)
            .unwrap_or(sender)
            .to_non_ad()
    }
}

fn record(ctx: &MessageContext, settings: &Settings, group_name: &str) {
    let source = &ctx.info.source;
    if source.chat.is_status_broadcast() || source.chat.is_newsletter() {
        return;
    }
    let base = ctx.message.get_base_message();
    let media = if base.image_message.as_option().is_some() {
        "image"
    } else if base.video_message.as_option().is_some() {
        "video"
    } else if base.audio_message.as_option().is_some() {
        "audio"
    } else if base.document_message.as_option().is_some() {
        "document"
    } else if base.sticker_message.as_option().is_some() {
        "sticker"
    } else if base.reaction_message.as_option().is_some() {
        "reaction"
    } else if command_text(&ctx.message).is_some() {
        "text"
    } else {
        "other"
    };
    let text = command_text(&ctx.message)
        .or_else(|| {
            base.document_message
                .as_option()
                .and_then(|m| m.caption.as_deref())
                .map(std::borrow::Cow::Borrowed)
        })
        .unwrap_or_default();
    let command = command_name(&text, settings);
    tracing::info!(target: "whatsapp_bot::messages",
        kind = if command.is_some() { "command" } else { "message" },
        chat_type = if source.is_group { "group" } else { "private" },
        direction = if source.is_from_me { "outgoing" } else { "incoming" },
        sender = %sender_id(&source.sender, source.sender_alt.as_ref()), chat = %source.chat,
        group_name,
        name = %ctx.info.push_name, media, command = %command.unwrap_or_default(),
        preview = %crate::infrastructure::logging::preview(&text), message_time = %ctx.info.timestamp.to_rfc3339(),
        "Message received");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uses_phone_numbers_when_known_and_preserves_unknown_lids() {
        let phone: Jid = "6281234567890:3@s.whatsapp.net".parse().unwrap();
        let lid: Jid = "123@lid".parse().unwrap();
        assert_eq!(
            sender_id(&phone, Some(&lid)).to_string(),
            "6281234567890@s.whatsapp.net"
        );
        assert_eq!(
            sender_id(&lid, Some(&phone)).to_string(),
            "6281234567890@s.whatsapp.net"
        );
        assert_eq!(sender_id(&lid, None), lid);
    }
    #[tokio::test]
    async fn group_names_are_cached_expire_and_refresh_after_updates() {
        let logger = MessageLogger::default();
        let jid: Jid = "123@g.us".parse().unwrap();
        assert_eq!(
            logger
                .group_name(&jid, std::future::ready(Some("Old group".into())))
                .await,
            "Old group"
        );
        assert_eq!(
            logger
                .group_name(&jid, async {
                    panic!("Warm entries must not query WhatsApp")
                })
                .await,
            "Old group"
        );
        logger
            .group_names
            .lock()
            .await
            .get_mut(&jid.to_string())
            .unwrap()
            .0 = Instant::now() - Duration::from_secs(301);
        assert_eq!(
            logger
                .group_name(&jid, std::future::ready(Some("Fresh group".into())))
                .await,
            "Fresh group"
        );
        logger.invalidate(&jid).await;
        let stale_query = async {
            logger.invalidate(&jid).await;
            logger
                .group_name(&jid, std::future::ready(Some("Renamed group".into())))
                .await;
            Some("Stale group".into())
        };
        logger.group_name(&jid, stale_query).await;
        assert_eq!(
            logger
                .group_name(&jid, async {
                    panic!("An older query must not overwrite a rename")
                })
                .await,
            "Renamed group"
        );
    }
    #[tokio::test]
    async fn in_flight_group_lookup_does_not_delay_other_messages() {
        let logger = std::sync::Arc::new(MessageLogger::default());
        let jid: Jid = "123@g.us".parse().unwrap();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (finish_tx, finish_rx) = tokio::sync::oneshot::channel();
        let task_logger = logger.clone();
        let task_jid = jid.clone();
        let lookup = tokio::spawn(async move {
            task_logger
                .group_name(&task_jid, async {
                    started_tx.send(()).unwrap();
                    finish_rx.await.ok()
                })
                .await
        });
        started_rx.await.unwrap();
        let cached = tokio::time::timeout(
            Duration::from_millis(100),
            logger.group_name(&jid, async {
                panic!("An in-flight query must not be repeated")
            }),
        )
        .await
        .expect("Other messages must not wait for an in-flight query");
        assert_eq!(cached, "");
        finish_tx.send("Rust Group".into()).unwrap();
        assert_eq!(lookup.await.unwrap(), "Rust Group");
        assert_eq!(
            logger
                .group_name(&jid, async { panic!("Completed metadata must be cached") })
                .await,
            "Rust Group"
        );
    }
    #[tokio::test]
    async fn missing_group_names_do_not_repeat_queries_and_cache_is_bounded() {
        let logger = MessageLogger::default();
        let jid: Jid = "123@g.us".parse().unwrap();
        assert_eq!(logger.group_name(&jid, std::future::ready(None)).await, "");
        assert_eq!(
            logger
                .group_name(&jid, async { panic!("Failed lookups must be cached too") })
                .await,
            ""
        );
        for id in 0..513 {
            let jid: Jid = format!("{id}@g.us").parse().unwrap();
            logger
                .group_name(&jid, std::future::ready(Some("Group".into())))
                .await;
        }
        assert!(logger.group_names.lock().await.len() <= 512);
    }
    #[test]
    fn classifies_aliases_unknown_commands_and_ordinary_messages() {
        let mut settings =
            toml::from_str::<crate::app::config::Config>(include_str!("../../config.example.toml"))
                .unwrap()
                .bot;
        assert_eq!(
            command_name("!PING test", &settings).as_deref(),
            Some("ping")
        );
        assert_eq!(
            command_name("!unknown", &settings).as_deref(),
            Some("unknown")
        );
        assert_eq!(command_name("hello!", &settings), None);
        assert_eq!(command_name("!   ", &settings), None);
        let mut ping = commands::COMMANDS[0].defaults();
        ping.name = "speed".into();
        settings.command_overrides.insert("ping".into(), ping);
        assert_eq!(command_name("!p", &settings).as_deref(), Some("speed"));
        settings.prefix = "##".into();
        assert_eq!(command_name("##menu", &settings).as_deref(), Some("menu"));
    }
}
