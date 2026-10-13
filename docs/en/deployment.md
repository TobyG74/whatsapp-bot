# Linux deployment with systemd

[Bahasa Indonesia](../id/deployment.md) | [README](../../README.md)

## Menu and command typo suggestions

`!setlang en` is a user command that saves the sender's own reply language in the encrypted application database, across group/private chats and restarts. Users without a preference use the dashboard's default reply language. Bundled `id`/`en` require no provider; other supported languages are generated before saving. Self mode continues to restrict all commands to owners. An unavailable personal language falls back temporarily to the bot default without erasing the choice; users can still run `setlang` to change it.

Owners can use `!setting` to list features or `!setting <feature> <on|off>` to toggle `autocorrect`, `use_limit`, `use_cooldown`, `autoreadmessage`, or `autoreadstatus`. The first three default to on; automatic reads default to off. Changes persist and apply to subsequent events. The dashboard preserves these toggles when saving general settings. `!setlimit 50` changes each non-owner's daily quota (1..10000) without resetting usage. `!setcooldown 3` sets a global cooldown (0..86400 seconds); `!setcooldown default` restores each command's configured cooldown. Disabling limit stops checks/charges while preserving previous usage; disabling cooldown bypasses command and suggestion cooldowns. Free commands remain usable at exhausted quota.

Automatic message reads cover incoming ordinary/command messages in group/private chats independently of command permissions and mode. Status reads use a separate toggle. Both skip outgoing echoes, newsletters, and events older than five minutes. Receipt sending runs alongside command handling and respects the account's WhatsApp read-receipt privacy settings.

Use `!menu` for all accessible commands, or `!menu debugging` to show one category. Other built-in category names are `general`, `media`, and `owner`. Filtering is case insensitive and accepts dashboard category names with spaces, such as `!menu Custom tools`. Aliases work too: `!help media`. The response lists only categories available to the sender in the current chat, including when the requested category does not match. The usage hint follows the configured prefix and menu name.

`!menu` groups active commands by category and shows their descriptions, aliases, current prefix, mode, and your role. 👤 marks user commands and 👑 marks owner commands. 🌐 means group/private, 👥 means group only, and 💬 means private only. Commands unavailable to the sender or the current chat are hidden. Menu content follows dashboard edits and deletions.

Autocorrect is automatic: send `!pnig` to get a suggestion for `!ping`, or `!hepl` for `!menu`. It compares the command token with current primary names and aliases and returns up to three equally close primary names using your prefix. Type the suggested command again to run it. Suggestions obey owner/scope/self-mode restrictions, ignore unrelated text and duplicate messages, cost no quota, and have a separate five-second cooldown per sender. Rebuild/restart to apply the new behavior. New English catalog keys require regenerating cached generated languages; bundled `id`/`en` work directly.

## Debugging message commands

After rebuilding/restarting, owners can use `!testbutton` (`button`), `!testlist` (`list`), `!testinteractive` (`interactive`), and `!testhtml` (`sendhtml`) in groups or private chats. Each sends a sample to the current chat, with a shared 3-second cooldown per command/alias. `!testhtml` immediately sends the built-in **WhatsApp BOT by TobyG74** HTML demo; no arguments are needed, and extra arguments are ignored. Customize `TITLE` and `HTML` in `src/bot/commands/debugging/testhtml.rs`, then rebuild/restart. The demo uses a light background, green accent, author credit, and bold/italic text with inline CSS and no external assets or scripts. Buttons/lists use the current prefix and configured action names; responses retain normal command authorization, cooldown, and quota checks.

`testhtml` (alias `sendhtml`) explicitly sends a `text` stanza, matching Chisato/Baileys. The default whatsapp-rust 0.7 classification is `media` with no media type and can leave the message invisible. Rebuild and restart the bot to apply this correction.

These native flow/rich response formats depend on WhatsApp client/account support. Live rendering has not been verified. See [message format examples](../../README.md#debugging-message-formats). Rebuilding changes the source catalog fingerprint, so regenerate previously generated languages with LibreTranslate available before using them; bundled `id`/`en` need no generation.

## Terminal logs

Startup shows a green ASCII **WhatsApp BOT** banner with a dimmed `Github: TobyG74` credit below it. When the dashboard starts, `[INF] Dashboard running on port 3000 | Open http://127.0.0.1:3000` displays the actual bound port and a browser URL. The values follow your configured address; wildcard binds use a loopback URL and port `0` shows the OS-assigned port. `NO_COLOR`/`CLICOLOR_FORCE` also apply to the banner. `LOG_FORMAT=json` omits it to preserve structured output.

Windows MSVC builds suppress linker warning `LNK4099` for missing dependency PDB files through `build.rs`. OpenSSL code still lacks those debug symbols; other linker warnings and errors remain visible. If Cargo replays warnings from an earlier build, stop the bot and run `cargo build --locked` once to rebuild the package with the new option. A full `cargo clean` is unnecessary.

Stop the existing bot process before starting another one with the same data directory. Run the compiled binary in the foreground without redirecting stdout:

```sh
RUST_LOG=whatsapp_bot=info,whatsapp_rust=error,wacore=error LOG_FORMAT=pretty ./whatsapp-bot --config config.toml run
```

Keep the terminal open. New command and ordinary group/private message events appear with `HH:mm:ss` timestamps and colored `[CMD]`/`[MSG]` tags. The sender is shown by name, falling back to their phone number or ID. Group chats add an arrow and the group name, falling back to the group ID. Private chats show only the sender. Names and identifiers are never appended together; slashes within actual names remain unchanged. Info/errors use `[INF]`/`[ERR]`, and warnings use `[WRN]`. Ctrl+C stops the bot. For PowerShell and example output, see the README. The dashboard uses English independently of the bot's reply language; original message content is displayed as received.

Group names are cached for five minutes (up to 512 entries), with invalidation on subject changes. A new lookup is limited to two seconds; it can delay the log entry while command handling proceeds concurrently. If it fails, the log uses the group ID and caches that result. Command completion can appear before the corresponding message entry during a lookup. Media without a caption uses a type marker such as `[image]`. Unresolved LIDs retain `@lid`, rather than being presented as phone numbers.

For failed sticker commands, check `[ERR] Command failed` in the terminal/dashboard logs. FFmpeg conversion errors include sanitized stderr; media download, upload, and delivery failures identify the failed stage. `[ERR] Command timed out` indicates the overall operation timeout. Oversized animated stickers are retried at lower quality/FPS within the same timeout, reserving space for pack/author metadata. The final limit remains 500 KiB. Rebuild and restart after deploying this fix; an already running binary keeps its previous behavior.

## Response time and uptime commands

`!ping` (alias `!p`) sends a temporary reply and edits it to show the measured send duration in milliseconds. It measures message preparation/transmission through the library, not delivery or read time. `!runtime` (alias `!uptime`) shows how long the current process has been running in days, hours, minutes, and seconds. A process restart resets uptime; a WhatsApp reconnect within the same process does not. Use your configured prefix instead of `!` if changed. Both commands work in group/private chats under the current bot mode and reply in the sender's saved language, falling back to the bot default.

Use `cargo run --release --locked -- --config config.toml run` for an optimized build. Contact/group persistence runs concurrently with the handler after the required permission, deduplication, and quota/cooldown checks. Sending time does not include those earlier checks, and performance varies with cache state, CPU, disk, and the network.

## systemd service

For automatic reply translations, start the optional service with `docker compose --profile translation up -d translate` and set `LIBRETRANSLATE_URL=http://127.0.0.1:5000` in the bot's `.env`. A bot in the Compose network uses `http://translate:5000`. LibreTranslate loads its installed language models; wait until the service is ready before generating catalogs. An API key is optional unless your instance requires it; store it in `LIBRETRANSLATE_API_KEY`.

Docker is optional. For a native service, follow the [official Python installation](https://docs.libretranslate.com/guides/installation/). On Linux, create a virtual environment with `python3 -m venv .tools/libretranslate`, install with `.tools/libretranslate/bin/python -m pip install libretranslate==1.9.6`, and run `.tools/libretranslate/bin/libretranslate --host 127.0.0.1 --port 5000`. Keep the service running while generating; first startup downloads models. Windows PowerShell commands are in the [README](../../README.md#automatic-reply-translations). The bot uses the same localhost URL for native and Docker services. Native installation/model loading have not been verified on this machine.

After rebuilding/restarting the bot, choose **Settings > Reply language > Save settings**. Missing catalogs are generated automatically before changing settings, and each user can use `!setlang <code>` for their personal preference. **Reload language list** updates the options. To prepare every language supported from English, run `cargo run --locked -- generate-locales --all`; use `--language fr,ja,ar` for selected codes. No files need to be created by hand.

Back up `data/i18n`, where generated catalogs are saved atomically. Cached replies work while the service is unavailable; generation failures preserve existing settings. Rebuilding after changing `locales/en.json` invalidates old catalogs, so keep LibreTranslate available to regenerate them. The dashboard remains English; this feature translates built-in bot templates. See the README for [setup and limitations](../../README.md#automatic-reply-translations).

Build a release as described in the README. Create a service user and an application directory, then copy the binary, `config.toml`, and `.env` to `/opt/whatsapp-bot`. The service user needs write access to the data directory and configured session location. Restrict access to `.env` (`chmod 600`) and keep the encryption key in a secret manager or a private backup.

Set `[sticker]` with `packname = "WhatsApp Bot"` and `authorname = "Your Name"` in `config.toml` as startup defaults for stickers generated from images/videos. Both fields allow up to 256 UTF-8 bytes without control characters. To update them immediately, use **Settings > Sticker settings** in the dashboard. Saved dashboard values override the file defaults and survive service restarts in the encrypted application database. Existing WebP files sent through the API keep their own metadata. Remove `[update]` from older configuration files.

### Managing commands and stickers

After deploying a rebuilt binary, open the dashboard and sign in. In **Commands**, use **Edit** to change the name, aliases, category, custom description, cooldown, quota cost, group/private scope, and owner restriction. **Save command** applies the change to subsequent messages without restarting. A blank custom description uses the default translation. Names/aliases must be unique lowercase identifiers starting with a letter; owner commands cannot be made public to non-owners.

**Delete** asks for confirmation, then removes the command and its aliases from the dispatcher, menu, and dashboard list. Its Rust handler file stays in the repository. A deletion persists across service restarts, and commands already executing may finish. Restore a deleted command through authenticated `PUT /api/commands/{original-id}` with its configuration and `deleted: false`; see the README for fields.

Use **Settings > Sticker settings > Save sticker settings** to change pack/author metadata for newly generated stickers. Command and sticker changes share the encrypted settings record with general bot settings on every database backend. Existing databases need no new schema migration. Saving general bot settings does not reset these overrides. A running binary must be rebuilt and restarted once to activate this dashboard feature.

### Service unit

Example unit file at `/etc/systemd/system/whatsapp-bot.service`:

```ini
[Unit]
Description=Rust WhatsApp Bot
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=whatsapp-bot
WorkingDirectory=/opt/whatsapp-bot
EnvironmentFile=/opt/whatsapp-bot/.env
ExecStart=/opt/whatsapp-bot/whatsapp-bot --config /opt/whatsapp-bot/config.toml run
Restart=on-failure
RestartSec=10
TimeoutStopSec=90
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ReadWritePaths=/opt/whatsapp-bot

[Install]
WantedBy=multi-user.target
```

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now whatsapp-bot
sudo journalctl -u whatsapp-bot -f
```

Keep the dashboard bound to `127.0.0.1:3000`. Access it locally through an SSH tunnel:

```sh
ssh -L 3000:127.0.0.1:3000 user@server
```

Set `LOG_FORMAT=json` in `.env` for full timestamps and structured service logs, or keep `LOG_FORMAT=pretty` for compact `HH:mm:ss` timestamps and chat labels. Pretty logs use the server's local timezone; interactive terminals get ANSI colors. `NO_COLOR=1` disables colors and `CLICOLOR_FORCE=1` forces them for redirected output. The dashboard displays times in the browser's timezone.

Logs now include previews of up to 300 message characters, sender identifiers, and group/private JIDs, including messages ignored in self mode. These previews are plaintext in stdout and the authenticated 200-entry in-memory dashboard buffer. Limit access to log files and configure rotation/retention in your service manager. The buffer clears on restart; it is not a message archive or delivery receipt. Status broadcasts, newsletters, and events older than five minutes are excluded.

Alternatively, use an HTTPS reverse proxy with your own domain and certificate. Set `client_max_body_size` high enough for base64 media; the default 16 MiB media limit needs about 22 MiB for a JSON request. Restrict network access to the dashboard. Do not expose the database or send tokens over public HTTP.

Deploy updates manually: stop the service with `sudo systemctl stop whatsapp-bot`, install the rebuilt binary and any required configuration/migrations, then run `sudo systemctl start whatsapp-bot`. The application has no automatic updater or `update` CLI command.

Back up the application database, SQLCipher session, and encryption key before upgrading. This project runs one process per account/database; it does not support a cluster of servers sharing a WhatsApp session.

For MongoDB, set `database.driver = "mongodb"`, include the database name in the URI, and set `media.max_bytes = 8388608` in `config.toml`. Store the URI containing credentials in `DATABASE_URL` in `.env`. The database user needs read/write and index creation permissions on the application database; the `readWrite` role on that database provides these permissions. Use authentication and TLS for network connections, and back up the database with `mongodump` together with the encryption key. The WhatsApp session still uses a local SQLCipher file, so a MongoDB backup alone does not include the bot session.
