use crate::bot::commands::{Command, Context, Scope, reply};
use anyhow::Result;
use rust_i18n::t;

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "setprefix",
    aliases: &["prefix"],
    category: "owner",
    description: "commands.setprefix",
    cooldown_secs: 2,
    limit_cost: 0,
    scope: Scope::Private,
    owner_only: true,
    handler: |context| Box::pin(execute(context)),
};

async fn execute(context: Context<'_>) -> Result<()> {
    let locale = context.settings.language.as_str();
    let mut next = context.settings.clone();
    next.prefix = context.args.into();
    let text = if next.validate().is_err() {
        t!("errors.prefix", locale = locale)
    } else {
        context
            .state
            .change_settings(|s| s.prefix = context.args.into())
            .await?;
        t!("replies.prefix", locale = locale, prefix = context.args)
    };
    reply(context.state, context.message, text).await
}
