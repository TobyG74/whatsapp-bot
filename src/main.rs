use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use clap::{Parser, Subcommand};
use fs2::FileExt;
use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicU64},
    time::Instant,
};
use tokio::sync::{RwLock, Semaphore};
use whatsapp_bot::{
    app::{
        config::Config,
        state::{AppState, ConnectionStatus},
    },
    bot::messaging::{self, Kind, SendRequest},
    infrastructure::{
        i18n::{self, Translator},
        logging,
        security::{Crypto, random_secret},
    },
    storage::Database,
};

#[derive(Parser)]
#[command(version, about = "Rust WhatsApp bot with an authenticated dashboard")]
struct Cli {
    #[arg(long, default_value = "config.toml", global = true)]
    config: PathBuf,
    #[command(subcommand)]
    command: Option<Action>,
}
#[derive(Subcommand)]
enum Action {
    /// Start the bot and dashboard. Offline runs the dashboard without WhatsApp.
    Run {
        #[arg(long)]
        offline: bool,
    },
    /// Print fresh dashboard token and data encryption key for .env.
    Secrets,
    /// Generate cached bot translations for all supported languages or selected codes.
    GenerateLocales {
        #[arg(
            long,
            required_unless_present = "language",
            conflicts_with = "language"
        )]
        all: bool,
        #[arg(long, value_delimiter = ',', required_unless_present = "all")]
        language: Vec<String>,
    },
    /// Queue a message using the running dashboard API.
    Send {
        #[arg(long)]
        to: String,
        #[arg(long, default_value = "")]
        text: String,
        #[arg(long)]
        media: Option<PathBuf>,
        #[arg(long, value_enum)]
        kind: Option<Kind>,
        #[arg(long)]
        mimetype: Option<String>,
        #[arg(long)]
        at: Option<chrono::DateTime<chrono::Utc>>,
        #[arg(long, default_value = "http://127.0.0.1:3000")]
        api_url: String,
    },
}

fn token() -> Result<String> {
    let value = std::env::var("DASHBOARD_TOKEN")
        .context("Set DASHBOARD_TOKEN in .env; generate it with the secrets command")?;
    ensure!(
        (32..=256).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_alphanumeric()),
        "Dashboard token must contain 32..256 ASCII letters/digits"
    );
    Ok(value)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let _ = dotenvy::dotenv();
    match cli.command.unwrap_or(Action::Run { offline: false }) {
        Action::Secrets => {
            println!(
                "DASHBOARD_TOKEN={}\nDATA_ENCRYPTION_KEY={}",
                random_secret(),
                random_secret()
            );
            Ok(())
        }
        Action::Run { offline } => run(Config::load(&cli.config)?, offline).await,
        Action::GenerateLocales { all, language } => {
            let translator = Translator::from_env()?;
            i18n::load_cache()?;
            let languages = if all {
                translator
                    .supported_languages()
                    .await?
                    .into_iter()
                    .map(|l| l.code)
                    .collect()
            } else {
                language
            };
            let mut failed = 0;
            for code in languages {
                match translator.ensure(&code).await {
                    Ok(()) => println!("Ready: {code}"),
                    Err(error) => {
                        failed += 1;
                        eprintln!("Failed: {code}: {error}");
                    }
                }
            }
            ensure!(
                failed == 0,
                "{failed} language(s) failed; successful translations were kept. Run the command again to retry"
            );
            Ok(())
        }
        Action::Send {
            to,
            text,
            media,
            kind,
            mimetype,
            at,
            api_url,
        } => {
            let config = Config::load(&cli.config)?;
            let mut request = SendRequest {
                to,
                text,
                kind: kind.unwrap_or(if media.is_some() {
                    Kind::Document
                } else {
                    Kind::Text
                }),
                data_base64: String::new(),
                file_name: String::new(),
                mimetype: String::new(),
                send_at: at,
            };
            if let Some(path) = media {
                ensure!(
                    tokio::fs::metadata(&path).await?.len() <= config.media.max_bytes as u64,
                    "File exceeds configured size limit"
                );
                let bytes = tokio::fs::read(&path).await?;
                ensure!(
                    bytes.len() <= config.media.max_bytes,
                    "File exceeds configured size limit"
                );
                request.data_base64 = STANDARD.encode(bytes);
                request.file_name = path
                    .file_name()
                    .context("Invalid file name")?
                    .to_string_lossy()
                    .into();
                request.mimetype = mimetype.unwrap_or_else(|| guess_mime(&path).into());
            }
            request.validate(config.media.max_bytes)?;
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()?;
            let response = client
                .post(format!("{}/api/messages", api_url.trim_end_matches('/')))
                .bearer_auth(token()?)
                .json(&request)
                .send()
                .await?;
            let status = response.status();
            let body = response.text().await?;
            ensure!(
                status.is_success(),
                "Dashboard rejected request ({status}): {body}"
            );
            println!("{body}");
            Ok(())
        }
    }
}

fn guess_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "ogg" | "opus" => "audio/ogg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

async fn run(config: Config, offline: bool) -> Result<()> {
    let logs = logging::init();
    logging::startup_banner();
    let dashboard_token = token()?;
    let crypto = Arc::new(Crypto::new(
        &std::env::var("DATA_ENCRYPTION_KEY").context("Set DATA_ENCRYPTION_KEY in .env")?,
    )?);
    tokio::fs::create_dir_all("data").await?;
    if let Some(parent) = Path::new(&config.connection.session_path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        tokio::fs::create_dir_all(parent).await?;
    }
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open("data/bot.lock")?;
    lock.try_lock_exclusive()
        .context("Another bot process is using this data directory")?;
    i18n::load_cache()?;
    let translations = Translator::from_env()?;
    let database = Database::connect(&config.database.url, crypto).await?;
    let settings = database.settings(&config.bot).await?;
    translations.ensure(&settings.language).await?;
    let state = Arc::new(AppState {
        config,
        settings: RwLock::new(settings),
        database,
        translations,
        token: dashboard_token,
        client: RwLock::new(None),
        connection: RwLock::new(ConnectionStatus::new(if offline {
            "offline"
        } else {
            "starting"
        })),
        logs,
        started: Instant::now(),
        received: AtomicU64::new(0),
        commands: AtomicU64::new(0),
        sent: AtomicU64::new(0),
        failures: AtomicU64::new(0),
        media_slots: Semaphore::new(2),
    });
    let listener = tokio::net::TcpListener::bind(state.config.dashboard.bind)
        .await
        .context("Cannot bind dashboard address")?;
    let dashboard_address = listener.local_addr()?;
    let bot_task = if offline {
        None
    } else {
        let bot = tokio::time::timeout(state.timeout(), messaging::build_bot(state.clone()))
            .await
            .context("WhatsApp initialization timed out")??;
        let bot_state = state.clone();
        Some(tokio::spawn(async move {
            bot.run().await;
            let mut connection = bot_state.connection.write().await;
            if !matches!(
                connection.state.as_str(),
                "logged_out" | "pairing_expired" | "client_outdated"
            ) {
                *connection = ConnectionStatus::new("stopped");
            }
            tracing::warn!("WhatsApp session stopped; dashboard remains available");
        }))
    };
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let app = whatsapp_bot::web::router(state.clone());
    let mut web_task = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
    });
    logging::dashboard_started(dashboard_address);
    let mut worker = tokio::spawn(messaging::outbox_worker(state.clone()));
    let result: Result<()> = tokio::select! {
        _ = whatsapp_rust::shutdown_signal() => Ok(()),
        result = &mut web_task => result.context("Dashboard task panicked").and_then(|r| r.context("Dashboard failed")).and_then(|_| Err(anyhow::anyhow!("Dashboard stopped unexpectedly"))),
        result = &mut worker => result.context("Outbox task panicked").and_then(|r| r.context("Outbox failed")).and_then(|_| Err(anyhow::anyhow!("Outbox stopped unexpectedly"))),
    };
    tracing::info!("Shutting down");
    worker.abort();
    let _ = shutdown_tx.send(());
    if let Some(client) = state.client.read().await.clone()
        && tokio::time::timeout(state.timeout(), client.disconnect())
            .await
            .is_err()
    {
        tracing::warn!("WhatsApp shutdown timed out");
    }
    if let Some(task) = bot_task {
        let _ = tokio::time::timeout(state.timeout(), task).await;
    }
    if !web_task.is_finished()
        && tokio::time::timeout(state.timeout(), &mut web_task)
            .await
            .is_err()
    {
        web_task.abort();
    }
    drop(lock);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_keeps_run_secrets_and_send_but_rejects_update() {
        assert!(Cli::try_parse_from(["whatsapp-bot", "generate-locales", "--all"]).is_ok());
        assert!(
            Cli::try_parse_from(["whatsapp-bot", "generate-locales", "--language", "es,fr"])
                .is_ok()
        );
        assert!(Cli::try_parse_from(["whatsapp-bot", "generate-locales"]).is_err());
        assert!(
            Cli::try_parse_from([
                "whatsapp-bot",
                "generate-locales",
                "--all",
                "--language",
                "es"
            ])
            .is_err()
        );
        assert!(Cli::try_parse_from(["whatsapp-bot", "run", "--offline"]).is_ok());
        assert!(Cli::try_parse_from(["whatsapp-bot", "secrets"]).is_ok());
        assert!(
            Cli::try_parse_from([
                "whatsapp-bot",
                "send",
                "--to",
                "6281234567890",
                "--text",
                "Hi"
            ])
            .is_ok()
        );
        assert!(Cli::try_parse_from(["whatsapp-bot", "update"]).is_err());
        assert!(Cli::try_parse_from(["whatsapp-bot", "update", "--check"]).is_err());
    }
}
