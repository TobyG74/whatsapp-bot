use crate::bot::commands::{Command, Context, Scope, reply};
use anyhow::Result;
use rust_i18n::t;

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "setlang",
    aliases: &["lang"],
    category: "general",
    description: "commands.setlang",
    cooldown_secs: 2,
    limit_cost: 0,
    scope: Scope::Both,
    owner_only: false,
    handler: |context| Box::pin(execute(context)),
};

async fn execute(context: Context<'_>) -> Result<()> {
    let mut code =
        if context.args.eq_ignore_ascii_case("en") || context.args.eq_ignore_ascii_case("id") {
            context.args.to_ascii_lowercase()
        } else {
            context.args.to_owned()
        };
    if !crate::infrastructure::i18n::ready(&code) {
        let (languages, _) = context.state.translations.languages().await;
        if let Some(language) = languages
            .iter()
            .find(|l| l.code.eq_ignore_ascii_case(&code))
        {
            code = language.code.clone();
        } else {
            return reply(
                context.state,
                context.message,
                t!(
                    "errors.language",
                    locale = context.settings.language.as_str(),
                    languages = languages
                        .iter()
                        .map(|l| l.code.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
            .await;
        }
    }
    context.state.translations.ensure(&code).await?;
    context
        .state
        .database
        .save_user_language(context.actor, &code)
        .await?;
    let text = t!("replies.language", locale = code.as_str(), language = &code);
    reply(context.state, context.message, text).await
}
