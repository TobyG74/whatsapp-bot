use chrono::Local;
use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    io::IsTerminal,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::{Arc, Mutex},
};
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{
    EnvFilter, Layer,
    fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
    layer::Context,
    prelude::*,
    registry::LookupSpan,
};

pub type LogBuffer = Arc<Mutex<VecDeque<LogEntry>>>;

const STARTUP_BANNER: &str = r"
 _       __ __          __       ___                ____  ____  ______
| |     / // /_  ____ _/ /______/   |  ____  ____   / __ )/ __ \/_  __/
| | /| / // __ \/ __ `/ __/ ___/ /| | / __ \/ __ \ / __  / / / / / /
| |/ |/ // / / / /_/ / /_(__  ) ___ |/ /_/ / /_/ // /_/ / /_/ / / /
|__/|__//_/ /_/\__,_/\__/____/_/  |_/ .___/ .___//_____/\____/ /_/
                                  /_/   /_/
";

fn ansi_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none()
        && (std::io::stdout().is_terminal()
            || std::env::var("CLICOLOR_FORCE").is_ok_and(|v| v != "0"))
}

pub fn startup_banner() {
    if std::env::var("LOG_FORMAT").is_ok_and(|v| v == "json") {
        return;
    }
    let ansi = ansi_enabled();
    println!(
        "{}{}{}\n  {}Github: TobyG74{}\n",
        if ansi { "\x1b[1;92m" } else { "" },
        STARTUP_BANNER,
        if ansi { "\x1b[0m" } else { "" },
        if ansi { "\x1b[2m" } else { "" },
        if ansi { "\x1b[0m" } else { "" },
    );
}

pub fn dashboard_started(address: SocketAddr) {
    let mut browser_address = address;
    if address.ip().is_unspecified() {
        browser_address.set_ip(match address.ip() {
            IpAddr::V4(_) => Ipv4Addr::LOCALHOST.into(),
            IpAddr::V6(_) => Ipv6Addr::LOCALHOST.into(),
        });
    }
    tracing::info!(address = %address, port = address.port(),
        "Dashboard running on port {} | Open http://{}", address.port(), browser_address);
}
#[derive(Clone, Serialize)]
pub struct LogEntry {
    pub time: String,
    pub level: String,
    pub message: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}

pub fn preview(value: &str) -> String {
    let mut chars = value.chars().filter(|c| !matches!(*c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'));
    let mut result: String = chars
        .by_ref()
        .take(300)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if chars.next().is_some() {
        result.push('…');
    }
    result
}

#[derive(Default)]
struct Fields(BTreeMap<String, String>);
impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        if matches!(
            field.name(),
            "message"
                | "command"
                | "state"
                | "failure_code"
                | "error"
                | "job_id"
                | "kind"
                | "chat_type"
                | "direction"
                | "sender"
                | "chat"
                | "group_name"
                | "name"
                | "preview"
                | "media"
                | "message_time"
        ) {
            self.0.insert(field.name().into(), preview(value));
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.record_str(field, &format!("{value:?}"));
    }
}

impl Fields {
    fn summary(&self) -> String {
        let message = self.0.get("message").cloned().unwrap_or_default();
        if self.0.contains_key("chat_type") {
            return message;
        }
        let extra: Vec<_> = self
            .0
            .iter()
            .filter(|(k, _)| k.as_str() != "message")
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        if extra.is_empty() {
            message
        } else {
            format!("{message} · {}", extra.join(" "))
        }
    }
}

struct DashboardLogs(LogBuffer);
impl<S: Subscriber> Layer<S> for DashboardLogs {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        if !event.metadata().target().starts_with("whatsapp_bot") {
            return;
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        let message = fields.summary();
        let details = if fields.0.contains_key("chat_type") {
            fields.0
        } else {
            BTreeMap::new()
        };
        if let Ok(mut logs) = self.0.lock() {
            if logs.len() >= 200 {
                logs.pop_front();
            }
            logs.push_back(LogEntry {
                time: chrono::Utc::now().to_rfc3339(),
                level: event.metadata().level().to_string(),
                message,
                details,
            });
        }
    }
}

struct Console;
impl<S, N> FormatEvent<S, N> for Console
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let ansi = writer.has_ansi_escapes();
        let get = |key: &str| fields.0.get(key).map(String::as_str).unwrap_or("");
        let level = event.metadata().level();
        let tag = if *level == tracing::Level::ERROR {
            "ERR"
        } else if *level == tracing::Level::WARN {
            "WRN"
        } else if get("chat_type").is_empty() {
            match *level {
                tracing::Level::DEBUG => "DBG",
                tracing::Level::TRACE => "TRC",
                _ => "INF",
            }
        } else if get("kind") == "command" {
            "CMD"
        } else {
            "MSG"
        };
        let color = if tag == "ERR" {
            "\x1b[91m"
        } else if *level == tracing::Level::WARN {
            "\x1b[93m"
        } else if get("kind") == "command" {
            "\x1b[96m"
        } else if get("chat_type") == "group" {
            "\x1b[93m"
        } else {
            "\x1b[92m"
        };
        write!(
            writer,
            "{} {}[{}]{} ",
            Local::now().format("%H:%M:%S"),
            if ansi { color } else { "" },
            tag,
            if ansi { "\x1b[0m" } else { "" }
        )?;
        if fields.0.contains_key("chat_type") {
            let sender = get("sender");
            let number = sender.strip_suffix("@s.whatsapp.net").unwrap_or(sender);
            let name = get("name").trim();
            let sender = if !name.is_empty() {
                name
            } else if !number.is_empty() {
                number
            } else {
                get("chat")
            };
            write!(writer, "{sender}")?;
            if get("chat_type") == "group" {
                let group_name = get("group_name").trim();
                let group = if group_name.is_empty() {
                    get("chat")
                } else {
                    group_name
                };
                write!(writer, " → {group}")?;
            }
            if !get("preview").is_empty() {
                writeln!(writer, " | {}", get("preview"))
            } else if tag == "CMD" {
                writeln!(writer, " | {}", get("command"))
            } else {
                writeln!(writer, " | [{}]", get("media"))
            }
        } else {
            writeln!(writer, "{}", fields.summary())
        }
    }
}

pub fn init() -> LogBuffer {
    let logs = Arc::new(Mutex::new(VecDeque::new()));
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "whatsapp_bot=info,whatsapp_rust=error,wacore=error".into());
    let ansi = ansi_enabled();
    let output = if std::env::var("LOG_FORMAT").is_ok_and(|v| v == "json") {
        tracing_subscriber::fmt::layer()
            .json()
            .with_ansi(false)
            .boxed()
    } else {
        tracing_subscriber::fmt::layer()
            .event_format(Console)
            .with_ansi(ansi)
            .boxed()
    };
    tracing_subscriber::registry()
        .with(filter)
        .with(output)
        .with(DashboardLogs(logs.clone()))
        .init();
    logs
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dashboard_startup_keeps_url_and_port_in_compact_logs() {
        for (bind, url) in [
            ("127.0.0.1:3400", "http://127.0.0.1:3400"),
            ("0.0.0.0:3400", "http://127.0.0.1:3400"),
            ("[::]:3400", "http://[::1]:3400"),
            ("[::1]:3400", "http://[::1]:3400"),
        ] {
            let output = Capture(Arc::new(Mutex::new(Vec::new())));
            let writer = output.clone();
            let subscriber = tracing_subscriber::registry().with(
                tracing_subscriber::fmt::layer()
                    .event_format(Console)
                    .with_ansi(false)
                    .with_writer(move || writer.clone()),
            );
            tracing::subscriber::with_default(subscriber, || {
                dashboard_started(bind.parse().unwrap())
            });
            let text = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
            assert!(text.contains(&format!(
                "[INF] Dashboard running on port 3400 | Open {url}"
            )));
        }
    }
    #[derive(Clone)]
    struct Capture(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Capture {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn console_has_time_chat_labels_and_optional_color() {
        for ansi in [false, true] {
            let output = Capture(Arc::new(Mutex::new(Vec::new())));
            let writer = output.clone();
            let subscriber = tracing_subscriber::registry().with(
                tracing_subscriber::fmt::layer()
                    .event_format(Console)
                    .with_ansi(ansi)
                    .with_writer(move || writer.clone()),
            );
            tracing::subscriber::with_default(subscriber, || {
                tracing::info!(
                    kind = "command",
                    chat_type = "group",
                    direction = "incoming",
                    name = "Tester",
                    sender = "1@s.whatsapp.net",
                    chat = "1@g.us",
                    group_name = "Test Group",
                    command = "ping",
                    media = "text",
                    preview = "!ping\n\x1b[31m",
                    "Message received"
                );
                tracing::info!(
                    kind = "message",
                    chat_type = "private",
                    name = "Tester",
                    sender = "12345678@s.whatsapp.net",
                    chat = "12345678@s.whatsapp.net",
                    direction = "incoming",
                    preview = "hello",
                    "Message received"
                );
            });
            let text = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
            assert!(text.contains("[CMD]"));
            assert!(text.contains("Tester → Test Group | !ping"));
            assert!(text.contains("Tester | hello"));
            assert!(chrono::NaiveTime::parse_from_str(&text[..8], "%H:%M:%S").is_ok());
            assert_eq!(text.contains("\x1b[96m"), ansi);
            assert!(!text.contains("\x1b[31m"));
            assert_eq!(text.lines().count(), 2);
        }
    }
    #[test]
    fn console_matches_group_private_info_and_error_examples() {
        let output = Capture(Arc::new(Mutex::new(Vec::new())));
        let writer = output.clone();
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .event_format(Console)
                .with_ansi(false)
                .with_writer(move || writer.clone()),
        );
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(
                kind = "message",
                chat_type = "group",
                name = "Alice",
                sender = "6281234567890@s.whatsapp.net",
                chat = "123@g.us",
                group_name = "Rust Group",
                preview = "hello",
                "Message received"
            );
            tracing::info!(
                kind = "command",
                chat_type = "group",
                name = "Alice",
                sender = "6281234567890@s.whatsapp.net",
                chat = "123@g.us",
                group_name = "Rust Group",
                preview = "!ping",
                "Message received"
            );
            tracing::info!(
                kind = "message",
                chat_type = "private",
                name = "Alice",
                sender = "6281234567890@s.whatsapp.net",
                chat = "6281234567890@s.whatsapp.net",
                preview = "hello",
                "Message received"
            );
            tracing::info!(
                kind = "command",
                chat_type = "private",
                name = "Alice",
                sender = "6281234567890@s.whatsapp.net",
                preview = "!ping",
                "Message received"
            );
            tracing::info!("Bot ready");
            tracing::error!("Command failed");
            tracing::warn!("Retrying");
            tracing::info!(
                kind = "message",
                chat_type = "group",
                sender = "123@lid",
                chat = "123@g.us",
                media = "image",
                "Message received"
            );
            for (kind, chat_type, name, sender, chat, group_name, text) in [
                (
                    "message",
                    "group",
                    " ",
                    "6281234567890@s.whatsapp.net",
                    "123@g.us",
                    " ",
                    "hello",
                ),
                (
                    "command",
                    "group",
                    "Alice",
                    "6281234567890@s.whatsapp.net",
                    "123@g.us",
                    "",
                    "!ping",
                ),
                (
                    "message",
                    "group",
                    "",
                    "6281234567890@s.whatsapp.net",
                    "123@g.us",
                    "Rust Group",
                    "hello",
                ),
                (
                    "message",
                    "private",
                    "",
                    "6281234567890@s.whatsapp.net",
                    "6281234567890@s.whatsapp.net",
                    "",
                    "hello",
                ),
                ("command", "private", "", "", "123@lid", "", "!ping"),
                (
                    "message",
                    "group",
                    "Alice",
                    "6281234567890@s.whatsapp.net",
                    "123@g.us",
                    "Rust / 2027",
                    "hello",
                ),
            ] {
                tracing::info!(
                    kind,
                    chat_type,
                    name,
                    sender,
                    chat,
                    group_name,
                    preview = text,
                    "Message received"
                );
            }
        });
        let text = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
        let lines: Vec<_> = text
            .lines()
            .map(|line| {
                assert!(chrono::NaiveTime::parse_from_str(&line[..8], "%H:%M:%S").is_ok());
                &line[9..]
            })
            .collect();
        assert_eq!(
            lines,
            [
                "[MSG] Alice → Rust Group | hello",
                "[CMD] Alice → Rust Group | !ping",
                "[MSG] Alice | hello",
                "[CMD] Alice | !ping",
                "[INF] Bot ready",
                "[ERR] Command failed",
                "[WRN] Retrying",
                "[MSG] 123@lid → 123@g.us | [image]",
                "[MSG] 6281234567890 → 123@g.us | hello",
                "[CMD] Alice → 123@g.us | !ping",
                "[MSG] 6281234567890 → Rust Group | hello",
                "[MSG] 6281234567890 | hello",
                "[CMD] 123@lid | !ping",
                "[MSG] Alice → Rust / 2027 | hello",
            ]
        );
    }
    #[test]
    fn previews_cannot_inject_terminal_controls_and_preserve_unicode() {
        assert_eq!(preview("halo\n\x1b[31m\0\u{202e}dunia"), "halo  [31m dunia");
        assert_eq!(preview(&"🦀".repeat(301)).chars().count(), 301);
        assert!(!preview(&"🦀".repeat(301)).contains('\u{fffd}'));
    }
    #[test]
    fn command_errors_are_visible_and_sanitized_in_console_and_dashboard() {
        let output = Capture(Arc::new(Mutex::new(Vec::new())));
        let writer = output.clone();
        let logs = Arc::new(Mutex::new(VecDeque::new()));
        let subscriber = tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .event_format(Console)
                    .with_ansi(false)
                    .with_writer(move || writer.clone()),
            )
            .with(DashboardLogs(logs.clone()));
        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(
                error = "FFmpeg conversion failed\n\x1b[31m",
                "Command failed"
            );
            tracing::error!("Command timed out");
        });
        let text = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
        assert!(text.contains("[ERR] Command failed"));
        assert!(text.contains("error=FFmpeg conversion failed"));
        assert!(text.contains("[ERR] Command timed out"));
        assert!(!text.contains('\x1b'));
        assert_eq!(text.lines().count(), 2);
        let entries = logs.lock().unwrap();
        assert_eq!(entries[0].level, "ERROR");
        assert!(entries[0].message.contains("FFmpeg conversion failed"));
        assert!(!entries[0].message.contains(['\n', '\x1b']));
    }
    #[test]
    fn dashboard_retains_only_latest_200_safe_events() {
        let logs = Arc::new(Mutex::new(VecDeque::new()));
        let subscriber = tracing_subscriber::registry().with(DashboardLogs(logs.clone()));
        tracing::subscriber::with_default(subscriber, || {
            for _ in 0..205 {
                tracing::info!(
                    chat_type = "group",
                    kind = "command",
                    preview = "<script>hello</script>",
                    secret = "never-store",
                    "Message received"
                );
            }
        });
        let entries = logs.lock().unwrap();
        assert_eq!(entries.len(), 200);
        assert_eq!(entries[0].details["preview"], "<script>hello</script>");
        assert!(
            !serde_json::to_string(&*entries)
                .unwrap()
                .contains("never-store")
        );
    }
}
