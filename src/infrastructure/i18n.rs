use anyhow::{Context, Result, ensure};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    sync::{LazyLock, RwLock},
    time::Duration,
};
use tokio::sync::Mutex;

const CACHE_DIR: &str = "data/i18n";
static SOURCE: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../locales/en.json")).expect("Valid English catalog")
});
static CATALOGS: RwLock<BTreeMap<&'static str, &'static Catalog>> = RwLock::new(BTreeMap::new());

#[derive(Deserialize, Serialize)]
struct Catalog {
    code: String,
    name: String,
    source_hash: String,
    messages: BTreeMap<String, String>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Language {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub ready: bool,
    #[serde(default, skip_serializing)]
    targets: Vec<String>,
}

pub fn valid_code(code: &str) -> bool {
    (2..=16).contains(&code.len())
        && code
            .split('-')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric()))
        && code.as_bytes()[0].is_ascii_alphabetic()
}

pub fn ready(code: &str) -> bool {
    matches!(code, "en" | "id") || CATALOGS.read().is_ok_and(|m| m.contains_key(code))
}

fn source_hash() -> String {
    hex::encode(Sha256::digest(include_bytes!("../../locales/en.json")))
}

fn placeholders(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("%{") {
        let Some(end) = rest[start..].find('}') else {
            break;
        };
        result.push(&rest[start..start + end + 1]);
        rest = &rest[start + end + 1..];
    }
    result
}

fn install(catalog: Catalog) -> Result<()> {
    ensure!(
        valid_code(&catalog.code) && !matches!(catalog.code.as_str(), "id" | "en"),
        "Invalid generated locale"
    );
    ensure!(
        !catalog.name.trim().is_empty()
            && catalog.name.len() <= 80
            && !catalog.name.chars().any(char::is_control),
        "Invalid language name"
    );
    ensure!(
        catalog.source_hash == source_hash() && catalog.messages.len() == SOURCE.len(),
        "Outdated translation catalog; generate it again"
    );
    validate_catalog(&catalog)?;
    let mut catalogs = CATALOGS
        .write()
        .map_err(|_| anyhow::anyhow!("Translation cache unavailable"))?;
    if !catalogs.contains_key(catalog.code.as_str()) {
        ensure!(catalogs.len() < 256, "Too many generated languages");
        // rust-i18n 3 borrows strings; each validated locale is installed once per process.
        let catalog = Box::leak(Box::new(catalog));
        catalogs.insert(catalog.code.as_str(), catalog);
    }
    Ok(())
}

pub fn load_cache() -> Result<()> {
    load_cache_from(Path::new(CACHE_DIR))
}

fn load_cache_from(path: &Path) -> Result<()> {
    let dir = match std::fs::read_dir(path) {
        Ok(dir) => dir,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).context("Cannot read translation cache"),
    };
    for entry in dir {
        let entry = entry?;
        if entry.path().extension().is_some_and(|ext| ext == "json")
            && entry.metadata()?.len() <= 256 * 1024
        {
            let result = std::fs::read(entry.path())
                .map_err(anyhow::Error::from)
                .and_then(|bytes| Ok(serde_json::from_slice::<Catalog>(&bytes)?))
                .and_then(install);
            if result.is_err() {
                tracing::warn!("Skipping invalid or outdated translation catalog");
            }
        }
    }
    Ok(())
}

pub struct RuntimeBackend;
impl rust_i18n::Backend for RuntimeBackend {
    fn available_locales(&self) -> Vec<&str> {
        CATALOGS
            .read()
            .map(|m| m.keys().copied().collect())
            .unwrap_or_default()
    }
    fn translate(&self, locale: &str, key: &str) -> Option<&str> {
        let catalog = *CATALOGS.read().ok()?.get(locale)?;
        catalog.messages.get(key).map(String::as_str)
    }
}

pub struct Translator {
    client: Client,
    url: Option<Url>,
    api_key: Option<String>,
    generating: Mutex<()>,
    cache_dir: PathBuf,
}

impl Translator {
    pub fn from_env() -> Result<Self> {
        Self::new(
            std::env::var("LIBRETRANSLATE_URL").ok().as_deref(),
            std::env::var("LIBRETRANSLATE_API_KEY").ok(),
        )
    }

    pub fn new(url: Option<&str>, api_key: Option<String>) -> Result<Self> {
        let url = url
            .filter(|s| !s.trim().is_empty())
            .map(|s| Url::parse(&format!("{}/", s.trim_end_matches('/'))))
            .transpose()
            .context("Invalid LIBRETRANSLATE_URL")?;
        if let Some(url) = &url {
            ensure!(
                matches!(url.scheme(), "http" | "https")
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none(),
                "LIBRETRANSLATE_URL must be an HTTP(S) base URL without credentials or query parameters"
            );
        }
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(20))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            url,
            api_key,
            generating: Mutex::new(()),
            cache_dir: CACHE_DIR.into(),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_cache(mut self, path: &Path) -> Self {
        self.cache_dir = path.into();
        self
    }

    pub async fn supported_languages(&self) -> Result<Vec<Language>> {
        let url = self
            .url
            .as_ref()
            .context("Set LIBRETRANSLATE_URL to enable automatic translation")?
            .join("languages")?;
        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Translation service unavailable"))?;
        ensure!(
            response.status().is_success(),
            "Translation service rejected language lookup ({})",
            response.status()
        );
        let languages: Vec<Language> = response_json(response).await?;
        ensure!(
            languages.len() <= 256
                && languages.iter().all(|l| valid_code(&l.code)
                    && !l.name.trim().is_empty()
                    && l.name.len() <= 80
                    && !l.name.chars().any(char::is_control)),
            "Invalid translation language list"
        );
        let english = languages
            .iter()
            .find(|l| l.code == "en")
            .context("Translation service must support English")?;
        let targets = english.targets.clone();
        Ok(languages
            .into_iter()
            .filter(|l| l.code == "en" || targets.contains(&l.code))
            .collect())
    }

    pub async fn languages(&self) -> (Vec<Language>, Option<String>) {
        let mut languages = BTreeMap::from([
            (
                "en".into(),
                Language {
                    code: "en".into(),
                    name: "English".into(),
                    ready: true,
                    targets: vec![],
                },
            ),
            (
                "id".into(),
                Language {
                    code: "id".into(),
                    name: "Indonesian".into(),
                    ready: true,
                    targets: vec![],
                },
            ),
        ]);
        if let Ok(catalogs) = CATALOGS.read() {
            for catalog in catalogs.values() {
                languages.insert(
                    catalog.code.clone(),
                    Language {
                        code: catalog.code.clone(),
                        name: catalog.name.clone(),
                        ready: true,
                        targets: vec![],
                    },
                );
            }
        }
        let mut warning = None;
        if self.url.is_some() {
            match self.supported_languages().await {
                Ok(remote) => {
                    for mut language in remote {
                        language.ready = ready(&language.code);
                        languages.entry(language.code.clone()).or_insert(language);
                    }
                }
                Err(_) => {
                    warning = Some(
                        "Translation service unavailable. Cached languages remain usable.".into(),
                    )
                }
            }
        }
        (languages.into_values().collect(), warning)
    }

    pub async fn ensure(&self, code: &str) -> Result<()> {
        ensure!(valid_code(code), "Invalid language code");
        if ready(code) {
            return Ok(());
        }
        tokio::time::timeout(Duration::from_secs(60), async {
            let _guard = self.generating.lock().await;
            if ready(code) {
                return Ok(());
            }
            load_cache_from(&self.cache_dir)?;
            if ready(code) {
                return Ok(());
            }
            let language = self
                .supported_languages()
                .await?
                .into_iter()
                .find(|l| l.code == code)
                .context("Language is not supported by the translation service")?;
            // Placeholders never leave the bot; translate only the literal text around them.
            let mut segments = Vec::new();
            for (key, source) in SOURCE.iter() {
                let mut rest = source.as_str();
                while let Some(start) = rest.find("%{") {
                    segments.push((key, &rest[..start]));
                    let end = rest[start..]
                        .find('}')
                        .context("Invalid source placeholder")?
                        + start
                        + 1;
                    segments.push((key, &rest[start..end]));
                    rest = &rest[end..];
                }
                segments.push((key, rest));
            }
            let texts: Vec<&str> = segments
                .iter()
                .map(|(_, text)| text.trim())
                .filter(|s| !s.is_empty() && !s.starts_with("%{"))
                .collect();
            let mut results = Vec::new();
            for batch in texts.chunks(8) {
                results.extend(self.translate_batch(batch, code).await?);
            }
            let mut results = results.into_iter();
            let mut messages: BTreeMap<String, String> = BTreeMap::new();
            for (key, text) in segments {
                let message = messages.entry(key.clone()).or_default();
                if text.trim().is_empty() || text.starts_with("%{") {
                    message.push_str(text);
                } else {
                    let leading = &text[..text.len() - text.trim_start().len()];
                    let trailing = &text[text.trim_end().len()..];
                    message.push_str(leading);
                    message.push_str(
                        results
                            .next()
                            .context("Incomplete translation response")?
                            .trim(),
                    );
                    message.push_str(trailing);
                }
            }
            let catalog = Catalog {
                code: code.into(),
                name: language.name,
                source_hash: source_hash(),
                messages,
            };
            validate_catalog(&catalog)?;
            let bytes = serde_json::to_vec_pretty(&catalog)?;
            ensure!(
                bytes.len() <= 256 * 1024,
                "Translation catalog is too large"
            );
            std::fs::create_dir_all(&self.cache_dir).context("Cannot create translation cache")?;
            let mut file = tempfile::NamedTempFile::new_in(&self.cache_dir)?;
            file.write_all(&bytes)?;
            file.as_file().sync_all()?;
            file.persist(self.cache_dir.join(format!("{code}.json")))
                .context("Cannot save translation cache")?;
            install(catalog)?;
            tracing::info!(language = code, "Language generated and cached");
            Ok(())
        })
        .await
        .context("Language generation timed out; existing settings were kept")?
    }

    async fn translate_batch(&self, texts: &[&str], target: &str) -> Result<Vec<String>> {
        let mut body = serde_json::json!({"q":texts,"source":"en","target":target,"format":"text"});
        if let Some(key) = &self.api_key {
            body["api_key"] = serde_json::json!(key)
        }
        let response = self
            .client
            .post(
                self.url
                    .as_ref()
                    .context("Translation service not configured")?
                    .join("translate")?,
            )
            .json(&body)
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Translation service unavailable"))?;
        ensure!(
            response.status().is_success(),
            "Translation service rejected generation ({})",
            response.status()
        );
        let body: serde_json::Value = response_json(response).await?;
        let translated: Vec<String> = serde_json::from_value(body["translatedText"].clone())
            .context("Missing translated text array")?;
        ensure!(
            translated.len() == texts.len()
                && translated
                    .iter()
                    .all(|s| !s.trim().is_empty() && s.len() <= 8192),
            "Invalid translated text batch"
        );
        Ok(translated)
    }
}

async fn response_json<T: serde::de::DeserializeOwned>(
    mut response: reqwest::Response,
) -> Result<T> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| anyhow::anyhow!("Cannot read translation response"))?
    {
        ensure!(
            bytes.len() + chunk.len() <= 1024 * 1024,
            "Translation response is too large"
        );
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).context("Invalid translation response")
}

fn validate_catalog(catalog: &Catalog) -> Result<()> {
    ensure!(
        catalog.messages.len() == SOURCE.len(),
        "Incomplete translation catalog"
    );
    for (key, source) in SOURCE.iter() {
        let text = catalog
            .messages
            .get(key)
            .context("Missing translated template")?;
        ensure!(
            !text.trim().is_empty()
                && text.len() <= 8192
                && !text.chars().any(|c| c.is_control() && c != '\n')
                && placeholders(source) == placeholders(text),
            "Invalid translated template"
        );
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use axum::{
        Json, Router,
        routing::{get, post},
    };
    use serde_json::{Value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    pub(crate) async fn mock(
        code: &str,
        broken: bool,
    ) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
        let code = code.to_owned();
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let app = Router::new()
            .route(
                "/languages",
                get(move || {
                    let code = code.clone();
                    async move {
                        Json(json!([
                            {"code":"en","name":"English","targets":["en",code]},
                            {"code":code,"name":"Mock language","targets":["en"]},
                            {"code":"zz","name":"Unsupported from English","targets":[]}
                        ]))
                    }
                }),
            )
            .route(
                "/translate",
                post(move |Json(body): Json<Value>| {
                    let counter = counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::Relaxed);
                        assert_eq!(body["source"], "en");
                        assert_eq!(body["format"], "text");
                        let texts = body["q"].as_array().unwrap();
                        assert!(texts.len() <= 8);
                        assert!(texts.iter().all(|s| !s.as_str().unwrap().contains("%{")));
                        let results: Vec<String> = texts
                            .iter()
                            .map(|s| {
                                if broken {
                                    "%{unexpected}".into()
                                } else {
                                    format!("Translated: {}", s.as_str().unwrap())
                                }
                            })
                            .collect();
                        Json(json!({"translatedText":results}))
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{address}"), calls, handle)
    }

    #[test]
    fn validates_codes_catalogs_and_service_urls() {
        for code in ["fr", "zh-Hans", "pt-BR"] {
            assert!(valid_code(code));
        }
        for code in ["", "../en", "en/xx", "fr?x", "en--US", "-fr", "123"] {
            assert!(!valid_code(code));
        }
        assert!(Translator::new(Some("file:///tmp"), None).is_err());
        assert!(Translator::new(Some("https://user:pass@host"), None).is_err());
        assert!(Translator::new(Some("https://host?key=secret"), None).is_err());
        let mut catalog = Catalog {
            code: "qx".into(),
            name: "Test".into(),
            source_hash: source_hash(),
            messages: SOURCE.clone(),
        };
        validate_catalog(&catalog).unwrap();
        catalog
            .messages
            .insert("replies.pong".into(), "Missing placeholder".into());
        assert!(validate_catalog(&catalog).is_err());
        catalog.messages = SOURCE.clone();
        catalog.source_hash = "outdated".into();
        assert!(install(catalog).is_err());
    }

    #[tokio::test]
    async fn generates_complete_catalogs_once_without_exposing_placeholders() {
        let (url, calls, server) = mock("qz", false).await;
        let dir = tempfile::tempdir().unwrap();
        let translator = Translator::new(Some(&url), None)
            .unwrap()
            .with_cache(dir.path());
        let (languages, warning) = translator.languages().await;
        assert!(warning.is_none());
        assert!(languages.iter().any(|l| l.code == "qz" && !l.ready));
        assert!(!languages.iter().any(|l| l.code == "zz"));
        let (first, second) = tokio::join!(translator.ensure("qz"), translator.ensure("qz"));
        first.unwrap();
        second.unwrap();
        let request_count = calls.load(Ordering::Relaxed);
        assert!(request_count > 0);
        let (reference_url, reference_calls, reference_server) = mock("qy", false).await;
        let reference_dir = tempfile::tempdir().unwrap();
        Translator::new(Some(&reference_url), None)
            .unwrap()
            .with_cache(reference_dir.path())
            .ensure("qy")
            .await
            .unwrap();
        assert_eq!(request_count, reference_calls.load(Ordering::Relaxed));
        reference_server.abort();
        let catalog: Catalog =
            serde_json::from_slice(&std::fs::read(dir.path().join("qz.json")).unwrap()).unwrap();
        assert_eq!(catalog.messages.len(), SOURCE.len());
        assert_eq!(
            placeholders(&catalog.messages["replies.runtime"]),
            placeholders(&SOURCE["replies.runtime"])
        );
        let pong = rust_i18n::t!("replies.pong", locale = "qz", response_ms = "0.50");
        assert!(pong.contains("Translated:") && pong.contains("0.50") && !pong.contains("%{"));
        server.abort();
        translator.ensure("qz").await.unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), request_count);
        let (_, warning) = translator.languages().await;
        assert!(warning.is_some());
        load_cache_from(dir.path()).unwrap();
        assert!(ready("qz"));
    }

    #[tokio::test]
    async fn failed_generation_does_not_install_or_save_partial_catalogs() {
        let (url, _, server) = mock("qy", true).await;
        let dir = tempfile::tempdir().unwrap();
        let translator = Translator::new(Some(&url), None)
            .unwrap()
            .with_cache(dir.path());
        assert!(translator.ensure("qy").await.is_err());
        assert!(!ready("qy"));
        assert!(!dir.path().join("qy.json").exists());
        assert!(translator.ensure("zz").await.is_err());
        server.abort();
    }
}
