use crate::bot::commands::{Command, Context, Scope, reply};
use anyhow::Result;
use rust_i18n::t;

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "runtime",
    aliases: &["uptime"],
    category: "general",
    description: "commands.runtime",
    cooldown_secs: 2,
    limit_cost: 0,
    scope: Scope::Both,
    owner_only: false,
    handler: |context| Box::pin(execute(context)),
};

fn runtime_reply(seconds: u64, locale: &str) -> String {
    t!(
        "replies.runtime",
        locale = locale,
        days = seconds / 86400,
        hours = seconds / 3600 % 24,
        minutes = seconds / 60 % 60,
        seconds = seconds % 60
    )
    .to_string()
}

async fn execute(context: Context<'_>) -> Result<()> {
    reply(
        context.state,
        context.message,
        runtime_reply(
            context.state.started.elapsed().as_secs(),
            context.settings.language.as_str(),
        ),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_uptime_at_unit_boundaries_in_both_languages() {
        for (seconds, days, hours, minutes, secs) in [
            (0, 0, 0, 0, 0),
            (59, 0, 0, 0, 59),
            (60, 0, 0, 1, 0),
            (3599, 0, 0, 59, 59),
            (3600, 0, 1, 0, 0),
            (86399, 0, 23, 59, 59),
            (86400, 1, 0, 0, 0),
            (90061, 1, 1, 1, 1),
            (172801, 2, 0, 0, 1),
        ] {
            assert_eq!(
                runtime_reply(seconds, "en"),
                format!(
                    "Bot uptime: {days} days, {hours} hours, {minutes} minutes, {secs} seconds."
                )
            );
            assert_eq!(
                runtime_reply(seconds, "id"),
                format!(
                    "Bot aktif selama {days} hari, {hours} jam, {minutes} menit, {secs} detik."
                )
            );
        }
    }
}
