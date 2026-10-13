use super::{button, native, send};
use crate::{
    app::config::Settings,
    bot::commands::{Command, Context, Scope},
};
use anyhow::Result;
use rust_i18n::t;
use serde_json::json;
use whatsapp_rust::prelude::wa;

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "testlist",
    aliases: &["list"],
    category: "debugging",
    description: "commands.testlist",
    cooldown_secs: 3,
    limit_cost: 0,
    scope: Scope::Both,
    owner_only: true,
    handler: |context| Box::pin(execute(context)),
};

pub(super) fn build(settings: &Settings) -> wa::Message {
    let locale = settings.language.as_str();
    let rows: Vec<_> = ["ping", "runtime", "menu"].into_iter().map(|id| {
        let command = crate::bot::commands::COMMANDS.iter().find(|c| c.name == id).unwrap();
        let configured = command.configured(settings);
        json!({"title": configured.name, "description": command.description(&configured, locale), "id": format!("{}{}", settings.prefix, configured.name)})
    }).collect();
    native(
        "List test",
        &t!("debugging.choose", locale = locale),
        &settings.name,
        vec![button(
            "single_select",
            json!({"title": t!("replies.menu", locale = locale), "sections": [{"title": settings.name, "rows": rows}]}),
        )],
    )
}

async fn execute(context: Context<'_>) -> Result<()> {
    let message = build(context.settings);
    send(context, message).await
}
