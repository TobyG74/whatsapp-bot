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
    name: "testinteractive",
    aliases: &["interactive"],
    category: "debugging",
    description: "commands.testinteractive",
    cooldown_secs: 3,
    limit_cost: 0,
    scope: Scope::Both,
    owner_only: true,
    handler: |context| Box::pin(execute(context)),
};

pub(super) fn build(settings: &Settings) -> wa::Message {
    let locale = settings.language.as_str();
    let command = crate::bot::commands::COMMANDS
        .iter()
        .find(|c| c.name == "ping")
        .unwrap()
        .configured(settings);
    native(
        "Interactive test",
        &t!("debugging.choose", locale = locale),
        &settings.name,
        vec![
            button(
                "quick_reply",
                json!({"display_text": command.name, "id": format!("{}{}", settings.prefix, command.name)}),
            ),
            button(
                "cta_url",
                json!({"display_text": "GitHub", "url": "https://github.com/oxidezap/whatsapp-rust", "merchant_url": "https://github.com/oxidezap/whatsapp-rust"}),
            ),
            button(
                "cta_copy",
                json!({"display_text": "Copy", "copy_code": "WHATSAPP-BOT-TEST"}),
            ),
        ],
    )
}

async fn execute(context: Context<'_>) -> Result<()> {
    let message = build(context.settings);
    send(context, message).await
}
