use crate::bot::{
    commands::{Command, Context, Scope, reply},
    messaging,
};
use anyhow::Result;
use rust_i18n::t;

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "sticker",
    aliases: &["s", "stiker"],
    category: "media",
    description: "commands.sticker",
    cooldown_secs: 10,
    limit_cost: 2,
    scope: Scope::Both,
    owner_only: false,
    handler: |context| Box::pin(execute(context)),
};

async fn execute(context: Context<'_>) -> Result<()> {
    if !messaging::has_sticker_source(&context.message.message) {
        reply(
            context.state,
            context.message,
            t!(
                "errors.sticker_source",
                locale = context.settings.language.as_str()
            ),
        )
        .await
    } else {
        messaging::sticker(context.state, context.message).await
    }
}
