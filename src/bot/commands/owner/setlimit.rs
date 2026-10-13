use crate::bot::commands::{Command, Context, Scope, reply};
use anyhow::Result;
use rust_i18n::t;

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "setlimit",
    aliases: &[],
    category: "owner",
    description: "commands.setlimit",
    cooldown_secs: 0,
    limit_cost: 0,
    scope: Scope::Both,
    owner_only: true,
    handler: |context| Box::pin(execute(context)),
};

async fn execute(context: Context<'_>) -> Result<()> {
    let locale = context.settings.language.as_str();
    let Ok(limit) = context.args.parse::<u32>() else {
        return reply(
            context.state,
            context.message,
            t!("errors.setlimit", locale = locale),
        )
        .await;
    };
    if !(1..=10000).contains(&limit) {
        return reply(
            context.state,
            context.message,
            t!("errors.setlimit", locale = locale),
        )
        .await;
    }
    context
        .state
        .change_settings(|settings| settings.daily_limit = limit)
        .await?;
    reply(
        context.state,
        context.message,
        t!("replies.setlimit", locale = locale, limit = limit),
    )
    .await
}
