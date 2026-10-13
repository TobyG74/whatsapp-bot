use crate::{
    app::{
        config::{CommandSettings, Settings, StickerConfig},
        state::Shared,
    },
    bot::{
        commands::{COMMANDS, Command},
        messaging::SendRequest,
    },
    infrastructure::security::token_matches,
};
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Path, State},
    http::{Request, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post},
};
use chrono::Utc;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;

type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;
fn error(status: StatusCode, message: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({"error": message})))
}
fn internal() -> (StatusCode, Json<Value>) {
    tracing::error!("Dashboard database operation failed");
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Database operation failed",
    )
}

async fn authenticate(State(state): State<Shared>, request: Request<Body>, next: Next) -> Response {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .unwrap_or("");
    if !token_matches(&state.token, token) {
        return error(StatusCode::UNAUTHORIZED, "Invalid dashboard token").into_response();
    }
    next.run(request).await
}

async fn headers(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let h = response.headers_mut();
    h.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    h.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    h.insert(header::CONTENT_SECURITY_POLICY, "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'".parse().unwrap());
    h.insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
    response
}

pub fn router(state: Shared) -> Router {
    let limit = state.config.media.max_bytes.div_ceil(3) * 4 + 16 * 1024;
    let api = Router::new()
        .route("/status", get(status))
        .route("/commands", get(commands))
        .route(
            "/commands/{id}",
            axum::routing::put(save_command).delete(delete_command),
        )
        .route("/sticker", get(sticker_settings).put(save_sticker))
        .route("/logs", get(logs))
        .route("/settings", get(settings).put(save_settings))
        .route("/languages", get(languages))
        .route("/messages", post(enqueue).get(jobs))
        .route("/messages/{id}", delete(cancel))
        .route("/reconnect", post(reconnect))
        .route_layer(middleware::from_fn_with_state(state.clone(), authenticate));
    Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("assets/index.html")) }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/javascript")],
                    include_str!("assets/app.js"),
                )
            }),
        )
        .route(
            "/style.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css")],
                    include_str!("assets/style.css"),
                )
            }),
        )
        .route("/health", get(|| async { Json(json!({"status":"ok"})) }))
        .nest("/api", api)
        .layer(DefaultBodyLimit::max(limit))
        .layer(middleware::from_fn(headers))
        .with_state(state)
}

async fn status(State(state): State<Shared>) -> ApiResult {
    let mut connection = state.connection.read().await.clone();
    if connection
        .qr_expires_at
        .is_some_and(|at| at <= Utc::now().timestamp())
    {
        connection.qr_data_url = None;
    }
    let (contacts, groups) = state.database.peer_counts().await.map_err(|_| internal())?;
    Ok(Json(json!({
        "version":env!("CARGO_PKG_VERSION"), "connection":connection,
        "uptime_secs":state.started.elapsed().as_secs(), "received":state.received.load(Ordering::Relaxed),
        "commands":state.commands.load(Ordering::Relaxed), "sent":state.sent.load(Ordering::Relaxed),
        "failures":state.failures.load(Ordering::Relaxed), "contacts":contacts, "groups":groups,
        "database_driver":state.config.database.driver, "media_max_bytes":state.config.media.max_bytes,
    })))
}

async fn settings(State(state): State<Shared>) -> Json<Settings> {
    Json(state.settings.read().await.clone())
}
async fn languages(State(state): State<Shared>) -> ApiResult {
    crate::infrastructure::i18n::load_cache().map_err(|_| internal())?;
    let (languages, warning) = state.translations.languages().await;
    Ok(Json(json!({"languages":languages,"warning":warning})))
}
async fn save_change(state: &Shared, change: impl FnOnce(&mut Settings)) -> ApiResult {
    let mut guard = state.settings.write().await;
    let mut next = guard.clone();
    change(&mut next);
    next.validate()
        .map_err(|e| error(StatusCode::BAD_REQUEST, &e.to_string()))?;
    state
        .database
        .save_settings(&next)
        .await
        .map_err(|_| internal())?;
    *guard = next;
    tracing::info!("Dashboard settings saved");
    Ok(Json(json!({"saved":true})))
}

async fn commands(State(state): State<Shared>) -> ApiResult {
    let settings = state.settings.read().await;
    let mut list = Vec::new();
    for command in COMMANDS {
        let options = command.configured(&settings);
        if options.deleted {
            continue;
        }
        let mut value = serde_json::to_value(&options).map_err(|_| internal())?;
        value["id"] = json!(command.name);
        value["description_override"] = json!(options.description);
        value["description"] = json!(command.description(&options, "en"));
        value["owner_access_required"] = json!(command.owner_only);
        list.push(value);
    }
    Ok(Json(json!(list)))
}

fn command_by_id(id: &str) -> Result<&'static Command, (StatusCode, Json<Value>)> {
    COMMANDS
        .iter()
        .find(|c| c.name == id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "Command not found"))
}

async fn save_command(
    State(state): State<Shared>,
    Path(id): Path<String>,
    Json(options): Json<CommandSettings>,
) -> ApiResult {
    command_by_id(&id)?;
    save_change(&state, |settings| {
        settings.command_overrides.insert(id, options);
    })
    .await
}

async fn delete_command(State(state): State<Shared>, Path(id): Path<String>) -> ApiResult {
    let command = command_by_id(&id)?;
    save_change(&state, |settings| {
        let mut options = command.configured(settings);
        options.deleted = true;
        settings.command_overrides.insert(id, options);
    })
    .await
}

async fn sticker_settings(State(state): State<Shared>) -> Json<StickerConfig> {
    Json(
        state
            .settings
            .read()
            .await
            .sticker
            .clone()
            .unwrap_or_else(|| state.config.sticker.clone()),
    )
}

async fn save_sticker(
    State(state): State<Shared>,
    Json(sticker): Json<StickerConfig>,
) -> ApiResult {
    save_change(&state, |settings| settings.sticker = Some(sticker)).await
}
async fn save_settings(State(state): State<Shared>, Json(payload): Json<Value>) -> ApiResult {
    let mut settings: Settings = serde_json::from_value(payload.clone())
        .map_err(|e| error(StatusCode::BAD_REQUEST, &e.to_string()))?;
    settings
        .validate()
        .map_err(|e| error(StatusCode::BAD_REQUEST, &e.to_string()))?;
    state
        .translations
        .ensure(&settings.language)
        .await
        .map_err(|e| error(StatusCode::BAD_GATEWAY, &e.to_string()))?;
    save_change(&state, |s| {
        if payload.get("autocorrect").is_none() {
            settings.autocorrect = s.autocorrect;
        }
        if payload.get("use_limit").is_none() {
            settings.use_limit = s.use_limit;
        }
        if payload.get("use_cooldown").is_none() {
            settings.use_cooldown = s.use_cooldown;
        }
        if payload.get("autoreadmessage").is_none() {
            settings.autoreadmessage = s.autoreadmessage;
        }
        if payload.get("autoreadstatus").is_none() {
            settings.autoreadstatus = s.autoreadstatus;
        }
        if payload.get("cooldown_secs").is_none() {
            settings.cooldown_secs = s.cooldown_secs;
        }
        let overrides = std::mem::take(&mut s.command_overrides);
        let sticker = s.sticker.take();
        *s = settings;
        s.command_overrides = overrides;
        s.sticker = sticker;
    })
    .await
}
async fn logs(State(state): State<Shared>) -> ApiResult {
    let logs = state.logs.lock().map_err(|_| internal())?;
    Ok(Json(serde_json::to_value(&*logs).map_err(|_| internal())?))
}
async fn enqueue(State(state): State<Shared>, Json(mut request): Json<SendRequest>) -> ApiResult {
    request
        .validate(state.config.media.max_bytes)
        .map_err(|e| error(StatusCode::BAD_REQUEST, &e.to_string()))?;
    let at = request
        .send_at
        .map(|t| t.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp());
    let id = state
        .database
        .enqueue(&request, at)
        .await
        .map_err(|_| internal())?;
    tracing::info!(job_id = %id, "Message queued");
    Ok(Json(json!({"id":id,"status":"pending","send_at":at})))
}
async fn jobs(State(state): State<Shared>) -> ApiResult {
    Ok(Json(
        serde_json::to_value(state.database.jobs().await.map_err(|_| internal())?)
            .map_err(|_| internal())?,
    ))
}
async fn cancel(State(state): State<Shared>, Path(id): Path<String>) -> ApiResult {
    if !state.database.cancel(&id).await.map_err(|_| internal())? {
        return Err(error(
            StatusCode::CONFLICT,
            "Only pending jobs can be cancelled",
        ));
    }
    Ok(Json(json!({"cancelled":true})))
}
async fn reconnect(State(state): State<Shared>) -> ApiResult {
    let client = state.client.read().await.clone().ok_or_else(|| {
        error(
            StatusCode::CONFLICT,
            "WhatsApp is offline; restart without --offline",
        )
    })?;
    let status = state.connection.read().await.state.clone();
    if matches!(
        status.as_str(),
        "logged_out" | "stopped" | "pairing_expired" | "client_outdated"
    ) {
        return Err(error(
            StatusCode::CONFLICT,
            "Restart the bot to pair again; update the library if the client is outdated",
        ));
    }
    tokio::time::timeout(state.timeout(), client.reconnect())
        .await
        .map_err(|_| error(StatusCode::GATEWAY_TIMEOUT, "Reconnect timed out"))?;
    Ok(Json(json!({"reconnecting":true})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::state::{AppState, ConnectionStatus},
        infrastructure::security::Crypto,
        storage::Database,
    };
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex, atomic::AtomicU64},
        time::Instant,
    };
    use tokio::sync::{RwLock, Semaphore};
    use tower::ServiceExt;
    async fn state() -> Shared {
        let config: crate::app::config::Config =
            toml::from_str(include_str!("../../config.example.toml")).unwrap();
        let database = Database::connect(
            "sqlite::memory:",
            Arc::new(Crypto::new(&"11".repeat(32)).unwrap()),
        )
        .await
        .unwrap();
        let settings = database.settings(&config.bot).await.unwrap();
        Arc::new(AppState {
            config,
            settings: RwLock::new(settings),
            database,
            translations: crate::infrastructure::i18n::Translator::new(None, None).unwrap(),
            token: "test-token".into(),
            client: RwLock::new(None),
            connection: RwLock::new(ConnectionStatus::new("offline")),
            logs: Arc::new(Mutex::new(VecDeque::new())),
            started: Instant::now(),
            received: AtomicU64::new(0),
            commands: AtomicU64::new(0),
            sent: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            media_slots: Semaphore::new(2),
        })
    }
    async fn app() -> Router {
        router(state().await)
    }
    #[tokio::test]
    async fn personal_languages_do_not_change_the_bot_default_or_other_users() {
        let state = state().await;
        let default = state.settings.read().await.language.clone();
        state
            .database
            .save_user_language("alice", "en")
            .await
            .unwrap();
        state
            .database
            .save_user_language("bob", "id")
            .await
            .unwrap();
        assert_eq!(
            crate::bot::commands::user_language(&state, "alice", &default)
                .await
                .unwrap(),
            "en"
        );
        assert_eq!(
            crate::bot::commands::user_language(&state, "bob", &default)
                .await
                .unwrap(),
            "id"
        );
        assert_eq!(
            crate::bot::commands::user_language(&state, "carol", &default)
                .await
                .unwrap(),
            default
        );
        assert_eq!(state.settings.read().await.language, default);
        state
            .database
            .save_user_language("carol", "qv")
            .await
            .unwrap();
        assert_eq!(
            crate::bot::commands::user_language(&state, "carol", &default)
                .await
                .unwrap(),
            default
        );
        assert_eq!(
            state
                .database
                .user_language("carol")
                .await
                .unwrap()
                .as_deref(),
            Some("qv")
        );
        assert!(
            state
                .database
                .save_user_language("alice", "../bad")
                .await
                .is_err()
        );
        assert_eq!(
            state
                .database
                .user_language("alice")
                .await
                .unwrap()
                .as_deref(),
            Some("en")
        );
    }

    #[tokio::test]
    async fn dashboard_saves_preserve_feature_switches_not_in_the_form() {
        let state = state().await;
        state
            .change_settings(|s| {
                s.autocorrect = false;
                s.use_limit = false;
                s.use_cooldown = false;
                s.autoreadmessage = true;
                s.autoreadstatus = true;
                s.cooldown_secs = Some(12);
            })
            .await
            .unwrap();
        let app = router(state.clone());
        let payload = json!({"name":"Bot","owners":["6281234567890"],"prefix":"#","language":"en","mode":"public","daily_limit":42});
        assert_eq!(
            call(&app, "PUT", "/api/settings", payload).await.0,
            StatusCode::OK
        );
        let saved = state.database.settings(&state.config.bot).await.unwrap();
        assert!(!saved.autocorrect && !saved.use_limit && !saved.use_cooldown);
        assert!(saved.autoreadmessage && saved.autoreadstatus);
        assert_eq!(saved.cooldown_secs, Some(12));
        assert_eq!(saved.prefix, "#");
        let mut payload = serde_json::to_value(saved).unwrap();
        payload["autocorrect"] = json!(true);
        payload["cooldown_secs"] = Value::Null;
        assert_eq!(
            call(&app, "PUT", "/api/settings", payload).await.0,
            StatusCode::OK
        );
        assert!(state.settings.read().await.autocorrect);
        assert_eq!(state.settings.read().await.cooldown_secs, None);
    }

    async fn call(app: &Router, method: &str, path: &str, payload: Value) -> (StatusCode, Value) {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(if payload.is_null() {
                        Body::empty()
                    } else {
                        Body::from(payload.to_string())
                    })
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 65536)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    #[tokio::test]
    async fn command_and_sticker_edits_persist_and_bot_settings_do_not_reset_them() {
        let state = state().await;
        let app = router(state.clone());
        let (status, commands) = call(&app, "GET", "/api/commands", Value::Null).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(commands.as_array().unwrap().len(), COMMANDS.len());
        let mut ping = COMMANDS[0].defaults();
        ping.name = "speed".into();
        ping.aliases = vec!["fast".into()];
        ping.description = Some("My speed command".into());
        let payload = serde_json::to_value(&ping).unwrap();
        assert_eq!(
            call(&app, "PUT", "/api/commands/ping", payload.clone())
                .await
                .0,
            StatusCode::OK
        );
        let sticker = json!({"packname":"Pack 😎","authorname":"名前"});
        assert_eq!(
            call(&app, "PUT", "/api/sticker", sticker.clone()).await.0,
            StatusCode::OK
        );
        let mut bot = serde_json::to_value(&state.config.bot).unwrap();
        bot["prefix"] = json!("#");
        assert_eq!(
            call(&app, "PUT", "/api/settings", bot).await.0,
            StatusCode::OK
        );
        let saved = state.database.settings(&state.config.bot).await.unwrap();
        assert_eq!(saved.prefix, "#");
        assert_eq!(saved.command_overrides["ping"].name, "speed");
        assert_eq!(saved.sticker.as_ref().unwrap().packname, "Pack 😎");
        assert!(crate::bot::commands::parse("#fast", &saved).is_some());
        assert_eq!(
            call(&app, "GET", "/api/sticker", Value::Null).await.1,
            sticker
        );
        let commands = call(&app, "GET", "/api/commands", Value::Null).await.1;
        assert_eq!(commands[0]["name"], "speed");
        assert_eq!(commands[0]["description"], "My speed command");
        assert_eq!(
            call(&app, "DELETE", "/api/commands/ping", Value::Null)
                .await
                .0,
            StatusCode::OK
        );
        let saved = state.database.settings(&state.config.bot).await.unwrap();
        assert!(saved.command_overrides["ping"].deleted);
        assert!(crate::bot::commands::parse("#fast", &saved).is_none());
        assert_eq!(
            call(&app, "GET", "/api/commands", Value::Null)
                .await
                .1
                .as_array()
                .unwrap()
                .len(),
            COMMANDS.len() - 1
        );
        assert_eq!(
            call(&app, "PUT", "/api/commands/ping", payload).await.0,
            StatusCode::OK
        );
        assert!(crate::bot::commands::parse("#speed", &*state.settings.read().await).is_some());
    }

    #[tokio::test]
    async fn management_rejects_invalid_edits_without_changing_settings() {
        let state = state().await;
        let app = router(state.clone());
        let before = serde_json::to_value(state.settings.read().await.clone()).unwrap();
        let mut ping = COMMANDS[0].defaults();
        ping.name = "menu".into();
        assert_eq!(
            call(
                &app,
                "PUT",
                "/api/commands/ping",
                serde_json::to_value(ping).unwrap()
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        let mut owner = COMMANDS
            .iter()
            .find(|c| c.name == "setprefix")
            .unwrap()
            .defaults();
        owner.owner_only = false;
        assert_eq!(
            call(
                &app,
                "PUT",
                "/api/commands/setprefix",
                serde_json::to_value(owner).unwrap()
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        for pack in ["x".repeat(257), "bad\nname".into()] {
            assert_eq!(
                call(
                    &app,
                    "PUT",
                    "/api/sticker",
                    json!({"packname":pack,"authorname":"Tester"})
                )
                .await
                .0,
                StatusCode::BAD_REQUEST
            );
        }
        assert_eq!(
            call(&app, "DELETE", "/api/commands/unknown", Value::Null)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            serde_json::to_value(state.settings.read().await.clone()).unwrap(),
            before
        );
    }
    #[tokio::test]
    async fn selected_languages_generate_before_saving_and_failures_keep_settings() {
        let (url, calls, server) = crate::infrastructure::i18n::tests::mock("qw", false).await;
        let dir = tempfile::tempdir().unwrap();
        let mut state = state().await;
        Arc::get_mut(&mut state).unwrap().translations =
            crate::infrastructure::i18n::Translator::new(Some(&url), None)
                .unwrap()
                .with_cache(dir.path());
        let app = router(state.clone());
        let languages = call(&app, "GET", "/api/languages", Value::Null).await;
        assert_eq!(languages.0, StatusCode::OK);
        assert!(
            languages.1["languages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|l| l["code"] == "qw")
        );
        let mut payload = serde_json::to_value(state.settings.read().await.clone()).unwrap();
        payload["language"] = json!("qw");
        assert_eq!(
            call(&app, "PUT", "/api/settings", payload.clone()).await.0,
            StatusCode::OK
        );
        assert_eq!(
            state
                .database
                .settings(&state.config.bot)
                .await
                .unwrap()
                .language,
            "qw"
        );
        assert!(calls.load(Ordering::Relaxed) > 0);
        assert!(dir.path().join("qw.json").exists());
        server.abort();
        assert_eq!(
            call(&app, "PUT", "/api/settings", payload.clone()).await.0,
            StatusCode::OK
        );
        payload["language"] = json!("qt");
        assert_eq!(
            call(&app, "PUT", "/api/settings", payload).await.0,
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(state.settings.read().await.language, "qw");
        assert_eq!(
            state
                .database
                .settings(&state.config.bot)
                .await
                .unwrap()
                .language,
            "qw"
        );
    }
    #[tokio::test]
    async fn protects_all_sensitive_endpoints_and_ignores_query_tokens() {
        let app = app().await;
        for path in [
            "/api/status",
            "/api/settings",
            "/api/logs",
            "/api/commands",
            "/api/sticker",
            "/api/languages",
            "/api/messages",
            "/api/status?token=test-token",
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        }
        for (method, path) in [
            ("PUT", "/api/commands/ping"),
            ("DELETE", "/api/commands/ping"),
            ("PUT", "/api/sticker"),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
    #[tokio::test]
    async fn rejects_invalid_settings_and_messages() {
        let app = app().await;
        for (method, path, payload) in [
            (
                "POST",
                "/api/messages",
                json!({"to":"123@evil","kind":"text","text":"hello"}),
            ),
            (
                "PUT",
                "/api/settings",
                json!({"name":"Bot","owners":[],"prefix":"!","language":"id","mode":"public","daily_limit":30}),
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .header(header::AUTHORIZATION, "Bearer test-token")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(payload.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/reconnect")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
}
