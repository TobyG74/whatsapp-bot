use crate::{
    app::state::{ConnectionStatus, Shared},
    bot::commands,
};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use diesel::RunQueryDsl;
use qrcode::{QrCode, render::svg};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    io::{Cursor, Seek, SeekFrom, Write},
    sync::atomic::Ordering,
    time::Duration,
};
use tokio::process::Command;
use whatsapp_rust::wacore_binary::JidExt;
use whatsapp_rust::{
    download::{DownloadWriter, Downloadable, MediaType},
    media::{self, AudioOptions, DocumentOptions, ImageOptions, VideoOptions},
    prelude::*,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Text,
    Image,
    Video,
    Document,
    Audio,
    Sticker,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SendRequest {
    pub to: String,
    pub kind: Kind,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub data_base64: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub mimetype: String,
    pub send_at: Option<DateTime<Utc>>,
}

impl SendRequest {
    pub fn validate(&mut self, max_bytes: usize) -> Result<()> {
        self.to = normalize_recipient(&self.to)?;
        ensure!(
            self.text.len() <= 4096,
            "Text/caption cannot exceed 4096 bytes"
        );
        if self.kind == Kind::Text {
            ensure!(!self.text.trim().is_empty(), "Text is required");
            ensure!(
                self.data_base64.is_empty(),
                "Text messages cannot contain media"
            );
        } else {
            ensure!(
                self.data_base64.len() <= max_bytes.div_ceil(3) * 4,
                "Media exceeds configured size limit"
            );
            let data = STANDARD
                .decode(&self.data_base64)
                .context("Invalid base64 media")?;
            ensure!(
                !data.is_empty() && data.len() <= max_bytes,
                "Invalid media size"
            );
            ensure!(
                !self.mimetype.is_empty()
                    && self.mimetype.len() <= 128
                    && self.mimetype.is_ascii()
                    && !self.mimetype.chars().any(char::is_control)
                    && self.mimetype.contains('/'),
                "Provide a valid media MIME type"
            );
            ensure!(
                self.file_name.len() <= 128
                    && !self.file_name.contains(['/', '\\'])
                    && !self.file_name.chars().any(char::is_control),
                "Invalid media filename"
            );
            if self.kind == Kind::Sticker {
                ensure!(
                    data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP"),
                    "Sticker must be WebP"
                );
            }
        }
        if let Some(send_at) = self.send_at {
            ensure!(
                send_at <= Utc::now() + chrono::Duration::days(366),
                "Schedule cannot exceed one year"
            );
        }
        Ok(())
    }
}

pub fn normalize_recipient(value: &str) -> Result<String> {
    let value = value.trim().trim_start_matches('+');
    if !value.contains('@') {
        ensure!(
            (7..=15).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_digit()),
            "Use an international phone number (7..15 digits)"
        );
        return Ok(format!("{value}@s.whatsapp.net"));
    }
    let (user, server) = value.split_once('@').context("Invalid recipient")?;
    match server {
        "s.whatsapp.net" => ensure!(
            (7..=15).contains(&user.len()) && user.bytes().all(|b| b.is_ascii_digit()),
            "Invalid phone number"
        ),
        "g.us" => ensure!(
            !user.is_empty()
                && user.len() <= 40
                && user.bytes().all(|b| b.is_ascii_digit() || b == b'-')
                && user.bytes().any(|b| b.is_ascii_digit()),
            "Invalid group JID"
        ),
        _ => bail!("Only phone numbers and group JIDs are supported"),
    }
    let _: Jid = value.parse().context("Invalid recipient JID")?;
    Ok(value.into())
}

pub fn command_text(message: &wa::Message) -> Option<Cow<'_, str>> {
    let base = message.get_base_message();
    if let Some(response) = base.interactive_response_message.as_option() {
        use wa::message::interactive_response_message::InteractiveResponseMessage;
        let Some(InteractiveResponseMessage::NativeFlowResponseMessage(flow)) =
            &response.interactive_response_message
        else {
            return None;
        };
        if !matches!(flow.name.as_deref(), Some("quick_reply" | "single_select")) {
            return None;
        }
        let json = flow.params_json.as_deref()?;
        if json.len() > 4096 {
            return None;
        }
        let params: serde_json::Value = serde_json::from_str(json).ok()?;
        let id = params.get("id")?.as_str()?;
        if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
            return None;
        }
        return Some(Cow::Owned(id.into()));
    }
    if let Some(response) = base.buttons_response_message.as_option() {
        return response.selected_button_id.as_deref().map(Cow::Borrowed);
    }
    if let Some(response) = base.list_response_message.as_option() {
        return response
            .single_select_reply
            .as_option()?
            .selected_row_id
            .as_deref()
            .map(Cow::Borrowed);
    }
    base.text_content()
        .or_else(|| {
            base.image_message
                .as_option()
                .and_then(|m| m.caption.as_deref())
        })
        .or_else(|| {
            base.video_message
                .as_option()
                .and_then(|m| m.caption.as_deref())
        })
        .map(Cow::Borrowed)
}

fn sticker_source(message: &wa::Message) -> Option<&wa::Message> {
    let base = message.get_base_message();
    if base.image_message.as_option().is_some() || base.video_message.as_option().is_some() {
        return Some(base);
    }
    let quoted = base
        .extended_text_message
        .as_option()?
        .context_info
        .as_option()?
        .quoted_message
        .as_option()?
        .get_base_message();
    (quoted.image_message.as_option().is_some() || quoted.video_message.as_option().is_some())
        .then_some(quoted)
}

pub fn has_sticker_source(message: &wa::Message) -> bool {
    sticker_source(message).is_some()
}

struct LimitedWriter {
    buffer: Cursor<Vec<u8>>,
    limit: usize,
}
impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.buffer.position().saturating_add(bytes.len() as u64) > self.limit as u64 {
            return Err(std::io::Error::other("Media exceeds size limit"));
        }
        self.buffer.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Seek for LimitedWriter {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.buffer.seek(position)
    }
}
impl DownloadWriter for LimitedWriter {
    fn truncate(&mut self, len: u64) -> std::io::Result<()> {
        self.buffer
            .get_mut()
            .truncate(usize::try_from(len).unwrap_or(usize::MAX));
        Ok(())
    }
}

pub async fn convert_sticker(
    ffmpeg: &str,
    bytes: &[u8],
    animated: bool,
    timeout: Duration,
) -> Result<Vec<u8>> {
    let deadline = tokio::time::Instant::now() + timeout;
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("input");
    let output = directory.path().join("sticker.webp");
    tokio::fs::write(&input, bytes).await?;
    let encodings: &[(u8, u8)] = if animated {
        &[(50, 12), (20, 8), (5, 6)]
    } else {
        &[(50, 1)]
    };
    // Reserve EXIF space for the two validated 256-byte pack/author names.
    let limit = if animated {
        500 * 1024 - 2048
    } else {
        100 * 1024
    };
    for (quality, fps) in encodings {
        let scale = "scale=512:512:force_original_aspect_ratio=decrease,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=0x00000000";
        let filter = if animated {
            format!("fps={fps},{scale}")
        } else {
            scale.into()
        };
        let mut command = Command::new(ffmpeg);
        command
            .kill_on_drop(true)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-y",
                "-protocol_whitelist",
                "file,pipe",
                "-max_alloc",
                "67108864",
                "-max_pixels",
                "16777216",
                "-threads",
                "1",
                "-f",
                if animated { "mov" } else { "image2pipe" },
                "-i",
            ])
            .arg(&input)
            .args([
                "-t",
                "6",
                "-an",
                "-map",
                "0:v:0",
                "-vf",
                &filter,
                "-c:v",
                "libwebp",
                "-threads",
                "1",
                "-q:v",
                &quality.to_string(),
                "-loop",
                "0",
            ]);
        if !animated {
            command.args(["-frames:v", "1"]);
        }
        let result = tokio::time::timeout_at(deadline, command.arg(&output).output())
            .await
            .context("Sticker conversion timed out")?
            .context("Cannot start FFmpeg; check media.ffmpeg and the FFmpeg installation")?;
        ensure!(
            result.status.success(),
            "FFmpeg conversion failed: {}",
            crate::infrastructure::logging::preview(String::from_utf8_lossy(&result.stderr).trim())
        );
        if tokio::fs::metadata(&output).await?.len() > limit as u64 {
            continue;
        }
        let bytes = tokio::fs::read(&output).await?;
        ensure!(
            bytes.get(..4) == Some(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
            "FFmpeg did not produce a valid WebP sticker"
        );
        return Ok(bytes);
    }
    bail!("Sticker exceeds size limit after compression; use a shorter/simpler source")
}

fn sticker_message(upload: whatsapp_rust::upload::UploadResponse, animated: bool) -> wa::Message {
    wa::Message {
        sticker_message: MessageField::some(wa::message::StickerMessage {
            url: Some(upload.url),
            direct_path: Some(upload.direct_path),
            media_key: Some(upload.media_key.to_vec()),
            file_sha256: Some(upload.file_sha256.to_vec()),
            file_enc_sha256: Some(upload.file_enc_sha256.to_vec()),
            file_length: Some(upload.file_length),
            media_key_timestamp: Some(upload.media_key_timestamp),
            mimetype: Some("image/webp".into()),
            width: Some(512),
            height: Some(512),
            is_animated: Some(animated),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub async fn sticker(state: &Shared, ctx: &MessageContext) -> Result<()> {
    let _permit = state.media_slots.acquire().await?;
    let source = sticker_source(&ctx.message).context("Sticker source missing")?;
    let animated = source.video_message.as_option().is_some();
    let downloadable: &dyn Downloadable = if let Some(image) = source.image_message.as_option() {
        image
    } else {
        source.video_message.as_option().context("Video missing")?
    };
    ensure!(
        downloadable
            .file_length()
            .is_none_or(|len| len <= state.config.media.max_bytes as u64),
        "Sticker source exceeds configured media size limit"
    );
    let writer = LimitedWriter {
        buffer: Cursor::new(Vec::new()),
        limit: state.config.media.max_bytes + 32,
    };
    let bytes = ctx
        .client
        .download_to_writer(downloadable, writer)
        .await
        .context("Sticker media download failed")?
        .buffer
        .into_inner();
    ensure!(
        bytes.len() <= state.config.media.max_bytes,
        "Media exceeds limit"
    );
    let sticker = convert_sticker(
        &state.config.media.ffmpeg,
        &bytes,
        animated,
        state.timeout(),
    )
    .await?;
    let sticker_config = state
        .settings
        .read()
        .await
        .sticker
        .clone()
        .unwrap_or_else(|| state.config.sticker.clone());
    let sticker = super::sticker_metadata::with_metadata(&sticker, &sticker_config, animated)?;
    let upload = ctx
        .client
        .upload(sticker, MediaType::Sticker, Default::default())
        .await
        .context("Sticker upload failed")?;
    ctx.send_message(sticker_message(upload, animated))
        .await
        .context("Sticker delivery failed")?;
    state.sent.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

pub async fn send(state: &Shared, client: &Client, request: &SendRequest) -> Result<()> {
    let jid: Jid = request.to.parse()?;
    let message = if request.kind == Kind::Text {
        wa::Message::text(&request.text)
    } else {
        let _permit = state.media_slots.acquire().await?;
        let bytes = STANDARD.decode(&request.data_base64)?;
        let media_type = match request.kind {
            Kind::Image => MediaType::Image,
            Kind::Video => MediaType::Video,
            Kind::Audio => MediaType::Audio,
            Kind::Document => MediaType::Document,
            Kind::Sticker => MediaType::Sticker,
            Kind::Text => unreachable!(),
        };
        let upload = client.upload(bytes, media_type, Default::default()).await?;
        match request.kind {
            Kind::Image => media::image_message(
                upload,
                ImageOptions {
                    caption: Some(request.text.clone()),
                    mimetype: Some(request.mimetype.clone()),
                    ..Default::default()
                },
            ),
            Kind::Video => media::video_message(
                upload,
                VideoOptions {
                    caption: Some(request.text.clone()),
                    mimetype: Some(request.mimetype.clone()),
                    ..Default::default()
                },
            ),
            Kind::Audio => media::audio_message(
                upload,
                AudioOptions {
                    mimetype: Some(request.mimetype.clone()),
                    ..Default::default()
                },
            ),
            Kind::Document => media::document_message(
                upload,
                DocumentOptions {
                    caption: Some(request.text.clone()),
                    file_name: Some(request.file_name.clone()),
                    mimetype: Some(request.mimetype.clone()),
                    ..Default::default()
                },
            ),
            Kind::Sticker => {
                sticker_message(upload, request.mimetype == "image/webp; animated=true")
            }
            Kind::Text => unreachable!(),
        }
    };
    client.send_message(&jid, message).await?;
    Ok(())
}

pub async fn outbox_worker(state: Shared) -> Result<()> {
    let mut ticks = tokio::time::interval(Duration::from_secs(1));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_maintenance = std::time::Instant::now();
    loop {
        ticks.tick().await;
        if last_maintenance.elapsed() >= Duration::from_secs(3600) {
            state.database.maintenance().await?;
            last_maintenance = std::time::Instant::now();
        }
        let Some(client) = state.connected_client().await else {
            continue;
        };
        let Some((id, request)) = state.database.claim_due::<SendRequest>().await? else {
            continue;
        };
        let result = tokio::time::timeout(state.timeout(), send(&state, &client, &request)).await;
        let code = match result {
            Ok(Ok(())) => "",
            Ok(Err(_)) => "send_failed_delivery_unknown",
            Err(_) => "timeout_delivery_unknown",
        };
        state.database.finish(&id, code.is_empty(), code).await?;
        if code.is_empty() {
            state.sent.fetch_add(1, Ordering::Relaxed);
            tracing::info!(job_id = id, "Queued message sent");
        } else {
            state.failures.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                job_id = id,
                failure_code = code,
                "Queued message failed; check delivery before retrying"
            );
        }
    }
}

fn should_auto_read(settings: &crate::app::config::Settings, from_me: bool, chat: &Jid) -> bool {
    !from_me
        && !chat.is_newsletter()
        && if chat.is_status_broadcast() {
            settings.autoreadstatus
        } else {
            settings.autoreadmessage
        }
}

pub async fn build_bot(state: Shared) -> Result<Bot> {
    let store = open_session(
        &state.config.connection.session_path,
        &std::env::var("DATA_ENCRYPTION_KEY")?,
    )
    .await?;
    let message_state = state.clone();
    let qr_state = state.clone();
    let event_state = state.clone();
    let message_logger = std::sync::Arc::new(super::message_log::MessageLogger::default());
    let event_logger = message_logger.clone();
    let bot = Bot::builder()
        .with_backend(store)
        .skip_history_sync()
        .on_qr_code(move |code, timeout| {
            let state = qr_state.clone();
            async move {
                match QrCode::new(code.as_bytes()) {
                    Ok(qr) => {
                        let svg = qr.render::<svg::Color>().min_dimensions(256, 256).build();
                        *state.connection.write().await = ConnectionStatus {
                            state: "pairing".into(),
                            qr_data_url: Some(format!(
                                "data:image/svg+xml;base64,{}",
                                STANDARD.encode(svg)
                            )),
                            qr_expires_at: Some(Utc::now().timestamp() + timeout.as_secs() as i64),
                        };
                        tracing::info!("Pairing QR ready in authenticated dashboard");
                    }
                    Err(_) => tracing::error!("Could not render pairing QR"),
                }
            }
        })
        .on_event(move |event, _client| {
            let state = event_state.clone();
            let logger = event_logger.clone();
            async move {
                if let Event::GroupUpdate(update) = event.as_ref()
                    && matches!(&update.action, whatsapp_rust::wacore::stanza::groups::GroupNotificationAction::Subject { .. }) {
                    logger.invalidate(&update.group_jid).await;
                }
                let status = match event.as_ref() {
                    Event::Connected(_) => Some("connected"),
                    Event::Disconnected(_) => Some("reconnecting"),
                    Event::LoggedOut(_) => Some("logged_out"),
                    Event::ClientOutdated(_) => Some("client_outdated"),
                    Event::PairingQrCodesExhausted(_) => Some("pairing_expired"),
                    _ => None,
                };
                if let Some(status) = status {
                    *state.connection.write().await = ConnectionStatus::new(status);
                    tracing::info!(state = status, "WhatsApp connection state changed");
                }
            }
        })
        .on_message(move |ctx| {
            let state = message_state.clone();
            let logger = message_logger.clone();
            async move {
                state.received.fetch_add(1, Ordering::Relaxed);
                if (Utc::now() - ctx.info.timestamp).num_seconds() > 300 {
                    return;
                }
                let settings = state.settings.read().await.clone();
                let command = async {
                    let result = tokio::time::timeout(
                        state.timeout(),
                        commands::handle(state.clone(), ctx.clone()),
                    )
                    .await;
                    match &result {
                        Ok(Ok(())) => return,
                        Ok(Err(error)) => tracing::error!(error = %error, "Command failed"),
                        Err(_) => tracing::error!("Command timed out"),
                    }
                    state.failures.fetch_add(1, Ordering::Relaxed);
                    let default = state.settings.read().await.language.clone();
                    let language = commands::user_language(&state, &commands::actor(&ctx), &default).await.unwrap_or(default);
                    if matches!(
                        tokio::time::timeout(
                            state.timeout(),
                            ctx.reply(rust_i18n::t!("errors.failed", locale = &language))
                        )
                        .await,
                        Ok(Ok(_))
                    ) {
                        state.sent.fetch_add(1, Ordering::Relaxed);
                    }
                };
                let read = async {
                    let source = &ctx.info.source;
                    if !should_auto_read(&settings, source.is_from_me, &source.chat) {
                        return;
                    }
                    let sender = (source.is_group || source.chat.is_status_broadcast()).then_some(&source.sender);
                    match tokio::time::timeout(state.timeout(), ctx.client.mark_as_read(&source.chat, sender, &[ctx.info.id.as_str()])).await {
                        Ok(Ok(())) => (),
                        Ok(Err(error)) => tracing::error!(error = %error, "Automatic read receipt failed"),
                        Err(_) => tracing::error!("Automatic read receipt timed out"),
                    }
                };
                tokio::join!(logger.record(&ctx, &settings), command, read);
            }
        })
        .build()
        .await?;
    *state.client.write().await = Some(bot.client());
    Ok(bot)
}

pub async fn open_session(path: &str, key: &str) -> Result<SqliteStore> {
    crate::infrastructure::security::Crypto::new(key)?;
    let key = key.to_owned();
    let config =
        whatsapp_rust::store::SqliteStoreConfig::default().with_connection_init(move |conn| {
            #[derive(diesel::QueryableByName)]
            struct CipherVersion {
                #[diesel(sql_type = diesel::sql_types::Text)]
                cipher_version: String,
            }
            let version =
                diesel::sql_query("PRAGMA cipher_version").get_result::<CipherVersion>(conn)?;
            if version.cipher_version.is_empty() {
                return Err("SQLCipher is required for session encryption".into());
            }
            diesel::sql_query(format!("PRAGMA key = \"x'{key}'\";")).execute(conn)?;
            diesel::sql_query("SELECT count(*) FROM sqlite_master").execute(conn)?;
            Ok(())
        });
    Ok(SqliteStore::with_config(path, config).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_reads_separate_messages_status_and_outgoing_events() {
        let mut settings =
            toml::from_str::<crate::app::config::Config>(include_str!("../../config.example.toml"))
                .unwrap()
                .bot;
        let private: Jid = "123456789@s.whatsapp.net".parse().unwrap();
        let group: Jid = "123456789@g.us".parse().unwrap();
        let status: Jid = "status@broadcast".parse().unwrap();
        let newsletter: Jid = "123456789@newsletter".parse().unwrap();
        assert!(!should_auto_read(&settings, false, &private));
        assert!(!should_auto_read(&settings, false, &status));
        settings.autoreadmessage = true;
        assert!(should_auto_read(&settings, false, &private));
        assert!(should_auto_read(&settings, false, &group));
        assert!(!should_auto_read(&settings, false, &status));
        settings.autoreadmessage = false;
        settings.autoreadstatus = true;
        assert!(!should_auto_read(&settings, false, &private));
        assert!(should_auto_read(&settings, false, &status));
        settings.autoreadmessage = true;
        for chat in [&private, &group, &status, &newsletter] {
            assert!(!should_auto_read(&settings, true, chat));
        }
        assert!(!should_auto_read(&settings, false, &newsletter));
    }
    #[test]
    fn validates_recipients_and_payloads() {
        assert_eq!(
            normalize_recipient("+6281234567890").unwrap(),
            "6281234567890@s.whatsapp.net"
        );
        assert!(normalize_recipient("120363012345678@g.us").is_ok());
        for bad in [
            "123",
            "123456789@lid",
            "123456789@evil",
            "123:4@s.whatsapp.net",
            "-@g.us",
        ] {
            assert!(normalize_recipient(bad).is_err());
        }
        let mut request = SendRequest {
            to: "6281234567890".into(),
            kind: Kind::Text,
            text: "hello".into(),
            data_base64: String::new(),
            file_name: String::new(),
            mimetype: String::new(),
            send_at: None,
        };
        request.validate(1024).unwrap();
        request.text.clear();
        assert!(request.validate(1024).is_err());
        request.kind = Kind::Image;
        request.mimetype = "image/png".into();
        request.data_base64 = STANDARD.encode(vec![0; 2048]);
        assert!(request.validate(1024).is_err());
    }
    #[test]
    fn download_writer_bounds_actual_bytes() {
        let mut writer = LimitedWriter {
            buffer: Cursor::new(Vec::new()),
            limit: 4,
        };
        writer.write_all(b"1234").unwrap();
        assert!(writer.write_all(b"5").is_err());
        writer.truncate(0).unwrap();
        writer.rewind().unwrap();
        writer.write_all(b"ab").unwrap();
        assert_eq!(writer.buffer.into_inner(), b"ab");
    }
    #[test]
    fn captions_are_commands() {
        let message = wa::Message {
            image_message: MessageField::some(wa::message::ImageMessage {
                caption: Some("!s".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(command_text(&message).as_deref(), Some("!s"));
        assert!(has_sticker_source(&message));
    }
    #[test]
    fn button_and_list_responses_use_action_ids_and_reject_invalid_json() {
        use wa::message::interactive_response_message::{
            InteractiveResponseMessage, NativeFlowResponseMessage,
        };
        let native = |name: &str, params: &str| wa::Message {
            interactive_response_message: MessageField::some(
                wa::message::InteractiveResponseMessage {
                    interactive_response_message: Some(
                        InteractiveResponseMessage::NativeFlowResponseMessage(Box::new(
                            NativeFlowResponseMessage {
                                name: Some(name.into()),
                                params_json: Some(params.into()),
                                version: Some(1),
                            },
                        )),
                    ),
                    ..Default::default()
                },
            ),
            ..Default::default()
        };
        for name in ["quick_reply", "single_select"] {
            let input = native(name, r#"{"id":"!!speed"}"#);
            assert_eq!(command_text(&input).as_deref(), Some("!!speed"));
        }
        for params in [
            "{",
            r#"{"id":42}"#,
            r#"{"display_text":"!self"}"#,
            r#"{"id":""}"#,
            r#"{"id":"!ping\n!self"}"#,
        ] {
            assert!(command_text(&native("quick_reply", params)).is_none());
        }
        assert!(command_text(&native("cta_url", r#"{"id":"!self"}"#)).is_none());
        assert!(command_text(&native("quick_reply", &"x".repeat(4097))).is_none());
        let legacy = wa::Message {
            buttons_response_message: MessageField::some(wa::message::ButtonsResponseMessage {
                selected_button_id: Some("!ping".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(command_text(&legacy).as_deref(), Some("!ping"));
        let list = wa::Message {
            list_response_message: MessageField::some(wa::message::ListResponseMessage {
                single_select_reply: MessageField::some(
                    wa::message::list_response_message::SingleSelectReply {
                        selected_row_id: Some("!runtime".into()),
                    },
                ),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(command_text(&list).as_deref(), Some("!runtime"));
    }
    #[tokio::test]
    #[ignore = "Requires FFmpeg with libwebp; run cargo test -- --ignored"]
    async fn converts_real_image_to_webp() {
        let ppm = b"P6\n1 1\n255\n\xff\x00\x00";
        let result = convert_sticker("ffmpeg", ppm, false, Duration::from_secs(30))
            .await
            .unwrap();
        let config = crate::app::config::StickerConfig {
            packname: "Test pack 😎".into(),
            authorname: "Test author".into(),
        };
        let result =
            super::super::sticker_metadata::with_metadata(&result, &config, false).unwrap();
        super::super::sticker_metadata::tests::assert_metadata(&result, &config);
        assert_eq!(&result[..4], b"RIFF");
        assert_eq!(&result[8..12], b"WEBP");
        let directory = tempfile::tempdir().unwrap();
        let sticker_path = directory.path().join("sticker.webp");
        tokio::fs::write(&sticker_path, &result).await.unwrap();
        let decoded = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-i"])
            .arg(&sticker_path)
            .args(["-f", "null", "-"])
            .output()
            .await
            .unwrap();
        assert!(
            decoded.status.success(),
            "{}",
            String::from_utf8_lossy(&decoded.stderr)
        );
        let path = directory.path().join("clip.mp4");
        let status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=64x64:rate=12",
                "-t",
                "0.5",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&path)
            .status()
            .await
            .unwrap();
        assert!(status.success());
        let result = convert_sticker(
            "ffmpeg",
            &tokio::fs::read(path).await.unwrap(),
            true,
            Duration::from_secs(30),
        )
        .await
        .unwrap();
        let result = super::super::sticker_metadata::with_metadata(&result, &config, true).unwrap();
        super::super::sticker_metadata::tests::assert_metadata(&result, &config);
        assert!(result.windows(4).any(|s| s == b"ANIM"));
        let invalid = convert_sticker("ffmpeg", b"not a video", true, Duration::from_secs(30))
            .await
            .unwrap_err();
        let error = invalid.to_string();
        assert!(error.starts_with("FFmpeg conversion failed: "));
        assert!(error.len() > "FFmpeg conversion failed: ".len());
        let timed_out = convert_sticker("ffmpeg", ppm, false, Duration::ZERO)
            .await
            .unwrap_err();
        assert!(timed_out.to_string().contains("timed out"));
    }
    #[tokio::test]
    #[ignore = "Requires FFmpeg with libwebp and libx264"]
    async fn converts_six_second_busy_video_within_sticker_budget() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("busy.mp4");
        let generated = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=512x512:rate=12,noise=alls=40:allf=t+u",
                "-t",
                "6",
                "-c:v",
                "libx264",
                "-crf",
                "20",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&path)
            .output()
            .await
            .unwrap();
        assert!(
            generated.status.success(),
            "{}",
            String::from_utf8_lossy(&generated.stderr)
        );
        let source = tokio::fs::read(path).await.unwrap();
        let result = convert_sticker("ffmpeg", &source, true, Duration::from_secs(30))
            .await
            .unwrap();
        let config = crate::app::config::StickerConfig {
            packname: "\\".repeat(256),
            authorname: "\"".repeat(256),
        };
        let result = super::super::sticker_metadata::with_metadata(&result, &config, true).unwrap();
        super::super::sticker_metadata::tests::assert_metadata(&result, &config);
        assert!(result.len() <= 500 * 1024);
        assert!(result.windows(4).any(|s| s == b"ANIM"));
    }
    #[tokio::test]
    async fn session_database_is_encrypted_on_disk() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.db");
        let store = open_session(path.to_str().unwrap(), &"11".repeat(32))
            .await
            .unwrap();
        assert!(
            !tokio::fs::read(&path)
                .await
                .unwrap()
                .starts_with(b"SQLite format 3")
        );
        drop(store);
        use diesel::Connection;
        let mut conn = diesel::SqliteConnection::establish(path.to_str().unwrap()).unwrap();
        assert!(
            diesel::sql_query("SELECT count(*) FROM sqlite_master")
                .execute(&mut conn)
                .is_err()
        );
    }
}
