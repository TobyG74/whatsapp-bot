use crate::{
    app::{
        config::{CommandSettings, Mode, Settings},
        state::Shared,
    },
    bot::messaging,
    storage::Admission,
};
use anyhow::{Result, ensure};
use chrono::Utc;
use rust_i18n::t;
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashSet},
    future::Future,
    pin::Pin,
    sync::atomic::Ordering,
};
use whatsapp_rust::prelude::*;
use whatsapp_rust::wacore_binary::JidExt;

mod autocorrect;
mod debugging;
mod general;
mod media;
mod owner;

struct Context<'a> {
    state: &'a Shared,
    message: &'a MessageContext,
    settings: &'a Settings,
    args: &'a str,
    owner: bool,
    actor: &'a str,
}

type CommandFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

pub use crate::app::config::Scope;

#[derive(Serialize)]
pub struct Command {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub category: &'static str,
    pub description: &'static str,
    pub cooldown_secs: u64,
    pub limit_cost: u32,
    pub scope: Scope,
    pub owner_only: bool,
    #[serde(skip)]
    handler: for<'a> fn(Context<'a>) -> CommandFuture<'a>,
}

pub static COMMANDS: &[Command] = &[
    general::ping::COMMAND,
    general::runtime::COMMAND,
    general::menu::COMMAND,
    general::about::COMMAND,
    general::setlang::COMMAND,
    media::sticker::COMMAND,
    debugging::testbutton::COMMAND,
    debugging::testlist::COMMAND,
    debugging::testinteractive::COMMAND,
    debugging::testhtml::COMMAND,
    owner::config::COMMAND,
    owner::setprefix::COMMAND,
    owner::setting::COMMAND,
    owner::setlimit::COMMAND,
    owner::setcooldown::COMMAND,
    owner::self_mode::COMMAND,
    owner::public::COMMAND,
];

impl Command {
    pub fn defaults(&self) -> CommandSettings {
        CommandSettings {
            name: self.name.into(),
            aliases: self.aliases.iter().map(|s| (*s).into()).collect(),
            category: self.category.into(),
            description: None,
            cooldown_secs: self.cooldown_secs,
            limit_cost: self.limit_cost,
            scope: self.scope,
            owner_only: self.owner_only,
            deleted: false,
        }
    }

    pub fn configured(&self, settings: &Settings) -> CommandSettings {
        settings
            .command_overrides
            .get(self.name)
            .cloned()
            .unwrap_or_else(|| self.defaults())
    }

    pub fn description(&self, configured: &CommandSettings, locale: &str) -> String {
        configured
            .description
            .clone()
            .unwrap_or_else(|| t!(self.description, locale = locale).to_string())
    }
}

pub fn validate_overrides(overrides: &BTreeMap<String, CommandSettings>) -> Result<()> {
    for (id, options) in overrides {
        let command = COMMANDS
            .iter()
            .find(|c| c.name == id)
            .ok_or_else(|| anyhow::anyhow!("Unknown command ID"))?;
        let valid_name = |s: &str| {
            !s.is_empty()
                && s.len() <= 32
                && s.as_bytes()[0].is_ascii_lowercase()
                && s.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b))
        };
        ensure!(
            valid_name(&options.name),
            "Command names must start with a lowercase letter and contain at most 32 lowercase letters, digits, underscores or hyphens"
        );
        ensure!(
            options.aliases.len() <= 20 && options.aliases.iter().all(|s| valid_name(s)),
            "Provide up to 20 valid aliases"
        );
        ensure!(
            !options.category.trim().is_empty()
                && options.category.len() <= 64
                && !options.category.chars().any(char::is_control),
            "Category must be 1..64 bytes without control characters"
        );
        if let Some(description) = &options.description {
            ensure!(
                !description.trim().is_empty()
                    && description.len() <= 512
                    && !description.chars().any(char::is_control),
                "Description must be 1..512 bytes without control characters"
            );
        }
        ensure!(
            options.cooldown_secs <= 86400,
            "Cooldown must be 0..86400 seconds"
        );
        ensure!(options.limit_cost <= 10000, "Quota cost must be 0..10000");
        ensure!(
            !command.owner_only || options.owner_only,
            "Owner commands must retain owner-only access"
        );
    }
    let mut names = HashSet::new();
    for command in COMMANDS {
        let options = overrides
            .get(command.name)
            .cloned()
            .unwrap_or_else(|| command.defaults());
        if !options.deleted {
            for name in std::iter::once(&options.name).chain(&options.aliases) {
                ensure!(
                    names.insert(name.clone()),
                    "Command name/alias '{name}' is already used"
                );
            }
        }
    }
    Ok(())
}

pub fn parse<'a>(
    text: &'a str,
    settings: &Settings,
) -> Option<(&'static Command, CommandSettings, &'a str)> {
    let text = text.strip_prefix(&settings.prefix)?.trim();
    let split = text.find(char::is_whitespace).unwrap_or(text.len());
    let name = text[..split].to_ascii_lowercase();
    COMMANDS.iter().find_map(|command| {
        let options = command.configured(settings);
        (!options.deleted && (options.name == name || options.aliases.contains(&name)))
            .then(|| (command, options, text[split..].trim()))
    })
}

pub fn allowed(command: &CommandSettings, owner: bool, group: bool, mode: Mode) -> bool {
    (mode == Mode::Public || owner)
        && (!command.owner_only || owner)
        && matches!(
            (command.scope, group),
            (Scope::Both, _) | (Scope::Group, true) | (Scope::Private, false)
        )
}

pub fn is_owner(
    settings: &Settings,
    sender: &Jid,
    alternative: Option<&Jid>,
    from_me: bool,
) -> bool {
    from_me
        || [Some(sender), alternative]
            .into_iter()
            .flatten()
            .any(|jid| {
                jid.server == Server::Pn
                    && settings
                        .owners
                        .iter()
                        .any(|number| number == jid.user.as_str())
            })
}

pub async fn handle(state: Shared, ctx: MessageContext) -> Result<()> {
    let mut settings = state.settings.read().await.clone();
    let Some(text) = messaging::command_text(&ctx.message) else {
        return Ok(());
    };
    let source = &ctx.info.source;
    if source.chat.is_status_broadcast() || source.chat.is_newsletter() {
        return Ok(());
    }
    let owner = is_owner(
        &settings,
        &source.sender,
        source.sender_alt.as_ref(),
        source.is_from_me,
    );
    if settings.mode == Mode::SelfOnly && !owner {
        return Ok(());
    }
    let parsed = parse(&text, &settings);
    let suggestions = if parsed.is_none() && settings.autocorrect {
        autocorrect::suggest(&text, &settings, owner, source.is_group)
    } else {
        Vec::new()
    };
    if parsed.is_none() && suggestions.is_empty() {
        return Ok(());
    }
    if !state
        .database
        .first_message(
            &source.chat.to_string(),
            &source.sender.to_string(),
            ctx.info.id.as_str(),
        )
        .await?
    {
        return Ok(());
    }
    let actor = actor(&ctx);
    settings.language = user_language(&state, &actor, &settings.language).await?;
    let locale = settings.language.as_str();
    let Some((command, options, args)) = parsed else {
        if state
            .database
            .admit(
                &actor,
                "_autocorrect",
                if settings.use_cooldown { 5 } else { 0 },
                0,
                settings.daily_limit,
                Utc::now().timestamp(),
            )
            .await?
            == Admission::Accepted
        {
            let suggestions = suggestions
                .iter()
                .map(|command| format!("• *{command}*"))
                .collect::<Vec<_>>()
                .join("\n");
            reply(
                &state,
                &ctx,
                t!(
                    "replies.suggestion",
                    locale = locale,
                    suggestions = suggestions
                ),
            )
            .await?;
            tracing::info!(command = "autocorrect", "Command suggestion sent");
        }
        return Ok(());
    };
    let rejection = if options.owner_only && !owner {
        Some(t!("errors.owner", locale = locale).to_string())
    } else if !allowed(&options, owner, source.is_group, settings.mode) {
        Some(t!("errors.scope", locale = locale).to_string())
    } else {
        None
    };
    if let Some(text) = rejection {
        reply(&state, &ctx, text).await?;
        return Ok(());
    }

    let (cooldown, cost) = admission_options(&settings, &options, owner);
    match state
        .database
        .admit(
            &actor,
            command.name,
            cooldown,
            cost,
            settings.daily_limit,
            Utc::now().timestamp(),
        )
        .await?
    {
        Admission::Cooldown(seconds) => {
            reply(
                &state,
                &ctx,
                t!("errors.cooldown", locale = locale, seconds = seconds),
            )
            .await?;
            return Ok(());
        }
        Admission::Limit => {
            reply(&state, &ctx, t!("errors.limit", locale = locale)).await?;
            return Ok(());
        }
        Admission::Accepted => (),
    }
    state.commands.fetch_add(1, Ordering::Relaxed);
    let remember_peers = async {
        state
            .database
            .remember_peer("contact", &source.sender.to_string(), &ctx.info.push_name)
            .await?;
        if source.is_group {
            state
                .database
                .remember_peer("group", &source.chat.to_string(), "")
                .await?;
        }
        Ok::<_, anyhow::Error>(())
    };
    let (peers, result) = tokio::join!(
        remember_peers,
        (command.handler)(Context {
            state: &state,
            message: &ctx,
            settings: &settings,
            args,
            owner,
            actor: &actor,
        })
    );
    peers?;
    result?;
    tracing::info!(command = %options.name, "Command completed");
    Ok(())
}

pub(crate) fn actor(ctx: &MessageContext) -> String {
    let source = &ctx.info.source;
    source
        .sender_alt
        .as_ref()
        .filter(|j| j.server == Server::Pn)
        .unwrap_or(&source.sender)
        .to_non_ad()
        .to_string()
}

pub(crate) async fn user_language(state: &Shared, actor: &str, default: &str) -> Result<String> {
    let language = state
        .database
        .user_language(actor)
        .await?
        .unwrap_or_else(|| default.into());
    if state.translations.ensure(&language).await.is_err() {
        tracing::warn!(language = %language, "Personal reply language unavailable; using bot default");
        return Ok(default.into());
    }
    Ok(language)
}

fn admission_options(settings: &Settings, command: &CommandSettings, owner: bool) -> (u64, u32) {
    (
        if settings.use_cooldown {
            settings.cooldown_secs.unwrap_or(command.cooldown_secs)
        } else {
            0
        },
        if owner || !settings.use_limit {
            0
        } else {
            command.limit_cost
        },
    )
}

async fn reply(state: &Shared, ctx: &MessageContext, text: impl Into<String>) -> Result<()> {
    ctx.reply(text).await?;
    state.sent.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

pub fn mode_name(mode: Mode) -> &'static str {
    if mode == Mode::SelfOnly {
        "self"
    } else {
        "public"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings(prefix: &str) -> Settings {
        let mut settings = toml::from_str::<crate::app::config::Config>(include_str!(
            "../../../config.example.toml"
        ))
        .unwrap()
        .bot;
        settings.prefix = prefix.into();
        settings
    }
    #[test]
    fn parses_aliases_case_and_multi_character_prefixes() {
        assert_eq!(parse("!!HeLP", &settings("!!")).unwrap().0.name, "menu");
        assert_eq!(parse("!prefix #", &settings("!")).unwrap().2, "#");
        assert!(parse("not !ping", &settings("!")).is_none());
        assert!(parse("!pingpong", &settings("!")).is_none());
        let mut names = std::collections::HashSet::new();
        for c in COMMANDS {
            assert!(names.insert(c.name));
            for alias in c.aliases {
                assert!(names.insert(alias));
            }
        }
    }
    #[test]
    fn every_command_and_alias_uses_the_registered_handler_metadata() {
        for command in COMMANDS {
            for name in std::iter::once(command.name).chain(command.aliases.iter().copied()) {
                let text = format!("!{} argument", name.to_uppercase());
                let (parsed, _, args) = parse(&text, &settings("!")).unwrap();
                assert!(std::ptr::eq(parsed, command));
                assert_eq!(args, "argument");
            }
            let metadata = serde_json::to_value(command).unwrap();
            assert_eq!(metadata.as_object().unwrap().len(), 8);
            assert_eq!(metadata["name"], command.name);
            assert_eq!(metadata["category"], command.category);
            assert!(metadata.get("handler").is_none());
        }
    }
    #[test]
    fn enforces_owner_mode_and_chat_scope() {
        let ping = COMMANDS[0].defaults();
        let config = COMMANDS
            .iter()
            .find(|command| command.name == "config")
            .unwrap()
            .defaults();
        assert!(!allowed(&ping, false, false, Mode::SelfOnly));
        assert!(allowed(&ping, false, true, Mode::Public));
        assert!(!allowed(&config, false, false, Mode::Public));
        assert!(!allowed(&config, true, true, Mode::Public));
        assert!(allowed(&config, true, false, Mode::SelfOnly));
        let setlang = COMMANDS
            .iter()
            .find(|c| c.name == "setlang")
            .unwrap()
            .defaults();
        assert!(allowed(&setlang, false, false, Mode::Public));
        assert!(allowed(&setlang, false, true, Mode::Public));
        for name in ["setting", "setlimit", "setcooldown"] {
            let options = COMMANDS.iter().find(|c| c.name == name).unwrap().defaults();
            assert!(!allowed(&options, false, false, Mode::Public));
            assert!(allowed(&options, true, true, Mode::Public));
        }
    }

    #[tokio::test]
    async fn feature_switches_control_admission_without_resetting_usage() {
        let database = crate::storage::Database::connect(
            "sqlite::memory:",
            std::sync::Arc::new(
                crate::infrastructure::security::Crypto::new(&"11".repeat(32)).unwrap(),
            ),
        )
        .await
        .unwrap();
        let mut settings = settings("!");
        let mut command = COMMANDS[0].defaults();
        command.limit_cost = 1;
        command.cooldown_secs = 3;
        settings.daily_limit = 1;
        let now = Utc::now().timestamp();
        let admit = |options: (u64, u32), time| {
            database.admit("user", "ping", options.0, options.1, 1, time)
        };
        assert_eq!(admission_options(&settings, &command, false), (3, 1));
        assert_eq!(
            admit(admission_options(&settings, &command, false), now)
                .await
                .unwrap(),
            Admission::Accepted
        );
        assert_eq!(
            admit(admission_options(&settings, &command, false), now)
                .await
                .unwrap(),
            Admission::Cooldown(3)
        );
        settings.use_cooldown = false;
        assert_eq!(
            admit(admission_options(&settings, &command, false), now)
                .await
                .unwrap(),
            Admission::Limit
        );
        settings.use_limit = false;
        assert_eq!(
            admit(admission_options(&settings, &command, false), now)
                .await
                .unwrap(),
            Admission::Accepted
        );
        assert_eq!(
            admit(admission_options(&settings, &command, false), now)
                .await
                .unwrap(),
            Admission::Accepted
        );
        settings.use_limit = true;
        assert_eq!(
            admit(admission_options(&settings, &command, false), now)
                .await
                .unwrap(),
            Admission::Limit
        );
        assert_eq!(
            admit(admission_options(&settings, &command, true), now)
                .await
                .unwrap(),
            Admission::Accepted
        );
        settings.use_cooldown = true;
        settings.cooldown_secs = Some(9);
        assert_eq!(admission_options(&settings, &command, false), (9, 1));
        assert_eq!(
            admit(admission_options(&settings, &command, false), now + 1)
                .await
                .unwrap(),
            Admission::Cooldown(8)
        );
        settings.cooldown_secs = Some(0);
        assert_eq!(admission_options(&settings, &command, false), (0, 1));
        command.limit_cost = 0;
        assert_eq!(
            admit(admission_options(&settings, &command, false), now)
                .await
                .unwrap(),
            Admission::Accepted
        );
    }
    #[test]
    fn lid_cannot_impersonate_owner_phone_number() {
        let settings = toml::from_str::<crate::app::config::Config>(include_str!(
            "../../../config.example.toml"
        ))
        .unwrap()
        .bot;
        let pn: Jid = "6281234567890@s.whatsapp.net".parse().unwrap();
        let lid: Jid = "6281234567890@lid".parse().unwrap();
        assert!(is_owner(&settings, &pn, None, false));
        assert!(!is_owner(&settings, &lid, None, false));
        assert!(is_owner(&settings, &lid, Some(&pn), false));
    }
    #[test]
    fn edited_and_deleted_commands_change_resolution_and_permissions() {
        let mut settings = settings("!");
        let mut ping = COMMANDS[0].defaults();
        ping.name = "speed".into();
        ping.aliases = vec!["fast".into()];
        ping.cooldown_secs = 60;
        ping.limit_cost = 3;
        ping.scope = Scope::Group;
        ping.description = Some("Custom description".into());
        settings.command_overrides.insert("ping".into(), ping);
        settings.validate().unwrap();
        let (handler, options, args) = parse("!FAST hello", &settings).unwrap();
        assert_eq!(handler.name, "ping");
        assert_eq!(options.name, "speed");
        assert_eq!(args, "hello");
        assert_eq!((options.cooldown_secs, options.limit_cost), (60, 3));
        assert_eq!(handler.description(&options, "id"), "Custom description");
        assert!(allowed(&options, false, true, Mode::Public));
        assert!(!allowed(&options, false, false, Mode::Public));
        assert!(parse("!ping", &settings).is_none());
        assert!(parse("!p", &settings).is_none());
        settings.command_overrides.get_mut("ping").unwrap().deleted = true;
        assert!(parse("!speed", &settings).is_none());
        assert!(parse("!fast", &settings).is_none());
    }

    #[test]
    fn rejects_collisions_invalid_options_and_weakened_owner_access() {
        let mut settings = settings("!");
        for (name, aliases) in [
            ("menu", vec![]),
            ("speed", vec!["help".into()]),
            ("speed", vec!["speed".into()]),
            ("Speed", vec![]),
        ] {
            let mut ping = COMMANDS[0].defaults();
            ping.name = name.into();
            ping.aliases = aliases;
            settings.command_overrides.insert("ping".into(), ping);
            assert!(settings.validate().is_err());
        }
        settings.command_overrides.clear();
        let mut config = COMMANDS
            .iter()
            .find(|c| c.name == "config")
            .unwrap()
            .defaults();
        config.owner_only = false;
        settings.command_overrides.insert("config".into(), config);
        assert!(settings.validate().is_err());
        settings.command_overrides.clear();
        let mut ping = COMMANDS[0].defaults();
        ping.cooldown_secs = 86401;
        settings.command_overrides.insert("ping".into(), ping);
        assert!(settings.validate().is_err());
    }

    #[test]
    fn registry_descriptions_exist_in_both_locales() {
        for locale in ["id", "en"] {
            for command in COMMANDS {
                assert_ne!(
                    t!(command.description, locale = locale).as_ref(),
                    command.description
                );
            }
            assert!(!t!("errors.cooldown", locale = locale, seconds = 5).contains("%{seconds}"));
            assert!(!t!("replies.pinging", locale = locale).contains("replies.pinging"));
            let pong = t!("replies.pong", locale = locale, response_ms = "123.45");
            assert!(pong.contains("123.45 ms"));
            assert!(!pong.contains("%{"));
            let suggestion = t!(
                "replies.suggestion",
                locale = locale,
                suggestions = "• *!!ping*"
            );
            assert!(suggestion.contains("• *!!ping*"));
            assert!(!suggestion.contains("%{"));
        }
        assert_ne!(
            t!("errors.owner", locale = "id"),
            t!("errors.owner", locale = "en")
        );
    }
}
