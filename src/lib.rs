pub mod app;
pub mod bot;
pub mod infrastructure;
pub mod storage;
pub mod web;

rust_i18n::i18n!(
    "locales",
    fallback = "en",
    backend = infrastructure::i18n::RuntimeBackend
);
