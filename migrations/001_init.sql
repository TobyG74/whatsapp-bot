CREATE TABLE IF NOT EXISTS bot_settings (
    id BIGINT PRIMARY KEY CHECK (id = 1),
    payload TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS peers (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('contact', 'group')),
    payload TEXT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE TABLE IF NOT EXISTS user_preferences (
    id TEXT PRIMARY KEY,
    payload TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS command_usage (
    actor TEXT NOT NULL,
    day TEXT NOT NULL,
    command TEXT NOT NULL,
    used BIGINT NOT NULL,
    last_at BIGINT NOT NULL,
    PRIMARY KEY (actor, day, command)
);
CREATE TABLE IF NOT EXISTS seen_messages (
    id TEXT PRIMARY KEY,
    seen_at BIGINT NOT NULL
);
CREATE TABLE IF NOT EXISTS outbox (
    id TEXT PRIMARY KEY,
    payload TEXT NOT NULL,
    send_at BIGINT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'sending', 'sent', 'failed', 'cancelled')),
    failure_code TEXT NOT NULL DEFAULT '',
    created_at BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS outbox_due ON outbox (status, send_at);

