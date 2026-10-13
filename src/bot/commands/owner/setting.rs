use crate::app::config::Settings;
use crate::bot::commands::{Command, Context, Scope, reply};
use anyhow::Result;
use rust_i18n::t;

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "setting",
    aliases: &[],
    category: "owner",
    description: "commands.setting",
    cooldown_secs: 0,
    limit_cost: 0,
    scope: Scope::Both,
    owner_only: true,
    handler: |context| Box::pin(execute(context)),
};

fn toggle(settings: &mut Settings, args: &str) -> bool {
    let args = args.to_ascii_lowercase();
    let parts: Vec<_> = args.split_whitespace().collect();
    let [name, value] = parts.as_slice() else {
        return false;
    };
    let value = match *value {
        "on" => true,
        "off" => false,
        _ => return false,
    };
    let field = match *name {
        "autocorrect" => &mut settings.autocorrect,
        "use_limit" | "uselimit" => &mut settings.use_limit,
        "use_cooldown" | "usecooldown" => &mut settings.use_cooldown,
        "autoreadmessage" => &mut settings.autoreadmessage,
        "autoreadstatus" => &mut settings.autoreadstatus,
        _ => return false,
    };
    *field = value;
    true
}

async fn execute(context: Context<'_>) -> Result<()> {
    let mut preview = context.settings.clone();
    let locale = context.settings.language.as_str();
    let name = context
        .settings
        .command_overrides
        .get("setting")
        .map(|command| command.name.as_str())
        .unwrap_or("setting");
    if context.args.is_empty() {
        let features = [
            ("autocorrect", preview.autocorrect),
            ("use_limit", preview.use_limit),
            ("use_cooldown", preview.use_cooldown),
            ("autoreadmessage", preview.autoreadmessage),
            ("autoreadstatus", preview.autoreadstatus),
        ]
        .into_iter()
        .map(|(name, value)| format!("{name}: {}", if value { "on" } else { "off" }))
        .collect::<Vec<_>>()
        .join("\n");
        return reply(
            context.state,
            context.message,
            t!(
                "replies.settings",
                locale = locale,
                features = features,
                prefix = &preview.prefix,
                command = name
            ),
        )
        .await;
    }
    if !toggle(&mut preview, context.args) {
        return reply(
            context.state,
            context.message,
            t!(
                "errors.setting",
                locale = locale,
                prefix = &preview.prefix,
                command = name
            ),
        )
        .await;
    }
    context
        .state
        .change_settings(|settings| {
            toggle(settings, context.args);
        })
        .await?;
    reply(
        context.state,
        context.message,
        t!("replies.setting", locale = locale, setting = context.args),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_only_known_features_with_explicit_on_off() {
        let mut settings = toml::from_str::<crate::app::config::Config>(include_str!(
            "../../../../config.example.toml"
        ))
        .unwrap()
        .bot;
        for feature in [
            "autocorrect",
            "use_limit",
            "uselimit",
            "use_cooldown",
            "usecooldown",
            "autoreadmessage",
            "autoreadstatus",
        ] {
            assert!(toggle(&mut settings, &format!("{feature} OFF")));
            assert!(toggle(&mut settings, &format!("{feature} on")));
        }
        assert!(
            settings.autocorrect
                && settings.use_limit
                && settings.use_cooldown
                && settings.autoreadmessage
                && settings.autoreadstatus
        );
        for args in [
            "",
            "use_limit",
            "use_limit yes",
            "mode off",
            "use_limit off extra",
        ] {
            let before = serde_json::to_value(&settings).unwrap();
            assert!(!toggle(&mut settings, args));
            assert_eq!(before, serde_json::to_value(&settings).unwrap());
        }
    }
}
