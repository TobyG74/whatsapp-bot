# whatsapp-bot

A WhatsApp bot written in Rust using [oxidezap/whatsapp-rust](https://github.com/oxidezap/whatsapp-rust), Axum, SQLx, rust-i18n, and tracing. Run it locally or on a server with persistent storage.

The library connects through **WhatsApp Web**, rather than the official WhatsApp Business Cloud API. Upstream warns that its use may violate Meta's terms and put accounts at risk of suspension. Choose an account appropriate for your use case and send messages to recipients who expect them.

## Documentation

The README is in English. Deployment instructions and verification notes are available in both languages:

| Document | English | Bahasa Indonesia |
| --- | --- | --- |
| Linux deployment with systemd | [Deployment guide](docs/en/deployment.md) | [Panduan deployment](docs/id/deployment.md) |
| Local test results and limitations | [Verification notes](docs/en/verification.md) | [Catatan verifikasi](docs/id/verification.md) |

## Features

- Commands with aliases, categories, descriptions, cooldowns, quota costs, group/private scopes, and owner checks.
- `self` and `public` modes; the default is `self`.
- Indonesian and English replies through `rust-i18n`, plus automatic translation into the languages supported by a self-hosted LibreTranslate server. Generated catalogs are cached; the global locale stays unchanged between requests.
- Text, image, video, audio, document, and WebP sticker delivery through the API/dashboard/CLI, immediately or on a schedule.
- A sticker command for images/videos supplied with a caption or by replying to media, converted with FFmpeg, with configurable pack and author names. Videos are limited to 6 seconds.
- SQLite, PostgreSQL, or MongoDB for application data. Settings, contacts/groups that use commands, and queued payloads use AES-256-GCM. JID indexes use HMAC-SHA256.
- Persistent WhatsApp sessions in SQLite with **SQLCipher**. The library handles device authentication and reconnection with backoff; the application limits operation duration.
- A responsive dashboard with dark/light themes, Bearer token authentication, QR pairing, statistics, searchable message activity, command editing/deletion, sticker settings, and queue cancellation.
- Timestamped, colored terminal logs for commands and ordinary group/private messages; optional JSON output and the latest 200 entries in the dashboard.

## Requirements

- Rust stable **1.94 or later** and Cargo. `rust-toolchain.toml` selects stable. Upstream's nightly SIMD feature is disabled.
- A C toolchain, Make, and Perl to build vendored SQLCipher/OpenSSL.
- FFmpeg with the `libwebp` encoder for the sticker command.
- Linux: `sudo apt-get install build-essential pkg-config perl ffmpeg ca-certificates`.
- Windows: install Rust through [rustup](https://rustup.rs/), Visual Studio Build Tools with the **Desktop development with C++** workload, [Strawberry Perl](https://strawberryperl.com/), and FFmpeg on PATH. Use a Visual Studio developer terminal if the linker cannot be found.
- Alternatively, use Docker for Linux builds and runtime.

## Getting started in PowerShell

On Windows with MSVC, `build.rs` suppresses linker warning `LNK4099` about missing dependency PDB files (such as OpenSSL's `ossl_static.pdb`). Other warnings and errors remain visible. This hides the diagnostic; it does not restore the missing OpenSSL debug symbols. The option applies only to this package on MSVC and is not passed to GNU/Linux builds.

```powershell
Copy-Item config.example.toml config.toml
Copy-Item .env.example .env
cargo run -- secrets
```

Copy the values printed by `secrets` into `.env`; do not commit or share this file. Replace `bot.owners` in `config.toml` with the owner's phone number in international format, using digits only. Indonesian example: `6281234567890`.

```powershell
cargo run -- run
```

Open `http://127.0.0.1:3000`, enter `DASHBOARD_TOKEN`, and scan the QR code through **WhatsApp > Linked devices > Link a device**. The application does not print the QR code to logs. It saves the session so subsequent restarts do not require pairing while the session remains valid.

On Linux/macOS, use `cp` instead of `Copy-Item`. For a production server:

```sh
cargo build --release --locked
./target/release/whatsapp-bot --config config.toml run
```

Try the dashboard without a WhatsApp account:

```sh
cargo run -- run --offline
```

`--offline` does not establish a WhatsApp connection. Queued messages are still saved and may be sent when a later process runs online. Cancel test messages before connecting a real account.

## WhatsApp commands

The default prefix is `!`. All aliases share permissions, quota costs, and cooldowns with their primary command.

| Command | Aliases | Category | Cooldown | Quota cost | Chat | Access |
| --- | --- | --- | --- | --- | --- | --- |
| `!ping` | `p` | general | 2 seconds | 1 | group/private | user |
| `!runtime` | `uptime` | general | 2 seconds | 0 | group/private | user |
| `!menu [category]` | `help`, `commands` | general | 3 seconds | 0 | group/private | user |
| `!about` | `info` | general | 3 seconds | 0 | group/private | user |
| `!sticker` | `s`, `stiker` | media | 10 seconds | 2 | group/private | user |
| `!testbutton` | `button` | debugging | 3 seconds | 0 | group/private | owner |
| `!testlist` | `list` | debugging | 3 seconds | 0 | group/private | owner |
| `!testinteractive` | `interactive` | debugging | 3 seconds | 0 | group/private | owner |
| `!testhtml` | `sendhtml` | debugging | 3 seconds | 0 | group/private | owner |
| `!config` | `settings` | owner | 2 seconds | 0 | private | owner |
| `!setprefix #` | `prefix` | owner | 2 seconds | 0 | private | owner |
| `!setlang en` | `lang` | general | 2 seconds | 0 | group/private | user |
| `!setting [feature on/off]` | none | owner | 0 seconds | 0 | group/private | owner |
| `!setlimit 30` | none | owner | 0 seconds | 0 | group/private | owner |
| `!setcooldown 3` | none | owner | 0 seconds | 0 | group/private | owner |
| `!self` | `privatebot` | owner | 2 seconds | 0 | group/private | owner |
| `!public` | `publicbot` | owner | 2 seconds | 0 | group/private | owner |

`self` ignores commands from non-owners. `public` allows general commands for everyone; owner commands remain restricted. `!menu` groups commands by category, shows the current prefix/mode/role, and lists descriptions and aliases. Markers identify access (👤 user, 👑 owner) and chat scope (🌐 group/private, 👥 group, 💬 private). The menu shows only active commands available to the sender in the current chat type and uses the command names, categories, and descriptions configured in the dashboard.

To filter the menu, use `!menu general`, `!menu media`, `!menu owner`, or `!menu debugging`. Without arguments, `!menu` displays all accessible categories. The category filter ignores capitalization and surrounding whitespace, and supports custom category names from the dashboard, including names with spaces (for example, `!menu Custom tools`). Aliases such as `!help debugging` also work. The menu lists accessible category names and an example using the current prefix and configured menu name. An unknown or inaccessible category returns an empty-category notice and the available category list; it does not reveal restricted commands.

### Command typo suggestions

`!setlang en` saves the sender's personal reply language in the encrypted application database. The choice applies in group and private chats and survives restarts; it does not change anyone else's language or the dashboard's default. Users without a saved choice use **Settings > Reply language** as their default. Self mode still restricts commands to owners. Bundled `id`/`en` work without LibreTranslate; other supported languages are generated before the choice is saved. If a saved language cannot be loaded/generated, replies temporarily use the bot default while retaining the preference, so the user can still change it with `setlang`.

Owners can run `!setting` to list feature switches, or `!setting <feature> <on|off>` to change one. The features are `autocorrect`, `use_limit` (also `uselimit`), `use_cooldown` (also `usecooldown`), `autoreadmessage`, and `autoreadstatus`. Autocorrect, quota enforcement, and cooldowns default to on; automatic reads default to off. Changes apply to subsequent messages and survive restarts. Saving the dashboard's general settings preserves these switches.

`!setlimit 50` sets the daily quota for each non-owner to 50 (valid range: 1..10000), preserving existing usage. Disabling `use_limit` stops quota checks/charges without clearing prior usage. Commands with zero quota cost remain available even after the daily limit is reached. `!setcooldown 3` overrides all command cooldowns with three seconds (valid range: 0..86400); `!setcooldown default` restores each command's configured cooldown. Disabling `use_cooldown` bypasses both command cooldowns and the five-second suggestion cooldown. Previously recorded usage/timestamps remain intact.

`autoreadmessage` marks incoming group/private messages as read, including ordinary messages. `autoreadstatus` independently marks incoming status broadcasts as viewed. Reads run concurrently with command handling, skip outgoing echoes/newsletters and events older than five minutes, and respect WhatsApp's read-receipt privacy setting. They do not send a reply. These switches apply regardless of command permissions or self/public mode.

Autocorrect runs automatically when an unknown prefixed command closely matches an available command or alias. For example, `!pnig` suggests `!ping` and `!hepl` suggests `!menu`. It suggests up to three equally close primary command names; type the intended command again to execute it. Adjacent letter swaps, missing/extra letters, and substitutions are recognized, with at most one edit for 2–4 character tokens or two edits for longer tokens (up to 32 ASCII characters). Unrelated tokens and ordinary messages receive no suggestion. Only the command name is compared; arguments are ignored.

Suggestions respect self/public mode, owner access, group/private scope, dashboard renames, aliases, and deletions. Duplicate messages are ignored, and suggestion replies have a separate five-second cooldown per sender across chats with zero quota cost. They do not run a command or increment the executed-command counter. The reply uses the sender's saved language, falling back to the bot default. New translation keys invalidate generated language caches; regenerate previously generated languages with LibreTranslate available before using them. Bundled English/Indonesian need no generation.

`!ping` sends a temporary Pong reply, measures how long the library takes to send it using a monotonic clock, and edits that reply to show milliseconds (for example, `Pong! Response time: 123.45 ms.`). This includes message preparation and transmission, rather than recipient delivery or read time. `!runtime` (alias `!uptime`) reports days, hours, minutes, and seconds since the current bot process started; restarting the process resets this timer.

For normal operation, use an optimized build: `cargo run --release --locked -- --config config.toml run`. Command handling runs concurrently with message logging, so group-name lookups do not delay replies. After permission, deduplication, and quota/cooldown checks, contact/group persistence runs alongside the command handler. Both operations are awaited and their errors remain checked. The Ping number measures sending only; it excludes earlier database checks and network delivery to the recipient. A result of 3 ms is already a short send duration, and a release build does not guarantee a lower result on every request.

The bot verifies owners using their WhatsApp phone number (PN) or the alternative PN identity supplied by the library. It does not compare LID numbers as phone numbers. Messages marked `from_me` by the linked bot account count as owner messages. Configure the owner's actual WhatsApp number or send commands from the bot account itself.

### Debugging message formats

Each debugging command sends one test message to the chat where it was invoked. `!testbutton` sends quick replies for Ping/Runtime, `!testlist` sends a native selection list for Ping/Runtime/Menu, and `!testinteractive` combines a quick reply, a GitHub URL button, and a copy-code button. Action IDs use the current prefix and configured command names. Clicking a reply/list action goes through the normal dispatcher, including permissions, cooldowns, and quotas; deleted commands cannot execute.

`!testhtml` (alias `!sendhtml`) sends the built-in **WhatsApp BOT by TobyG74** HTML demo directly, without arguments. The light layout uses a green accent, an author credit, and examples of bold/italic text. Edit `TITLE` and `HTML` in `src/bot/commands/debugging/testhtml.rs` to customize the demo, then rebuild/restart. Command arguments are ignored. The payload follows the [referenced rich HTML envelope](https://github.com/TobyG74/ChisatoBOT/blob/598bade91bef28a47572e430ea777f2ce16e30db/src/libs/client/client.ts#L666); button/list payloads follow [TemplateBuilder](https://github.com/TobyG74/ChisatoBOT/blob/master/src/libs/interactive/TemplateBuilder.ts). HTML messages explicitly use a `text` stanza to match Chisato/Baileys: whatsapp-rust 0.7 otherwise classifies this envelope as `media` without a media type, which can result in an invisible message. These are WhatsApp Web protocol experiments: client versions/accounts can hide or reject them, and successful sending does not prove rendering. The demo uses inline styles without external assets or scripts. Live Android rendering of its styles has not been verified. Demo text is fixed in English; command descriptions and selection instructions use the configured reply language.

Rebuild and restart the bot to register these commands in the menu/dashboard. New descriptions change the English-source fingerprint; previously generated language catalogs must be regenerated with LibreTranslate available. Bundled Indonesian/English remain usable without translation generation.

Examples:

```text
!ping
!runtime
!help
!setlang en
!setprefix #
#public
#sticker   (reply to an image/video)
#self
```

To use a caption, send an image/video with `!sticker` as its caption. Static stickers are limited to 100 KiB and animated stickers to 500 KiB, including metadata; complex sources may need to be simplified. An accepted command consumes its quota cost even if conversion fails. Owners do not consume quota, but cooldowns still apply. Quotas reset at **00:00 UTC**. The database stores cooldowns, quotas, and message deduplication records. The bot ignores command messages older than 5 minutes to avoid executing old commands during synchronization.

Animated conversion starts at quality 50 and 12 FPS. If the result is too large, it retries at quality 20/8 FPS, then quality 5/6 FPS, with space reserved for pack/author metadata. All attempts share the configured operation timeout. Extremely complex videos can still exceed the limit; use a shorter or simpler clip. Missing source length metadata is accepted, while the download remains bounded by `media.max_bytes`. Command failures appear as `[ERR] Command failed` with an error detail in the terminal and dashboard logs; FFmpeg failures include a sanitized diagnostic. Timeouts appear separately as `[ERR] Command timed out`.

Set the pack and author names for generated stickers in `config.toml`:

```toml
[sticker]
packname = "WhatsApp Bot"
authorname = "Your Name"
```

Both names support Unicode and allow up to 256 UTF-8 bytes without control characters. These file settings are startup defaults. To change them while the bot runs, open **Settings > Sticker settings** in the dashboard and save the pack/author names. Dashboard values are stored in the encrypted application database, take precedence over `[sticker]`, and survive restarts. The sticker command embeds the current values in WebP EXIF metadata for both static and animated stickers. Existing files sent through the API/dashboard/CLI keep their own metadata. Omitting `[sticker]` or either field uses an empty name until a dashboard override is saved. Remove the obsolete `[update]` section from older configurations; the application no longer accepts it.

## Automatic reply translations

You only maintain the English source catalog, `locales/en.json`. Indonesian remains a bundled translation. Additional languages are generated from the English templates using the [LibreTranslate API](https://docs.libretranslate.com/guides/api_usage/), then stored as complete JSON catalogs in `data/i18n`. No manual translation files or rebuilds are needed for new languages. This translates the bot's built-in command descriptions/replies/errors; dashboard text and custom command descriptions stay in the language they were written in.

Docker is optional. To run LibreTranslate directly on Windows with Python 3.13, open PowerShell in the repository and create an isolated environment:

```powershell
py -3.13 -m venv .tools/libretranslate
.\.tools\libretranslate\Scripts\python.exe -m pip install libretranslate==1.9.6
.\.tools\libretranslate\Scripts\libretranslate.exe --host 127.0.0.1 --port 5000
```

Keep that terminal open while generating translations. The first startup downloads language models; wait until the service responds at `http://127.0.0.1:5000/languages`. This follows the [official Python installation](https://docs.libretranslate.com/guides/installation/); install Python 3.13 if `py -3.13` is unavailable. Native installation and model loading have not been verified on this machine.

Alternatively, start Docker Desktop on Windows, then start the optional translation container:

```sh
docker compose --profile translation up -d translate
```

Set this in `.env` for a bot running directly on the host:

```dotenv
LIBRETRANSLATE_URL=http://127.0.0.1:5000
# LIBRETRANSLATE_API_KEY=your-key-if-required
```

For a bot running in the same Compose network, use `LIBRETRANSLATE_URL=http://translate:5000`. Rebuild/restart the bot once to activate this feature and read the environment setting. The first [LibreTranslate startup](https://docs.libretranslate.com/guides/installation/) downloads language models and can take time. For Docker, inspect `docker compose logs -f translate` until the service is ready; model data is persisted in the `translation-models` volume. For a native service, watch its terminal output.

In **Settings > Reply language**, select a default language and choose **Save settings**. The bot generates missing translations before saving the selection; failed generation leaves the existing settings unchanged. **Reload language list** refreshes available models and newly generated catalogs. Each user can choose a personal language with `!setlang fr`, `!setlang ja`, or another code shown in the list. Available languages are those the server can translate from English, together with bundled/cached catalogs; this does not promise every language in the world.

Generate all supported languages ahead of time, or choose several codes:

```sh
cargo run --locked -- generate-locales --all
cargo run --locked -- generate-locales --language fr,ja,ar
```

Successful catalogs are kept if another language fails; rerun the same command to retry missing languages. Already generated languages are reused. Bulk generation can take several minutes. Each language has a 60-second generation deadline, and requests are sent in batches of eight literal text segments. Placeholders such as `%{name}` and `%{response_ms}` are kept locally and reinserted unchanged; user messages, tokens, settings, and names are not sent for translation.

Generated catalogs load at runtime and survive restarts without a rebuild. Normal replies use the local cache even when LibreTranslate is unavailable. Catalogs include an English-source fingerprint; after changing the source and rebuilding, stale catalogs are ignored and must be regenerated with the service available. A configured reply language without a valid cache requires generation at startup. Back up `data/i18n` with the bot data. Machine translation can need language-specific review, especially for phrases around variables.

## Automatic messaging

The bot process must be running. The CLI reads the token from `.env` and sends a request to the dashboard:

```sh
cargo run -- send --to 6281234567890 --text "Hello from Rust"
cargo run -- send --to 6281234567890 --media ./photo.jpg --kind image --text "Photo caption"
cargo run -- send --to 6281234567890 --media ./video.mp4 --kind video
cargo run -- send --to 6281234567890 --media ./report.pdf --kind document
cargo run -- send --to 6281234567890 --media ./audio.ogg --kind audio
cargo run -- send --to 6281234567890 --media ./sticker.webp --kind sticker
cargo run -- send --to 120363012345678@g.us --text "Reminder" --at "2026-12-01T09:00:00+07:00"
```

Without `--kind`, the CLI sends a file as a document. It infers the MIME type from the extension; use `--mimetype` to override it. Use `--api-url https://bot.example.com` for another server. The server does not download media from arbitrary URLs; send file contents as base64.

`send_at` is a one-time RFC3339 timestamp, at most one year in the future. Without a timestamp, the message enters the queue immediately. Messages wait while WhatsApp is disconnected and resume after reconnection. Use cron or Task Scheduler to invoke the `send` CLI for recurring schedules.

The worker processes one message at a time. It does not retry automatically when delivery may already have happened. `sent` means the send API succeeded, **not that the recipient read the message**. Timeouts/transmission failures receive `delivery_unknown`; a restart changes `sending` jobs to `failed`. Check with the recipient before scheduling the message again to avoid duplicates. Completed/failed/cancelled jobs are removed after 7 days, and completed payloads are cleared immediately. Deduplication records are retained for 7 days.

## Configuration and databases

`config.example.toml` contains the initial configuration. The application uses `[bot]` settings when it first creates the database. After that, bot settings live in the database; change them through the dashboard or commands. Command edits, command deletions, and dashboard sticker settings use the same encrypted settings record on SQLite, PostgreSQL, and MongoDB; the user-language table is created automatically for existing SQL databases at startup. Database, connection, media, and dashboard server settings are read at startup and require a restart.

Default SQLite configuration:

```toml
[database]
driver = "sqlite"
url = "sqlite://data/bot.db?mode=rwc"
```

PostgreSQL:

```toml
[database]
driver = "postgres"
url = "postgres://bot:password@localhost/whatsapp_bot?sslmode=require"
```

MongoDB, using the [official Rust driver](https://www.mongodb.com/docs/drivers/rust/current/):

```toml
[database]
driver = "mongodb"
url = "mongodb://127.0.0.1:27017/whatsapp_bot"

[media]
max_bytes = 8388608
ffmpeg = "ffmpeg"
```

Run MongoDB locally with `docker run -d --name whatsapp-mongo -p 127.0.0.1:27017:27017 -v whatsapp-mongo:/data/db mongo:8.0`. This example is for local development; production servers need authentication and network access restrictions. MongoDB Atlas is also supported through a URI such as `mongodb+srv://user:password@cluster.example.com/whatsapp_bot?retryWrites=true&w=majority`; replace the host and credentials with your cluster's values. The URI must include a database name.

Store URLs containing passwords in `DATABASE_URL` in `.env`; this overrides `database.url`. Create the PostgreSQL database first. The application creates SQL tables and MongoDB collections/indexes automatically. The `driver` must match the URL scheme. Switching backends selects a different database without copying existing data. Back up and migrate data separately if needed.

All three backends store settings, contacts/groups, message deduplication, cooldowns/quotas, and queued messages. MongoDB stores daily usage and command timestamps in one document per user/day and updates it atomically, without requiring a replica set for transactions. Queue claims are atomic, and deduplication uses unique `_id` values. MongoDB media is limited to **8 MiB** because JSON/base64 and encryption increase payload size within the [16 MiB BSON document limit](https://www.mongodb.com/docs/manual/reference/limits/#bson-document-size). Startup rejects larger media limits for MongoDB; SQLite/PostgreSQL can still use limits up to 64 MiB. GridFS is not used.

**WhatsApp session storage always uses SQLite encrypted with SQLCipher** (`connection.session_path`), including when the application database uses PostgreSQL or MongoDB. Keep the session file on persistent local storage. Run one bot instance per data directory/session/application database. The `data/bot.lock` file prevents two processes from using the same directory, but does not provide a lock across servers.

`DATA_ENCRYPTION_KEY` is 32 bytes encoded as 64 hexadecimal characters. Keep a private backup of the key together with database backups; changing or losing it makes existing data and sessions unreadable. The application does not provide key rotation. Back up SQLite with its backup API, or stop the process before copying the database and its WAL state. Use `pg_dump` for PostgreSQL or `mongodump` for MongoDB. Do not use a session/plaintext WhatsApp database from another application without a deliberate SQLCipher migration.

Application encryption protects settings, contacts, and payload values. Record types, timestamps, queue status, and usage counts remain visible in the database. Restrict access to `.env`, configuration, backups, and the `data` directory to the account running the bot.

## Dashboard and HTTP API

Every `/api/*` endpoint requires `Authorization: Bearer <DASHBOARD_TOKEN>`. The token grants full owner access. The browser stores it only in page memory. `/health` reports that the HTTP server is alive; it does not report the WhatsApp connection state.

In **Commands**, choose **Edit** to change a command's name, aliases, category, description, cooldown, quota cost, chat scope, or owner restriction, then choose **Save command**. Leave the custom description blank to keep the default translated description. Names and aliases must be unique lowercase identifiers starting with a letter, up to 32 characters; each command accepts up to 20 aliases. Built-in owner commands always retain owner access. These edits change command configuration; handlers remain in their Rust source files.

Choose **Delete** and confirm to remove a command and its aliases from the active dispatcher, bot menu, and dashboard list. Deletions survive restarts; source files remain available. Commands already executing may finish. Renaming a command keeps its existing cooldown history. All saved command settings apply to subsequent messages without restarting.

Under **Settings > Sticker settings**, edit the pack name and author name, then choose **Save sticker settings**. This applies to newly generated stickers immediately. Saving general bot settings preserves command and sticker edits.

| Method | Endpoint | Purpose |
| --- | --- | --- |
| GET | `/health` | Public HTTP health check |
| GET | `/api/status` | Connection state, active QR code, statistics |
| GET | `/api/commands` | Active command configuration, stable IDs, and English descriptions |
| PUT / DELETE | `/api/commands/{id}` | Edit or delete a built-in command by its original ID |
| GET / PUT | `/api/sticker` | Read/save generated sticker pack and author names |
| GET | `/api/languages` | Bundled/cached and provider-supported reply languages, readiness, service warning |
| GET | `/api/logs` | Latest 200 application log entries |
| GET / PUT | `/api/settings` | Read settings / replace general bot settings while preserving command and sticker overrides |
| GET / POST | `/api/messages` | Latest 100 jobs / create a job |
| DELETE | `/api/messages/{id}` | Cancel a pending job |
| POST | `/api/reconnect` | Request reconnection of an active session |

PowerShell example:

```powershell
$headers = @{ Authorization = "Bearer $env:DASHBOARD_TOKEN" }
$body = @{ to = "6281234567890"; kind = "text"; text = "Automatic message" } | ConvertTo-Json
Invoke-RestMethod http://127.0.0.1:3000/api/messages -Method Post -Headers $headers -ContentType application/json -Body $body
```

PowerShell does not load `.env` automatically. Set `$env:DASHBOARD_TOKEN` first, or use the `send` CLI, which reads `.env`.

Command IDs remain the original built-in names even after renaming. `PUT /api/commands/ping`, for example, accepts `name`, `aliases` (array), `category`, `description` (custom string or `null`), `cooldown_secs` (0..86400), `limit_cost` (0..10000), `scope` (`both`, `group`, or `private`), and `owner_only` (boolean). The optional `deleted` field defaults to `false`; submitting the configuration with `deleted: false` restores a deleted command if its names are still available. The commands GET response includes `description_override` for editing and `owner_access_required` for built-in restrictions. `PUT /api/sticker` accepts `{"packname":"My Pack","authorname":"My Name"}`.

Media requests use `data_base64`, `file_name`, `mimetype`, and `text` for captions. Audio/stickers do not use captions. Supported types are `text`, `image`, `video`, `audio`, `document`, and `sticker`. The default media limit is 16 MiB, and the text limit is 4096 bytes; oversized requests are rejected. The Content Security Policy restricts scripts to local assets, CORS is not enabled, and responses are not cached.

The dashboard binds to localhost by default. For cloud deployments, use an HTTPS reverse proxy and restrict dashboard access. Do not send tokens over public HTTP. Upstream handles temporary disconnections automatically; logout, expired QR codes, and outdated clients require a restart or library update. The application does not delete session files automatically after errors.

## Docker

Prepare `.env` and `config.toml`. Set `dashboard.bind` to **`0.0.0.0:3000`** inside the container, then run:

```sh
docker compose up --build -d
docker compose logs -f bot
```

The host port remains bound to `127.0.0.1:3000`. The `bot-data` volume stores the database/session, and the container runs as user `bot`. Keep the volume when updating the container. Rebuild and recreate the container to deploy a new version.

## Manual updates

Stop the bot and back up the application database, WhatsApp session, and encryption key before upgrading. Pull the desired version into your existing checkout, rebuild with `cargo build --release --locked`, and restart the bot. Review configuration and migration changes before deploying. There is no automatic updater or `update` CLI command.

## Project structure and development

```text
src/
  main.rs          CLI, startup, shutdown
  lib.rs           modules and i18n
  app/
    config.rs      TOML and validation
    state.rs       shared state and settings changes
  bot/
    messaging.rs   WhatsApp connection, media, queue worker
    message_log.rs Incoming command and group/private message activity
    sticker_metadata.rs WebP sticker pack/author metadata
    commands/
      mod.rs       registry, parser, permissions, cooldown/quota, dispatcher
      autocorrect.rs typo matching for the dispatcher
      general/     ping.rs, runtime.rs, menu.rs, about.rs, setlang.rs
      media/       sticker.rs
      debugging/   testbutton.rs, testlist.rs, testinteractive.rs, testhtml.rs
      owner/       config.rs, setprefix.rs, setting.rs, setlimit.rs,
                   setcooldown.rs, self_mode.rs, public.rs
  storage/
    mod.rs         database API and backend selection
    sql.rs         SQLite/PostgreSQL, migrations, queries
    mongo.rs       MongoDB, collections, indexes, atomic operations
  web/
    mod.rs         Axum router and dashboard authentication
    assets/        HTML, CSS, JavaScript embedded in the binary
  infrastructure/
    security.rs    encryption, private indexes, tokens
    logging.rs     Console/JSON tracing and dashboard buffer
locales/           en.json source and bundled id.yml
migrations/        shared SQLite/PostgreSQL schema
docs/
  en/              English deployment and verification guides
  id/              Indonesian deployment and verification guides
```

Each command file contains its own `COMMAND` metadata and `execute` function. `self_mode.rs` handles the `self` command; the file name differs because `self` is a Rust keyword. Parsing, permission checks, cooldowns, quotas, and usage recording remain centralized in `bot/commands/mod.rs`.

To add a command, create a file in the appropriate category folder using an existing command as a guide. Declare its module in the category's `mod.rs`, register its `COMMAND` constant in `COMMANDS`, and add description/reply keys to both locales. The metadata handler connects the command to `execute`, so the dispatcher needs no additional branch. Available scopes are `Scope::Group`, `Scope::Private`, and `Scope::Both`. All aliases go through the same permission/admission checks.

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --locked -- --ignored
```

The `--ignored` tests cover FFmpeg, PostgreSQL, and MongoDB. Install FFmpeg, set `POSTGRES_TEST_URL` to a **dedicated test database**, and set `MONGODB_TEST_URL=mongodb://localhost:27017/bot_test`. The MongoDB test creates a temporary database with the `bot_test_` prefix, checks encrypted storage, cooldowns/quotas, concurrent claims, restart recovery, and cleanup, then removes that database. CI supplies both database servers. Local tests do not send messages or connect a WhatsApp account.

Logs default to `LOG_FORMAT=pretty`: local time in `HH:mm:ss`, a colored `[MSG]`, `[CMD]`, `[INF]`, or `[ERR]` tag, and readable message details. Commands use cyan, group messages amber, private messages green, and errors red. Warnings use `[WRN]`; debug/trace events use `[DBG]`/`[TRC]` when enabled. Color is enabled for interactive terminals; use `NO_COLOR=1` to disable it or `CLICOLOR_FORCE=1` to enable it when redirecting stdout. Set `LOG_FORMAT=json` for full timestamps and structured server logs.

Startup prints a green ASCII **WhatsApp BOT** banner with a dimmed `Github: TobyG74` credit below it. Once the dashboard starts, an `[INF]` entry shows its actual port and browser URL, for example `Dashboard running on port 3000 | Open http://127.0.0.1:3000`. Wildcard binds show a local loopback URL; port `0` displays the port selected by the OS. `NO_COLOR` and `CLICOLOR_FORCE` also control the banner. JSON logging omits the banner and retains structured startup address/port fields.

Example output with fictional contacts/groups:

```text
18:44:15 [MSG] Alice → Rust Group | hello
18:44:15 [CMD] Alice → Rust Group | !ping
18:44:15 [MSG] Alice | hello
18:44:15 [CMD] Alice | !ping
18:44:15 [INF] Bot ready
18:44:15 [ERR] Command failed
```

Messages and commands show the sender's name when available, otherwise their phone number or ID. Group chats add an arrow and the group name, falling back to the group ID; private chats show only the sender. Names and identifiers are never appended together. Slashes within actual names remain unchanged. Unresolved LIDs retain `@lid`. Textless media appears as `[image]`, `[sticker]`, etc. Group names are cached for five minutes with a 512-entry bound and invalidated on subject changes. A cache miss may delay that log entry by up to two seconds for metadata, while command handling proceeds concurrently; failed lookups fall back to the group ID and are cached too. Previews contain up to 300 characters.

Message logs use the `INFO` level. Keep `whatsapp_bot=info` in `RUST_LOG`; inherited environment variables take precedence over `.env`.

To see logs directly in a terminal, stop any existing bot process and run it in the foreground without redirecting stdout. For PowerShell:

```powershell
$env:RUST_LOG = 'whatsapp_bot=info,whatsapp_rust=error,wacore=error'
$env:LOG_FORMAT = 'pretty'
cargo run -- --config config.toml run
```

Keep this terminal open while the bot runs. Press Ctrl+C to stop it. New group/private messages and commands appear with timestamps and colored labels when received. The authenticated dashboard remains available at the configured address.

The dashboard uses English throughout, including navigation, notifications, command descriptions, activity labels, and date formatting. **Reply language** sets the default for users without a personal choice; it does not change the dashboard language or translate message previews.

The dashboard's **Message activity** page displays the latest 200 application entries in memory, newest first, with search, command/message/system filters, group/private filters, and a pause control. It displays timestamps in the browser's local timezone. Theme preference is saved locally; authentication tokens are kept only in memory.

Message logging runs alongside command handling and includes ordinary messages in self mode and unknown prefixed commands. An activity labeled command means it was received, not that execution succeeded; successful execution emits a separate `Command completed` entry. A pending group-name lookup can make that completion entry appear before its message entry. Events older than five minutes, status broadcasts, and newsletters are excluded. Media contents are not downloaded for logging; only type and available caption are shown. Outgoing echoes received from WhatsApp are labeled outgoing; this feed is not a delivery receipt or a complete archive of bot replies.

**Message previews and sender/chat identifiers appear in plaintext stdout logs and the authenticated dashboard buffer.** Restrict access and retention for redirected logs; database encryption does not encrypt stdout files. The dashboard buffer clears on restart. The default filter limits upstream logs to errors. Upstream debug filters may include protocol data, so use them only for local diagnosis and keep the logs private.

