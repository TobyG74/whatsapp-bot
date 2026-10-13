use crate::bot::commands::{Command, Context, Scope, reply};
use anyhow::Result;
use rust_i18n::t;

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "setcooldown",
    aliases: &[],
    category: "owner",
    description: "commands.setcooldown",
    cooldown_secs: 0,
    limit_cost: 0,
    scope: Scope::Both,
    owner_only: true,
    handler: |context| Box::pin(execute(context)),
};

async fn execute(context: Context<'_>) -> Result<()> {
    let locale = context.settings.language.as_str();
    let cooldown = if context.args.eq_ignore_ascii_case("default") {
        None
    } else if let Ok(seconds @ 0..=86400) = context.args.parse::<u64>() {
        Some(seconds)
    } else {
        return reply(
            context.state,
            context.message,
            t!("errors.setcooldown", locale = locale),
        )
        .await;
    };
    context
        .state
        .change_settings(|settings| settings.cooldown_secs = cooldown)
        .await?;
    let text = match cooldown {
        Some(seconds) => t!("replies.setcooldown", locale = locale, seconds = seconds),
        None => t!("replies.cooldown_default", locale = locale),
    };
    reply(context.state, context.message, text).await
}
