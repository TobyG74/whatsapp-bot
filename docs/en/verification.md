# Local verification notes

[Bahasa Indonesia](../id/verification.md) | [README](../../README.md)

Date: October 12, 2026. Testing used Windows, Rust stable 1.99 with the GNU target, PostgreSQL 17.6, MongoDB 8.0.15 standalone, FFmpeg, and Chromium through Playwright. The local test toolchain used OpenSSL from Strawberry Perl; builds with vendored OpenSSL on Linux/MSVC were not run locally.

| Check | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --locked --all-targets -- -D warnings` | PASS |
| `cargo test --locked -- --include-ignored` | PASS: 23 tests, no failures or ignored tests |
| `cargo build --locked` | PASS |
| `node --check src/web/assets/app.js` | PASS |
| Startup with `run --offline` and MongoDB | PASS: `/health` returned `ok` |
| CLI `send` through the dashboard, followed by cancellation | PASS: the pending job became cancelled |
| A second process using the same data directory | PASS: the file lock rejected it |
| Restart with MongoDB in offline mode | PASS: the prefix, language, mode, and three cancelled jobs persisted |

MongoDB tests used a temporary `bot_test_<UUID>` database and removed it after the test. Coverage included settings, contacts/groups, private JID indexes, payload encryption, concurrent deduplication, cooldowns, limits across commands, daily resets, owners with zero quota cost, admission from two connections, concurrent queue claims, cancellation, restart recovery for sending jobs, corrupted payloads, cleanup of old records, 8 MiB media, and rejection of oversized payloads. SQLite/PostgreSQL and SQLCipher were also tested. FFmpeg tests produced WebP stickers from a single-frame image and a short video.

## Previous dashboard baseline

Browser tests covered login with invalid/valid tokens, saving settings, queueing scheduled text and an image, cancelling both jobs, the offline reconnect error, refresh, every navigation link, logout, keyboard button activation, and visible keyboard focus. There were no JavaScript errors. Viewport widths of 320, 390, 768, 1024, and 1280 px caused no page overflow; wide tables used scroll containers. Desktop/mobile screenshots were reviewed locally.

The palette uses dark text and one green accent for operator controls. System fonts keep text readable without external assets; sections follow the operator's tasks: connection, sending, settings, queue, commands, and logs. The light theme supports reading tables/logs; the interface has no theme toggle. ENERGY 1 / RHYTHM 1 / MOTION 1, with motion limited to buttons and support for reduced motion.

- Hard Gate PASS: controls were tested in the browser, empty/loading/error states were available, and keyboard navigation and viewports were checked. Tested text color pairs had a minimum contrast ratio of 6.03:1, with control borders at 4.86:1.
- Purpose Gate PASS: green highlights actions, and bordered panels separate operator tasks. There are no illustrations, customer statistics, or testimonials.
- Liveliness PASS: connection status is the monitoring focus. Whitespace separates tasks, while heading hierarchy and the green palette remain consistent with a calm operator dashboard.
- Craftsmanship PASS: forms saved actual data to MongoDB; queue/cancel/refresh/logout used the API; statistics came from the bot process. Screenshots and browser tests documented these checks.

## Message activity and dashboard update

The later update on October 12, 2026 passed `cargo fmt --check`, Clippy with warnings denied, `cargo build --locked`, and JavaScript syntax checking. `cargo test --locked` passed **24 tests**, with the three existing FFmpeg/PostgreSQL/MongoDB integration tests left ignored; those backends were unchanged. New tests check prefixed commands, Unicode preview truncation, terminal control sanitization, timestamps and optional ANSI colors, allowed fields, and the 200-entry buffer.

Chromium tests against a separate SQLite bot in **offline mode** passed invalid/valid login, settings persistence, scheduled text/image queueing and cancellation, offline reconnect errors, command search, message search and kind/chat filters, pause/resume, empty states, HTML injection escaping, theme persistence, logout, mobile navigation, Escape, visible keyboard focus, and reduced motion. Both themes and all five views were checked at 320, 390, 768, 1024, and 1280px without page overflow; wide tables scroll within their own containers. A 640px viewport checked layout equivalent to 200% desktop zoom. Message-feed entries were browser-intercepted fixtures, not production messages. Desktop/mobile screenshots were inspected locally.

- Hard Gate PASS: working controls, keyboard/menu checks, both themes, 50 view/viewport combinations, text contrast at least **5.32:1**, and control borders at least **3.01:1**.
- Purpose Gate PASS: the sidebar separates operator tasks; cyan command, amber group, and green private labels distinguish activity, with text labels alongside colors. No decorative charts or invented traffic.
- Liveliness PASS: `DESIGN.md` records ENERGY 2 / RHYTHM 2 / MOTION 1; a forest console palette, connection focus, and timestamped message rows provide a consistent operator identity. Screenshots confirm desktop/mobile composition.
- Craftsmanship PASS: settings, queue/cancel, refresh, and authentication used the real offline API; fixture message content was rendered with `textContent`; tokens stayed in memory. The final binary reconnected the existing live WhatsApp session and exposed authenticated application logs without sending test messages to contacts.

## English dashboard and foreground terminal

The dashboard translation passed JavaScript syntax checking, `cargo fmt --check`, `cargo build --locked`, and the three targeted logging tests. The offline Chromium suite passed again: English HTML language, notifications, command descriptions, log labels and dates, settings, text/image queue cancellation, search/filter/pause, both themes, keyboard navigation, and all 50 view/viewport combinations. Changing reply language to Indonesian left the dashboard in English. Original Indonesian message fixtures stayed unchanged, and their HTML content remained escaped. Desktop/mobile screenshots were inspected.

- Hard Gate PASS: English labels and accessibility text were checked; keyboard controls, responsive widths, authentication, and log escaping passed the browser suite. Contrast remained at least 5.32:1 for text and 3.01:1 for control borders.
- Purpose Gate PASS: English copy names the existing operator actions and preserves their meaning; no new controls or layout were added.
- Liveliness PASS: the existing ENERGY 2 / RHYTHM 2 / MOTION 1 direction and activity colors remain consistent in both themes; screenshots show English labels fitting desktop/mobile layouts.
- Craftsmanship PASS: the rebuilt bot reconnected its existing live session. A visible terminal titled `WhatsApp Bot - Live message logs` runs the bot without stdout redirection; logging tests confirm timestamps, chat labels, optional colors, and safe previews. The offline test process was stopped after testing.

Real incoming WhatsApp message events were not generated during this check. Keep the foreground terminal open while running the bot; closing it stops the process.

## Compact terminal log format

The compact logger update passed `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo build --locked`. `cargo test --locked` passed **28 tests**, with the three existing FFmpeg/PostgreSQL/MongoDB integration tests ignored because those backends were unchanged.

Tests assert the six requested group/private message, command, info, and error formats with `HH:mm:ss` timestamps. They also check optional colors, control-character sanitization, media and missing-name fallbacks, phone-number preference, and preservation of unresolved LIDs. Group-name cache tests cover expiry, invalidation, failed lookups, the 512-entry bound, and prevention of stale queries overwriting newer names.

The rebuilt bot reconnected its existing WhatsApp session in one visible terminal. Real incoming messages and live group-name lookups were not generated during verification; unavailable names fall back to IDs. Keep the terminal open while the bot runs.

## Name fallback correction

The clarified format uses each name when present, falling back to a phone number or ID without appending both. Private entries omit the extra chat ID. All four logging tests passed, including whitespace-only names, sender and group fallbacks, unresolved LIDs, and slashes within actual group names. Formatting, Clippy with warnings denied, and the build passed. The rebuilt bot reconnected in the visible terminal. Real incoming messages were not generated for this check.

## Ping response time and runtime

The command update passed formatting, Clippy with warnings denied, JavaScript syntax checking, and `cargo build --locked`. `cargo test --locked` passed **29 tests**, with the three existing backend/media integration tests ignored. New uptime checks cover zero, minute/hour/day boundaries, and multiple days in both languages. Registry checks cover command/alias metadata and translated descriptions; Pong checks verify milliseconds are substituted without unresolved placeholders.

The rebuilt bot reconnected its existing session in one visible terminal. The authenticated commands API exposes `ping` and `runtime`, including the `uptime` alias and group/private scope. Real command delivery and the Pong edit were not exercised. Ping measures completion of the library's send operation, not recipient delivery/read time; process restarts reset runtime.

## Response-path optimization

The update passed `cargo fmt --check`, Clippy with warnings denied, and `cargo test --locked`: **30 tests passed**, with the three existing backend/media integration tests ignored. A new cache test verifies that messages can use an ID fallback without waiting for an in-flight group-name query, then use the cached name after it completes. Command handling now runs alongside logging, and contact/group persistence runs alongside the admitted handler; permissions, deduplication, cooldowns, and quotas remain checked before execution.

`cargo build --release --locked -j 4` passed, and the optimized executable returned version `0.1.0`. The local console launcher now points to that release executable. Automatic approval review rejected the attempted process replacement with `blocked by policy`; the debug process was left running and activation requires stopping it with Ctrl+C before starting the release. Live response times below the user's reported 3 ms were not measured, and no real test messages were sent. Ping still reports the actual library send duration; no timing values were adjusted.

## Windows MSVC linker warning suppression

`build.rs` now adds `/IGNORE:4099` to this package's MSVC linker invocation to suppress missing dependency PDB diagnostics. Formatting and Clippy with warnings denied passed. `cargo test --locked` on the installed Windows MSVC toolchain linked both test executables without `LNK4099` and passed **30 tests**, with the three existing backend/media integration tests ignored. A standalone build-script check verified that the flag is emitted for `CARGO_CFG_TARGET_ENV=msvc` and omitted for `gnu`.

The running debug bot was left active. Its main executable still needs to be rebuilt after stopping it with Ctrl+C; no full dependency cleanup is required. Suppression does not restore OpenSSL's missing PDB symbols. Other linker warning numbers and errors remain enabled.

## Sticker metadata and updater removal

`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked` passed: **34 tests passed**, with three integration tests ignored by default. The FFmpeg integration test was also run explicitly and passed for static and animated stickers, verifying embedded pack/author names and decoding the static output. Unit tests cover Unicode names, replacing existing EXIF, preservation of animation/alpha data, truncated containers, metadata-inclusive size limits, and configuration validation/defaults. A CLI test confirms that `run`, `secrets`, and `send` remain available while `update` is rejected. The updater dependency was removed from the manifest and lockfile.

The active configuration and local offline fixtures now use `[sticker]` and no longer contain `[update]`. The running bot still uses its previous binary; stop it, rebuild, and restart to activate these changes. Pack/author display in the WhatsApp client was not tested, and no live messages were sent. PostgreSQL and MongoDB integration tests were not rerun for this change.

## Animated sticker size and command error diagnostics

A six-second noisy video reproduced the previous failure: conversion at quality 50/12 FPS produced 5,282,864 bytes and was rejected by the 500 KiB limit. The regression test failed before the fix and passed afterward. Animated conversion now retries at lower quality/FPS only when oversized, reserves 2 KiB for validated pack/author metadata, and shares one conversion deadline across attempts. Downloads with missing declared file length still enforce the writer and final byte limits.

Formatting and Clippy with warnings denied passed. `cargo test --locked` passed **35 tests**, with four integration tests ignored by default. `cargo test --locked bot::messaging::tests -- --ignored` passed both FFmpeg tests: static/short animated conversion and the six-second busy video with maximum-length escaped pack/author names. Coverage also includes malformed video diagnostics, conversion timeout, and sanitized `[ERR]` details in both console and dashboard logs. The running bot was not restarted and the user's actual video was not tested through WhatsApp. Compression may reduce quality and frame rate; exceptionally complex clips may still require shortening. PostgreSQL/MongoDB integration tests were not rerun.

## Dashboard command and sticker management

Formatting, JavaScript syntax checking, and Clippy with warnings denied passed. `cargo test --locked` passed **39 tests**, with four integration tests ignored by default. Both FFmpeg integration tests also passed when run explicitly. New checks cover edited names/aliases and permissions, deleted command resolution, collisions, invalid values, owner restrictions, authenticated management routes, database persistence, and general settings saves preserving command/sticker overrides. Legacy settings JSON without the new fields still loads with defaults.

An isolated SQLite dashboard ran offline on port 3400 with synthetic settings and separate test secrets/data. Chromium exercised Edit/Save/Cancel, Escape, confirmed and cancelled deletions, owner checkbox restrictions, escaped descriptions, sticker saves, validation errors, logout/login, automatic refresh, and unsaved form preservation. A process restart retained the renamed command, deleted command, prefix, and pack/author names; dashboard reload returned the persisted values. Production settings and sessions were unchanged.

The existing design remains at ENERGY 2 / RHYTHM 2 / MOTION 1. The editor reuses form/card styles; Delete uses the existing error color and a confirmation naming the affected command. Layout checks passed at 320, 390, 768, 1024, and 1280 pixels in both themes, with wide-table scrolling contained inside the table. Keyboard focus, 200% CSS zoom/reflow, reduced motion, an empty command list, and a shortened viewport simulating keyboard space passed. New action text contrast is at least 6.57:1 and component borders at least 3.27:1. Command/sticker screenshots were inspected; temporary scripts and screenshots were removed during repository cleanup. No imagery or navigation destinations were added.

The running production bot needs a rebuild/restart to activate this feature. Editing changes command configuration and keeps the Rust handler; deleting persists removal from the active registry without removing source files. Real WhatsApp execution and sticker metadata display were not tested. PostgreSQL/MongoDB integration tests and physical mobile keyboards were not rerun/tested. All backends continue using the existing encrypted settings record.

## Automatic language generation

Formatting, JavaScript syntax checking, Clippy with warnings denied, and `cargo build --locked -j 1` passed. `cargo test --locked -j 1` passed **43 tests**, with the four existing media/database integration tests ignored. New tests cover provider language filtering, complete catalog generation, concurrent requests generating once, exact placeholder preservation/interpolation, cached use without a provider, invalid locale codes/URLs/catalogs, stale source fingerprints, failure without partial files, authenticated language lookup, and failed generation preserving persisted bot settings.

The built executable ran offline in an isolated SQLite directory against a local mock implementing the LibreTranslate HTTP contract. Chromium exercised selecting/saving a new language, generation failure keeping the previous selection in the database, the reload button, and language readiness. The actual `generate-locales --all` CLI kept successful catalogs after a failing language and completed when retried. After shutting down the mock, cached selection worked without translation requests; restarting the offline bot restored the selected language and catalogs. Mock output was explicitly labeled and was not installed in production data. Temporary mock catalogs were removed during repository cleanup.

The existing ENERGY 2 / RHYTHM 2 / MOTION 1 design was preserved. Controls, keyboard focus, reduced motion, and overflow checks passed in both themes at 320, 390, 768, and 1280 pixels. Screenshots were inspected after theme transitions settled. Language helper text has at least 6.16:1 contrast, control text at least 6.57:1, and borders at least 3.27:1. No imagery or navigation destinations were added. Compose configuration validation passed for the optional translation profile, and the official image tag `v1.9.6` was verified.

The local `.env` now points to LibreTranslate on port 5000; other environment values and production data/session files were preserved. Docker's Linux engine was unavailable, so the actual LibreTranslate container/model downloads, translation quality, and live WhatsApp replies were not tested. PostgreSQL/MongoDB and physical mobile keyboards were not rerun/tested. Generated language availability follows the server's English translation targets; literal segments around placeholders can need review for grammar.

## Debugging message formats

The debugging category registers `testbutton`, `testlist`, `testinteractive`, and `sendhtml` as owner-only commands for both chat scopes. `cargo test --locked -j1` passed 46 tests (45 library, one CLI), with four existing FFmpeg/database tests ignored. New checks cover native flow types and JSON action IDs, group/private stanza nodes, rich HTML response ID/payload/size limits, and native/legacy button/list response parsing. Malformed, oversized, and unsupported native response JSON is rejected. Registry, bundled i18n descriptions, permissions, and dashboard API tests also passed. `cargo fmt --check` and `cargo clippy --locked --all-targets -j1 -- -D warnings` passed.

`cargo build --locked -j1` also passed, producing the updated native Windows binary. No real WhatsApp messages were sent. Delivery and client rendering of native flow/rich HTML remain unverified; account/client support can differ. Existing ignored FFmpeg/PostgreSQL/MongoDB checks were not rerun. New translation keys invalidate previously generated catalog fingerprints, requiring regeneration for those languages.

## Rich HTML stanza correction

Compared with Chisato's Baileys relay, whatsapp-rust 0.7 classified `botForwardedMessage` as `media` without a media type. The debugging sender now explicitly selects `text` for this envelope. Regression tests reproduce the default classification and verify the override in group/private chats, preserve native flow routing, and round-trip HTML and bot metadata through the actual group, recipient, and own-device plaintext encoders. `cargo test --locked -j1` passed 48 tests, with four existing FFmpeg/database tests ignored. Formatting and Clippy checks passed. A temporary example using the application's unchanged `main.rs` compiled and passed `--help`, checking the Windows executable without replacing the running bot. The temporary source was removed. A new bot process was subsequently observed using the rebuilt application binary. Live Android rendering of the correction has not been tested.

## Built-in HTML demo

`sendhtml` now builds the fixed WhatsApp BOT by TobyG74 demo without taking HTML arguments. The fallback title uses the same credit. Bundled command descriptions and deployment instructions were updated in English and Indonesian. The three existing HTML tests now exercise the built-in layout, including the text stanza override and group/private encoding. `cargo test --locked -j1 bot::commands` passed all 12 command tests; formatting and Clippy checks passed. The demo was previewed in Chromium and the mobile screenshot was inspected. No live WhatsApp messages were sent; Android may render inline styles differently. Rebuild/restart to use the new demo. Changing the English catalog invalidates generated language caches; regenerate those languages if used.

- Hard gate PASS: Chromium checks at 320, 390, and 1280px found no overflow; text contrast meets AA; the static demo has no scripts, external assets, or interactive controls.
- Purpose gate PASS: the green border reflects the WhatsApp theme; spacing separates the bot name, author credit, and formatting example; no gradients, shadows, or decorative icons were added.
- Liveliness PASS: ENERGY 1 / RHYTHM 1 / MOTION 1; the heading provides the focal point, with one green border accent and no animation.
- Craftsmanship PASS: title/author match the requested text; screenshots and browser checks confirm a readable layout, and 12 command tests plus Clippy passed.

## Menu roles and automatic typo suggestions

The menu now groups active commands into categories with role and chat-scope markers, localized headings/legend, descriptions, aliases, and current prefix/mode/role. Tests check user/owner visibility, private/group scope, custom categories/descriptions, renamed aliases, deleted commands, and both bundled languages. Automatic suggestions compare bounded ASCII command tokens to current names/aliases, support adjacent swaps, and return up to three equally close primary names. Tests cover insertions/deletions/substitutions/swaps, case, prefixes, irrelevant/invalid/oversized tokens, tie ordering, owner and debugging restrictions, self mode, chat scope, dashboard edits, and deletions. Localized suggestion placeholders are checked in English and Indonesian.

The dispatcher applies message deduplication and a separate five-second sender cooldown with zero quota cost before replying. An isolated SQLite test checks deduplication, concurrent cooldown admission, and preserving quota, including suggestions at the daily quota boundary. `cargo test --locked -j1` passed 53 tests (52 library, one CLI), with four existing FFmpeg/database checks ignored; `cargo fmt --check` and `cargo clippy --locked --all-targets -j1 -- -D warnings` passed. No production configuration, keys, sessions, or databases were changed; no live WhatsApp messages were sent. Rebuild/restart to apply the implementation. Live menu rendering and suggestion replies have not been verified. New English catalog keys require regeneration of generated language caches.

## Menu category filter

`menu [category]` now forwards its arguments to the menu builder and filters the already-authorized command groups. Without arguments, all accessible categories remain visible. The filter accepts mixed case, surrounding whitespace, and custom multiword category names. An unavailable category shows a localized notice and lists only accessible categories. The usage hint follows the configured prefix and menu name. A regression test covers filtering, owner/private/group restrictions, custom dashboard categories, renames, deletions, aliases, and both bundled locales. `cargo test --locked -j1 bot::commands` passed all 18 command tests; formatting and Clippy checks passed. No live WhatsApp menu replies were sent. Rebuild/restart to apply the change, and regenerate cached generated languages after the English source catalog changes.

## Personal languages and feature controls

`setlang` is now a general user command. The dispatcher resolves the sender's encrypted personal preference for replies, descriptions, suggestions, and errors, falling back to the bot default. SQLite/PostgreSQL create the preference table at startup; MongoDB uses a separate encrypted collection. Choices do not replace global settings. Owner commands `setting`, `setlimit`, and `setcooldown` persist feature switches, daily quota, and an optional global cooldown. Dashboard saves preserve feature values omitted by the form. Automatic message/status reads run alongside command handling using the library's read-receipt API; outgoing messages, newsletters, and old events are skipped.

The default suite passed 61 tests, and all four FFmpeg/PostgreSQL/MongoDB integration tests passed against isolated temporary databases. Tests cover legacy defaults, owner access, toggle parsing, quota/cooldown bypass without resetting history, free commands at exhausted quota, language isolation, encrypted preferences surviving SQLite restart, PostgreSQL/MongoDB preference storage, and dashboard preservation. Formatting and Clippy without warnings passed. The translation test now compares concurrent generation with a single-generation reference rather than a fixed batch count, accommodating the larger source catalog.

Temporary browser fixtures/screenshots, extracted upstream archives, empty directories, installers, and test launcher/log/linker artifacts were removed. Active tool installations, production data, configuration, and the running bot were retained. Forced removal of the remaining upstream Git clone was rejected by automatic approval review (`blocked by policy`); it remains ignored by Git. Active secret values were absent from the 62 prospective Git files checked. No commit or push was performed.

Real WhatsApp command replies/status views were not exercised. Rebuild/restart to activate the changes. Generated language catalogs require regeneration after source changes; bundled English/Indonesian remain available.

## Earlier baseline limitations

Live WhatsApp pairing/delivery, MongoDB Atlas/SRV/TLS, production database credentials, Docker images, and GitHub CI were not tested. All runtime messages were offline fixtures and were cancelled. Test secrets were not used for production configuration.

MongoDB limits media to 8 MiB because each encrypted payload uses a single document; GridFS is not used. WhatsApp sessions use SQLite SQLCipher with every backend. Switching databases does not migrate data automatically.
