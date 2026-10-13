use crate::bot::commands::{Command, Context, Scope, reply};
use anyhow::Result;
use rust_i18n::t;

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "about",
    aliases: &["info"],
    category: "general",
    description: "commands.about",
    cooldown_secs: 3,
    limit_cost: 0,
    scope: Scope::Both,
    owner_only: false,
    handler: |context| Box::pin(execute(context)),
};

async fn execute(context: Context<'_>) -> Result<()> {
    reply(
        context.state,
        context.message,
        t!(
            "replies.about",
            locale = context.settings.language.as_str(),
            name = &context.settings.name,
            version = env!("CARGO_PKG_VERSION")
        ),
    )
    .await
}
