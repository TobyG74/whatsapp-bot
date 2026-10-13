use super::{COMMANDS, allowed};
use crate::app::config::Settings;

pub(super) fn suggest(text: &str, settings: &Settings, owner: bool, group: bool) -> Vec<String> {
    if !settings.autocorrect {
        return Vec::new();
    }
    let Some(name) = text
        .strip_prefix(&settings.prefix)
        .and_then(|text| text.split_whitespace().next())
    else {
        return Vec::new();
    };
    let name = name.to_ascii_lowercase();
    if !(2..=32).contains(&name.len())
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b))
    {
        return Vec::new();
    }
    let threshold = if name.len() <= 4 { 1 } else { 2 };
    let mut matches = Vec::new();
    for command in COMMANDS {
        let options = command.configured(settings);
        if options.deleted || !allowed(&options, owner, group, settings.mode) {
            continue;
        }
        let score = std::iter::once(&options.name)
            .chain(&options.aliases)
            .map(|candidate| distance(name.as_bytes(), candidate.as_bytes()))
            .min()
            .unwrap();
        if score == 0 {
            return Vec::new();
        }
        if score <= threshold {
            matches.push((score, options.name));
        }
    }
    matches.sort();
    let Some((best, _)) = matches.first() else {
        return Vec::new();
    };
    matches
        .iter()
        .take_while(|(score, _)| score == best)
        .take(3)
        .map(|(_, name)| format!("{}{name}", settings.prefix))
        .collect()
}

fn distance(a: &[u8], b: &[u8]) -> usize {
    // Command tokens are ASCII and at most 32 bytes; adjacent swaps count as one typo.
    let mut matrix = [[0_u8; 33]; 33];
    for (i, row) in matrix.iter_mut().enumerate().take(a.len() + 1) {
        row[0] = i as u8;
    }
    for (j, cell) in matrix[0].iter_mut().enumerate().take(b.len() + 1) {
        *cell = j as u8;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + u8::from(a[i - 1] != b[j - 1]));
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                matrix[i][j] = matrix[i][j].min(matrix[i - 2][j - 2] + 1);
            }
        }
    }
    usize::from(matrix[a.len()][b.len()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::config::{Config, Mode},
        bot::commands::Scope,
    };

    fn settings() -> Settings {
        let mut settings = toml::from_str::<Config>(include_str!("../../../config.example.toml"))
            .unwrap()
            .bot;
        settings.mode = Mode::Public;
        settings
    }

    #[test]
    fn recognizes_typo_kinds_aliases_and_ignores_unrelated_or_invalid_tokens() {
        let mut settings = settings();
        for text in [
            "!pnig",
            "!pign",
            "!pimg",
            "!pingg",
            "!png",
            "!PNIG argument",
        ] {
            assert_eq!(suggest(text, &settings, false, false), ["!ping"]);
        }
        assert_eq!(suggest("!hepl", &settings, false, false), ["!menu"]);
        assert_eq!(suggest("!stikre", &settings, false, false), ["!sticker"]);
        for text in ["hello", "!", "!p", "!ping", "!zzzzzzzz", "!pïng", "!pi.ng"] {
            assert!(suggest(text, &settings, false, false).is_empty(), "{text}");
        }
        assert!(suggest(&format!("!{}", "x".repeat(10000)), &settings, false, false).is_empty());
        settings.prefix = "!!".into();
        assert_eq!(suggest("!!pnig", &settings, false, true), ["!!ping"]);
        assert!(suggest("!pnig", &settings, false, true).is_empty());
        settings.autocorrect = false;
        assert!(suggest("!!pnig", &settings, false, true).is_empty());
    }

    #[test]
    fn respects_owner_scope_self_mode_and_renamed_or_deleted_commands() {
        let mut settings = settings();
        assert_eq!(suggest("!setlng", &settings, false, false), ["!setlang"]);
        assert_eq!(suggest("!setlng", &settings, true, false), ["!setlang"]);
        assert!(suggest("!testbuton", &settings, false, false).is_empty());
        assert_eq!(
            suggest("!testbuton", &settings, true, false),
            ["!testbutton"]
        );
        assert!(suggest("!confg", &settings, true, true).is_empty());
        assert_eq!(suggest("!confg", &settings, true, false), ["!config"]);
        settings.mode = Mode::SelfOnly;
        assert!(suggest("!pnig", &settings, false, false).is_empty());
        assert_eq!(suggest("!pnig", &settings, true, false), ["!ping"]);
        settings.mode = Mode::Public;
        let mut ping = COMMANDS[0].defaults();
        ping.name = "speed".into();
        ping.aliases = vec!["fast".into()];
        ping.scope = Scope::Group;
        settings.command_overrides.insert("ping".into(), ping);
        assert_eq!(suggest("!sped", &settings, false, true), ["!speed"]);
        assert_eq!(suggest("!fsat", &settings, false, true), ["!speed"]);
        assert!(suggest("!sped", &settings, false, false).is_empty());
        assert!(suggest("!pnig", &settings, false, true).is_empty());
        settings
            .command_overrides
            .get_mut("ping")
            .unwrap()
            .owner_only = true;
        assert!(suggest("!sped", &settings, false, true).is_empty());
        assert_eq!(suggest("!sped", &settings, true, true), ["!speed"]);
        settings.command_overrides.get_mut("ping").unwrap().deleted = true;
        assert!(suggest("!sped", &settings, true, true).is_empty());
    }

    #[test]
    fn returns_at_most_three_best_matches_in_stable_order() {
        let mut settings = settings();
        for (id, name) in [
            ("ping", "cat"),
            ("runtime", "bat"),
            ("about", "hat"),
            ("sticker", "mat"),
        ] {
            let mut options = COMMANDS.iter().find(|c| c.name == id).unwrap().defaults();
            options.name = name.into();
            options.aliases.clear();
            settings.command_overrides.insert(id.into(), options);
        }
        assert_eq!(
            suggest("!dat", &settings, false, false),
            ["!bat", "!cat", "!hat"]
        );
    }

    #[tokio::test]
    async fn suggestion_admission_deduplicates_throttles_and_preserves_quota() {
        use crate::{
            infrastructure::security::Crypto,
            storage::{Admission, Database},
        };
        use std::sync::Arc;

        let db = Database::connect(
            "sqlite::memory:",
            Arc::new(Crypto::new(&"11".repeat(32)).unwrap()),
        )
        .await
        .unwrap();
        assert!(db.first_message("chat", "user", "typo-id").await.unwrap());
        assert!(!db.first_message("chat", "user", "typo-id").await.unwrap());
        let now = chrono::Utc::now().timestamp();
        let (first, second) = tokio::join!(
            db.admit("user", "_autocorrect", 5, 0, 1, now),
            db.admit("user", "_autocorrect", 5, 0, 1, now),
        );
        let results = [first.unwrap(), second.unwrap()];
        assert_eq!(
            results
                .iter()
                .filter(|result| **result == Admission::Accepted)
                .count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| **result == Admission::Cooldown(5))
                .count(),
            1
        );
        assert_eq!(
            db.admit("user", "ping", 0, 1, 1, now).await.unwrap(),
            Admission::Accepted
        );
        assert_eq!(
            db.admit("user", "_autocorrect", 5, 0, 1, now + 5)
                .await
                .unwrap(),
            Admission::Accepted
        );
        assert_eq!(
            db.admit("user", "ping", 0, 1, 1, now + 5).await.unwrap(),
            Admission::Limit
        );
    }
}
