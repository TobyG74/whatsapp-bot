use crate::bot::commands::{Command, Context, Scope};
use anyhow::Result;
use rust_i18n::t;
use std::{sync::atomic::Ordering, time::Instant};
use whatsapp_rust::prelude::{MessageBuilderExt, wa};

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "ping",
    aliases: &["p"],
    category: "general",
    description: "commands.ping",
    cooldown_secs: 2,
    limit_cost: 1,
    scope: Scope::Both,
    owner_only: false,
    handler: |context| Box::pin(execute(context)),
};

async fn execute(context: Context<'_>) -> Result<()> {
    let locale = context.settings.language.as_str();
    let started = Instant::now();
    let sent = context
        .message
        .reply(t!("replies.pinging", locale = locale))
        .await?;
    let response_ms = format!("{:.2}", started.elapsed().as_secs_f64() * 1000.0);
    context.state.sent.fetch_add(1, Ordering::Relaxed);
    context
        .message
        .edit_message(
            sent.message_id,
            wa::Message::text(t!(
                "replies.pong",
                locale = locale,
                response_ms = response_ms
            )),
        )
        .await?;
    Ok(())
}
