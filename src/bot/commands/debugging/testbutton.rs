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
    name: "testbutton",
    aliases: &["button"],
    category: "debugging",
    description: "commands.testbutton",
    cooldown_secs: 3,
    limit_cost: 0,
    scope: Scope::Both,
    owner_only: true,
    handler: |context| Box::pin(execute(context)),
};

pub(super) fn build(settings: &Settings) -> wa::Message {
    let locale = settings.language.as_str();
    let buttons = ["ping", "runtime"].map(|id| {
        let command = crate::bot::commands::COMMANDS.iter().find(|c| c.name == id).unwrap();
        let configured = command.configured(settings);
        button("quick_reply", json!({"display_text": configured.name, "id": format!("{}{}", settings.prefix, configured.name)}))
    });
    native(
        "Button test",
        &t!("debugging.choose", locale = locale),
        &settings.name,
        buttons.into(),
    )
}

async fn execute(context: Context<'_>) -> Result<()> {
    let message = build(context.settings);
    send(context, message).await
}
