use crate::{
    app::config::Settings,
    bot::commands::{COMMANDS, Command, Context, Scope, allowed, mode_name, reply},
};
use anyhow::Result;
use rust_i18n::t;
use std::collections::BTreeMap;

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "menu",
    aliases: &["help", "commands"],
    category: "general",
    description: "commands.menu",
    cooldown_secs: 3,
    limit_cost: 0,
    scope: Scope::Both,
    owner_only: false,
    handler: |context| Box::pin(execute(context)),
};

fn build(settings: &Settings, owner: bool, group: bool, category: &str) -> String {
    let locale = settings.language.as_str();
    let mut categories = BTreeMap::<_, Vec<_>>::new();
    for command in COMMANDS {
        let options = command.configured(settings);
        if options.deleted || !allowed(&options, owner, group, settings.mode) {
            continue;
        }
        categories
            .entry(options.category.clone())
            .or_default()
            .push((command, options));
    }
    let available_categories = categories.keys().cloned().collect::<Vec<_>>().join(", ");
    let category = category.trim().to_lowercase();
    if !category.is_empty() {
        categories.retain(|name, _| name.trim().to_lowercase() == category);
    }
    let count: usize = categories.values().map(Vec::len).sum();
    let role = if owner { "menu.owner" } else { "menu.user" };
    let mut menu = format!(
        "╭─ *{}*\n│ 🧭 {} ({count})\n│ {}: `{}` · {}: *{}*\n│ {}: {} {}\n╰────────────\n",
        settings.name,
        t!("replies.menu", locale = locale),
        t!("menu.prefix", locale = locale),
        settings.prefix,
        t!("menu.mode", locale = locale),
        mode_name(settings.mode),
        t!("menu.role", locale = locale),
        if owner { "👑" } else { "👤" },
        t!(role, locale = locale),
    );
    if !available_categories.is_empty() {
        menu.push_str(&format!(
            "\n{}\n{}\n",
            t!(
                "menu.categories",
                locale = locale,
                categories = available_categories
            ),
            t!(
                "menu.category_hint",
                locale = locale,
                prefix = &settings.prefix,
                command = COMMAND.configured(settings).name,
            ),
        ));
    }
    menu.push_str(&format!(
        "\n{}\n{}\n",
        t!("menu.roles", locale = locale),
        t!("menu.scopes", locale = locale),
    ));
    let mut categories: Vec<_> = categories.into_iter().collect();
    let order = |category: &str| match category {
        "general" => 0,
        "media" => 1,
        "owner" => 2,
        "debugging" => 3,
        _ => 4,
    };
    categories.sort_by(|(a, _), (b, _)| order(a).cmp(&order(b)).then(a.cmp(b)));
    for (category, commands) in categories {
        let (icon, label) = match category.as_str() {
            "general" => ("📚", t!("menu.general", locale = locale).to_string()),
            "media" => ("🎨", t!("menu.media", locale = locale).to_string()),
            "owner" => ("👑", t!("menu.owner_category", locale = locale).to_string()),
            "debugging" => ("🧪", t!("menu.debugging", locale = locale).to_string()),
            _ => ("📂", category),
        };
        menu.push_str(&format!("\n╭─ {icon} *{label}*\n"));
        for (command, options) in commands {
            let scope = match options.scope {
                Scope::Both => "🌐",
                Scope::Group => "👥",
                Scope::Private => "💬",
            };
            menu.push_str(&format!(
                "│ {} *{}{}* {scope}\n│   {}\n",
                if options.owner_only { "👑" } else { "👤" },
                settings.prefix,
                options.name,
                command.description(&options, locale),
            ));
            if !options.aliases.is_empty() {
                let aliases = options
                    .aliases
                    .iter()
                    .map(|alias| format!("`{}{alias}`", settings.prefix))
                    .collect::<Vec<_>>()
                    .join(", ");
                menu.push_str(&format!(
                    "│   ↳ {}: {aliases}\n",
                    t!("menu.aliases", locale = locale),
                ));
            }
        }
        menu.push_str("╰────────────\n");
    }
    if count == 0 {
        let key = if category.is_empty() {
            "menu.empty"
        } else {
            "menu.category_empty"
        };
        menu.push_str(&format!("\n{}\n", t!(key, locale = locale)));
    }
    menu
}

async fn execute(context: Context<'_>) -> Result<()> {
    let menu = build(
        context.settings,
        context.owner,
        context.message.info.source.is_group,
        context.args,
    );
    reply(context.state, context.message, menu).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::config::{Config, Mode};

    #[test]
    fn menu_groups_commands_and_respects_roles_scope_and_dashboard_edits() {
        let mut settings =
            toml::from_str::<Config>(include_str!("../../../../config.example.toml"))
                .unwrap()
                .bot;
        settings.mode = Mode::Public;
        settings.prefix = "!!".into();
        for locale in ["id", "en"] {
            settings.language = locale.into();
            let user = build(&settings, false, false, "");
            assert!(user.contains("👤 *!!ping* 🌐"));
            assert!(user.contains("`!!p`"));
            assert!(user.contains("👤 *!!setlang* 🌐"));
            assert!(!user.contains("*!!testbutton*"));
            assert!(!user.contains("menu."));
            assert!(user.find("*!!ping*").unwrap() < user.find("*!!sticker*").unwrap());
            let owner = build(&settings, true, false, "");
            assert!(owner.contains("👑 *!!config* 💬"));
            assert!(owner.contains("👑 *!!testbutton* 🌐"));
            assert!(!build(&settings, true, true, "").contains("*!!config*"));
        }
        let mut ping = COMMANDS[0].defaults();
        ping.name = "speed".into();
        ping.aliases = vec!["fast".into()];
        ping.category = "Custom tools".into();
        ping.description = Some("A custom description".into());
        ping.scope = Scope::Group;
        ping.owner_only = true;
        settings.command_overrides.insert("ping".into(), ping);
        let owner = build(&settings, true, true, "");
        assert!(owner.contains("📂 *Custom tools*"));
        assert!(owner.contains("👑 *!!speed* 👥"));
        assert!(owner.contains("A custom description"));
        assert!(owner.contains("`!!fast`"));
        assert!(!owner.contains("*!!ping*"));
        assert!(!build(&settings, false, true, "").contains("*!!speed*"));
        assert!(!build(&settings, true, false, "").contains("*!!speed*"));
        settings.command_overrides.get_mut("ping").unwrap().deleted = true;
        assert!(!build(&settings, true, true, "").contains("*!!speed*"));
        settings.mode = Mode::SelfOnly;
        let hidden = build(&settings, false, true, "");
        assert!(!hidden.contains("*!!menu*"));
        assert!(hidden.contains(t!("menu.empty", locale = "en").as_ref()));
    }

    #[test]
    fn category_filter_preserves_permissions_and_supports_custom_categories() {
        let mut settings =
            toml::from_str::<Config>(include_str!("../../../../config.example.toml"))
                .unwrap()
                .bot;
        settings.mode = Mode::Public;
        for locale in ["id", "en"] {
            settings.language = locale.into();
            let debugging = build(&settings, true, false, "  DeBuGgInG  ");
            assert!(debugging.contains("*!testbutton*"));
            assert!(debugging.contains("*!testhtml*"));
            assert!(!debugging.contains("*!ping*"));
            assert!(!debugging.contains("*!config*"));
            assert!(!debugging.contains("%{"));
            assert!(!debugging.contains("menu."));
            let general = build(&settings, false, false, "general");
            assert!(general.contains("*!ping*"));
            assert!(!general.contains("*!sticker*"));
            for category in ["unknown", "debugging", "owner"] {
                let unavailable = build(&settings, false, false, category);
                assert!(unavailable.contains(t!("menu.category_empty", locale = locale).as_ref()));
                assert!(!unavailable.contains("*!testbutton*"));
                assert!(!unavailable.contains("*!setlang*"));
                assert!(!unavailable.contains("debugging"));
                assert!(unavailable.contains("general, media"));
            }
            assert!(!build(&settings, true, true, "owner").contains("*!config*"));
        }
        let mut ping = COMMANDS[0].defaults();
        ping.name = "speed".into();
        ping.category = "Custom tools".into();
        settings.command_overrides.insert("ping".into(), ping);
        let mut menu = COMMAND.configured(&settings);
        menu.name = "catalog".into();
        settings.command_overrides.insert("menu".into(), menu);
        settings.prefix = "!!".into();
        let custom = build(&settings, false, false, "CUSTOM TOOLS");
        assert!(custom.contains("*!!speed*"));
        assert!(!custom.contains("*!!about*"));
        assert!(custom.contains("!!catalog <"));
        assert!(!build(&settings, false, false, "general").contains("*!!speed*"));
        settings.command_overrides.get_mut("ping").unwrap().deleted = true;
        let deleted = build(&settings, false, false, "custom tools");
        assert!(!deleted.contains("*!!speed*"));
        assert!(!deleted.contains("Custom tools"));
        let (_, _, args) = crate::bot::commands::parse("!!help debugging", &settings).unwrap();
        assert_eq!(args, "debugging");
    }
}
