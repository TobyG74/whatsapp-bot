use crate::{
    app::config::Mode,
    bot::commands::{Command, Context, Scope, mode_name, reply},
};
use anyhow::Result;
use rust_i18n::t;

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "self",
    aliases: &["privatebot"],
    category: "owner",
    description: "commands.self",
    cooldown_secs: 2,
    limit_cost: 0,
    scope: Scope::Both,
    owner_only: true,
    handler: |context| Box::pin(execute(context)),
};

async fn execute(context: Context<'_>) -> Result<()> {
    context
        .state
        .change_settings(|s| s.mode = Mode::SelfOnly)
        .await?;
    reply(
        context.state,
        context.message,
        t!(
            "replies.mode",
            locale = context.settings.language.as_str(),
            mode = mode_name(Mode::SelfOnly)
        ),
    )
    .await
}
