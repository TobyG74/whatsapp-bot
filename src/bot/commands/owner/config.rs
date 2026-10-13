use crate::bot::commands::{Command, Context, Scope, mode_name, reply};
use anyhow::Result;
use rust_i18n::t;

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "config",
    aliases: &["settings"],
    category: "owner",
    description: "commands.config",
    cooldown_secs: 2,
    limit_cost: 0,
    scope: Scope::Private,
    owner_only: true,
    handler: |context| Box::pin(execute(context)),
};

async fn execute(context: Context<'_>) -> Result<()> {
    let settings = context.state.settings.read().await.clone();
    let text = format!(
        "{}\n{}",
        settings.name,
        t!(
            "replies.config",
            locale = context.settings.language.as_str(),
            prefix = &settings.prefix,
            language = &settings.language,
            mode = mode_name(settings.mode),
            limit = settings.daily_limit
        )
    );
    reply(context.state, context.message, text).await
}
