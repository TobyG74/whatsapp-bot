use crate::{
    app::config::Mode,
    bot::commands::{Command, Context, Scope, mode_name, reply},
};
use anyhow::Result;
use rust_i18n::t;

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "public",
    aliases: &["publicbot"],
    category: "owner",
    description: "commands.public",
    cooldown_secs: 2,
    limit_cost: 0,
    scope: Scope::Both,
    owner_only: true,
    handler: |context| Box::pin(execute(context)),
};

async fn execute(context: Context<'_>) -> Result<()> {
    context
        .state
        .change_settings(|s| s.mode = Mode::Public)
        .await?;
    reply(
        context.state,
        context.message,
        t!(
            "replies.mode",
            locale = context.settings.language.as_str(),
            mode = mode_name(Mode::Public)
        ),
    )
    .await
}
