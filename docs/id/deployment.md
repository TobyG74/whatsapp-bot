# Deployment Linux dengan systemd

[English](../en/deployment.md) | [README](../../README.md)

## Menu dan saran typo command

`!setlang en` menjadi command pengguna yang menyimpan bahasa balasan pengirim di database terenkripsi, berlaku di grup/private dan tetap tersimpan setelah restart. Pengguna yang belum memilih memakai bahasa default dashboard. `id`/`en` bawaan tidak membutuhkan provider; bahasa lain yang didukung di-generate sebelum disimpan. Mode self tetap membatasi semua command untuk owner. Jika bahasa pribadi tidak tersedia, bot sementara memakai bahasa default tanpa menghapus pilihan pengguna; `setlang` tetap bisa dipakai untuk menggantinya.

Owner dapat memakai `!setting` untuk melihat fitur atau `!setting <fitur> <on|off>` untuk mengatur `autocorrect`, `use_limit`, `use_cooldown`, `autoreadmessage`, dan `autoreadstatus`. Tiga fitur pertama default on; baca otomatis default off. Perubahan tersimpan dan berlaku pada event berikutnya. Menyimpan pengaturan umum dari dashboard mempertahankan toggle ini. `!setlimit 50` mengatur kuota harian tiap non-owner (1..10000) tanpa mereset pemakaian. `!setcooldown 3` mengatur cooldown global (0..86400 detik); `!setcooldown default` mengembalikan cooldown masing-masing command. Limit off melewati pemeriksaan/biaya kuota tanpa menghapus pemakaian lama; cooldown off melewati cooldown command dan saran typo. Command gratis tetap bisa dipakai saat kuota habis.

Baca otomatis pesan mencakup pesan biasa/command yang masuk di grup/private tanpa bergantung pada izin command maupun mode bot. Status memakai toggle terpisah. Keduanya mengabaikan pesan sendiri, newsletter, dan event lebih lama dari lima menit. Pengiriman read receipt berjalan bersamaan dengan handler command dan mengikuti pengaturan privasi read receipt akun WhatsApp.

Gunakan `!menu` untuk seluruh command yang tersedia, atau `!menu debugging` untuk satu kategori. Nama kategori bawaan lainnya adalah `general`, `media`, dan `owner`. Filter tidak membedakan huruf besar/kecil dan mendukung nama kategori dashboard yang memakai spasi, misalnya `!menu Custom tools`. Alias juga bisa dipakai: `!help media`. Balasan hanya mencantumkan kategori yang dapat diakses pengirim pada chat tersebut, termasuk saat kategori yang diminta tidak cocok. Petunjuk penggunaan mengikuti prefix dan nama menu yang dikonfigurasi.

`!menu` mengelompokkan command aktif berdasarkan kategori dan menampilkan deskripsi, alias, prefix, mode, serta role kamu. 👤 menandai command pengguna dan 👑 menandai command owner. 🌐 berarti grup/private, 👥 khusus grup, dan 💬 khusus private. Command yang tidak dapat dipakai oleh pengirim atau pada chat tersebut disembunyikan. Isi menu mengikuti edit dan penghapusan dari dashboard.

Autocorrect bekerja otomatis: kirim `!pnig` untuk mendapat saran `!ping`, atau `!hepl` untuk `!menu`. Nama command dibandingkan dengan nama utama dan alias saat ini; maksimal tiga nama utama dengan kecocokan terbaik ditampilkan memakai prefix kamu. Ketik ulang command yang disarankan untuk menjalankannya. Saran mengikuti izin owner, jenis chat, dan mode self; mengabaikan teks yang tidak relevan serta pesan duplikat; tidak memakai kuota; dan memiliki cooldown terpisah lima detik per pengirim. Build ulang/restart untuk menerapkan perubahan. Key baru pada katalog Inggris memerlukan generate ulang bahasa yang memakai cache hasil generate; `id`/`en` bawaan langsung tersedia.

## Command debugging pesan

Setelah build ulang/restart, owner dapat memakai `!testbutton` (`button`), `!testlist` (`list`), `!testinteractive` (`interactive`), dan `!testhtml` (`sendhtml`) di grup maupun private. Setiap command mengirim contoh ke chat saat ini, dengan cooldown 3 detik yang sama untuk command/aliasnya. `!testhtml` langsung mengirim demo HTML **WhatsApp BOT by TobyG74**, tanpa perlu argumen; argumen tambahan diabaikan. Untuk menyesuaikan demo, edit `TITLE` dan `HTML` di `src/bot/commands/debugging/testhtml.rs`, lalu build ulang/restart. Demo memakai latar terang, aksen hijau, kredit pembuat, serta contoh teks tebal/miring dengan CSS inline tanpa aset eksternal atau script. Button/list memakai prefix dan nama aksi yang dikonfigurasi; respons tetap melalui pemeriksaan izin, cooldown, serta kuota command biasa.

`testhtml` (alias `sendhtml`) memakai stanza `text` secara eksplisit agar sesuai dengan Chisato/Baileys. Klasifikasi bawaan whatsapp-rust 0.7 adalah `media` tanpa jenis media dan dapat membuat pesan tidak terlihat. Build ulang dan restart bot untuk menerapkan perbaikan ini.

Format native flow/rich response ini bergantung pada dukungan akun/klien WhatsApp. Tampilan pesan pada WhatsApp nyata belum diverifikasi. Lihat [contoh format pesan](../../README.md#debugging-message-formats). Build ulang mengubah fingerprint katalog sumber, sehingga bahasa hasil generate sebelumnya perlu dibuat ulang dengan LibreTranslate tersedia sebelum dipakai; `id`/`en` bawaan tidak memerlukan generate.

## Log terminal

Startup menampilkan banner ASCII **WhatsApp BOT** berwarna hijau dengan kredit `Github: TobyG74` yang lebih redup di bawahnya. Saat dashboard dimulai, `[INF] Dashboard running on port 3000 | Open http://127.0.0.1:3000` menunjukkan port yang benar-benar dipakai dan URL untuk browser. Nilainya mengikuti konfigurasi; bind wildcard memakai URL loopback dan port `0` menampilkan port pilihan OS. `NO_COLOR`/`CLICOLOR_FORCE` juga berlaku pada banner. `LOG_FORMAT=json` melewati banner agar output tetap terstruktur.

Build Windows MSVC menyembunyikan warning linker `LNK4099` tentang file PDB dependensi yang tidak tersedia melalui `build.rs`. Simbol debug OpenSSL tersebut tetap tidak tersedia; warning dan error linker lainnya tetap terlihat. Jika Cargo menampilkan ulang warning dari build sebelumnya, hentikan bot lalu jalankan `cargo build --locked` sekali agar paket dibuild ulang dengan opsi baru. Tidak perlu menjalankan `cargo clean`.

Hentikan proses bot yang sudah berjalan sebelum menjalankan proses lain dengan data directory yang sama. Jalankan binary hasil build langsung di terminal tanpa mengalihkan stdout:

```sh
RUST_LOG=whatsapp_bot=info,whatsapp_rust=error,wacore=error LOG_FORMAT=pretty ./whatsapp-bot --config config.toml run
```

Biarkan terminal terbuka. Event command dan pesan biasa grup/private yang baru muncul dengan waktu `HH:mm:ss` serta tag `[CMD]`/`[MSG]` berwarna. Pengirim ditampilkan dengan nama; jika nama kosong, gunakan nomor telepon atau ID. Chat grup menambahkan panah dan nama grup, atau ID grup jika namanya kosong. Chat private cukup menampilkan pengirim. Nama dan identitas tidak digabung; tanda garis miring yang memang ada dalam nama tetap dipertahankan. Info/error memakai `[INF]`/`[ERR]`, sedangkan warning memakai `[WRN]`. Ctrl+C menghentikan bot. Contoh PowerShell dan format output tersedia pada README. Dashboard memakai bahasa Inggris secara terpisah dari bahasa balasan bot; isi pesan asli ditampilkan sesuai yang diterima.

Nama grup disimpan dalam cache selama lima menit (maksimal 512 entri), dengan invalidasi saat nama grup berubah. Pencarian baru dibatasi dua detik; entri log dapat tertunda sementara handler command berjalan bersamaan. Jika gagal, log memakai ID grup dan menyimpan hasil tersebut sementara. Entri penyelesaian command dapat muncul sebelum log pesannya ketika pencarian berlangsung. Media tanpa caption memakai penanda jenis seperti `[image]`. LID yang belum terpetakan tetap menyertakan `@lid` dan tidak dianggap sebagai nomor telepon.

Jika command stiker gagal, periksa `[ERR] Command failed` pada log terminal/dashboard. Error konversi FFmpeg menyertakan stderr yang telah disanitasi; kegagalan download, upload, dan pengiriman menunjukkan tahap yang gagal. `[ERR] Command timed out` berarti batas waktu operasi terlampaui. Stiker animasi yang terlalu besar dikonversi ulang dengan kualitas/FPS lebih rendah dalam batas waktu yang sama, dengan ruang disisihkan untuk metadata pack/author. Batas akhir tetap 500 KiB. Build ulang dan restart setelah memasang perbaikan ini; binary yang sudah berjalan tetap memakai perilaku sebelumnya.

## Command waktu respons dan runtime

`!ping` (alias `!p`) mengirim balasan sementara, lalu mengeditnya untuk menampilkan durasi pengiriman dalam milidetik. Pengukuran mencakup persiapan/pengiriman melalui library, bukan waktu pesan diterima atau dibaca. `!runtime` (alias `!uptime`) menampilkan lama proses bot berjalan dalam hari, jam, menit, dan detik. Restart proses mereset runtime; reconnect WhatsApp dalam proses yang sama tidak meresetnya. Gunakan prefix konfigurasi Anda jika sudah diubah dari `!`. Kedua command dapat dipakai di grup/private sesuai mode bot dan memakai bahasa pilihan pengirim, atau bahasa default bot jika belum memilih.

Gunakan `cargo run --release --locked -- --config config.toml run` untuk build dengan optimasi. Penyimpanan kontak/grup berjalan bersamaan dengan handler setelah pemeriksaan izin, deduplikasi, serta kuota/cooldown. Angka waktu kirim tidak mencakup pemeriksaan sebelumnya; performa dipengaruhi cache, CPU, disk, dan jaringan.

## Layanan systemd

Untuk terjemahan balasan otomatis, jalankan layanan opsional dengan `docker compose --profile translation up -d translate`, lalu isi `LIBRETRANSLATE_URL=http://127.0.0.1:5000` dalam `.env` bot. Bot di jaringan Compose memakai `http://translate:5000`. LibreTranslate memuat model bahasa yang terpasang; tunggu layanan siap sebelum generate katalog. API key bersifat opsional kecuali server mewajibkannya; simpan dalam `LIBRETRANSLATE_API_KEY`.

Docker bersifat opsional. Untuk menjalankan layanan langsung, ikuti [instalasi Python resmi](https://docs.libretranslate.com/guides/installation/). Di Linux, buat virtual environment dengan `python3 -m venv .tools/libretranslate`, pasang dengan `.tools/libretranslate/bin/python -m pip install libretranslate==1.9.6`, lalu jalankan `.tools/libretranslate/bin/libretranslate --host 127.0.0.1 --port 5000`. Biarkan layanan berjalan selama generate; startup pertama mengunduh model bahasa. Perintah PowerShell Windows tersedia pada [README](../../README.md#automatic-reply-translations). Bot memakai URL localhost yang sama untuk layanan langsung maupun Docker. Instalasi langsung/pemuatan model belum diverifikasi pada komputer ini.

Setelah build ulang/restart bot, pilih **Settings > Reply language > Save settings**. Katalog yang belum tersedia dibuat otomatis sebelum pengaturan bahasa diganti, dan tiap pengguna dapat memakai `!setlang <kode>` untuk bahasa pribadinya. **Reload language list** memperbarui pilihan. Untuk menyiapkan semua bahasa yang didukung dari bahasa Inggris, jalankan `cargo run --locked -- generate-locales --all`; gunakan `--language fr,ja,ar` untuk beberapa kode pilihan. File terjemahan tidak perlu dibuat sendiri.

Backup `data/i18n`, tempat katalog disimpan secara atomik. Balasan dari cache tetap bekerja saat layanan mati; kegagalan generate mempertahankan pengaturan sebelumnya. Build ulang setelah mengubah `locales/en.json` membuat katalog lama kedaluwarsa, sehingga LibreTranslate perlu tersedia untuk generate ulang. Dashboard tetap berbahasa Inggris; fitur ini menerjemahkan template bawaan bot. Panduan dan batasan lengkap ada pada [README](../../README.md#automatic-reply-translations).

Build release sesuai README. Buat user layanan dan directory aplikasi, lalu salin binary, `config.toml`, dan `.env` ke `/opt/whatsapp-bot`. User layanan perlu izin menulis pada directory data dan lokasi sesi yang dikonfigurasi. Batasi akses `.env` (`chmod 600`) dan simpan key di secret manager/backup privat.

Isi `[sticker]` dengan `packname = "WhatsApp Bot"` dan `authorname = "Your Name"` pada `config.toml` sebagai nilai awal pack dan author stiker dari gambar/video. Kedua field mendukung maksimal 256 byte UTF-8 tanpa karakter kontrol. Untuk mengubahnya langsung, gunakan **Settings > Sticker settings** pada dashboard. Nilai yang disimpan melalui dashboard menggantikan nilai awal file dan tetap tersimpan setelah restart pada database aplikasi terenkripsi. File WebP yang dikirim melalui API tetap memakai metadata aslinya. Hapus bagian `[update]` dari konfigurasi lama.

### Mengelola command dan stiker

Setelah memasang binary hasil build terbaru, masuk ke dashboard. Pada **Commands**, gunakan **Edit** untuk mengubah nama, alias, kategori, deskripsi khusus, cooldown, biaya kuota, cakupan grup/private, dan pembatasan owner. **Save command** menerapkan perubahan pada pesan berikutnya tanpa restart. Deskripsi khusus yang kosong memakai terjemahan bawaan. Nama/alias harus unik, memakai huruf kecil, dan diawali huruf; command owner tetap wajib membatasi akses owner.

**Delete** meminta konfirmasi, lalu menghapus command dan aliasnya dari dispatcher, menu, dan daftar dashboard. File handler Rust tetap tersedia dalam repository. Penghapusan bertahan setelah restart layanan, sedangkan command yang sudah berjalan dapat menyelesaikan prosesnya. Command yang dihapus dapat dipulihkan melalui `PUT /api/commands/{id-asli}` terautentikasi dengan konfigurasinya dan `deleted: false`; daftar field tersedia pada README.

Gunakan **Settings > Sticker settings > Save sticker settings** untuk mengubah metadata pack/author pada stiker baru. Perubahan command dan stiker memakai record pengaturan terenkripsi yang sama dengan pengaturan umum bot pada semua backend database. Database lama tidak memerlukan migrasi skema baru. Menyimpan pengaturan umum tidak mereset perubahan ini. Binary yang masih berjalan perlu dibuild ulang dan direstart sekali untuk mengaktifkan fitur dashboard ini.

### Unit layanan

Contoh unit `/etc/systemd/system/whatsapp-bot.service`:

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

Dashboard tetap bind `127.0.0.1:3000`. Akses lokal melalui SSH tunnel:

```sh
ssh -L 3000:127.0.0.1:3000 user@server
```

Isi `LOG_FORMAT=json` pada `.env` untuk timestamp lengkap dan log layanan terstruktur, atau gunakan `LOG_FORMAT=pretty` untuk waktu ringkas `HH:mm:ss` serta label chat yang mudah dibaca. Format pretty memakai zona waktu lokal server; terminal interaktif mendapat warna ANSI. `NO_COLOR=1` mematikan warna dan `CLICOLOR_FORCE=1` memaksanya pada output yang dialihkan. Dashboard menampilkan waktu sesuai zona waktu browser.

Log kini menyertakan pratinjau maksimal 300 karakter pesan, identitas pengirim, serta JID grup/private, termasuk pesan yang diabaikan dalam mode self. Pratinjau berupa plaintext pada stdout dan buffer dashboard terautentikasi berisi 200 entri di memori. Batasi akses file log dan atur rotasi/retensi melalui pengelola layanan. Buffer kosong saat restart; ini bukan arsip pesan atau bukti penerimaan. Status broadcast, newsletter, dan event lebih lama dari lima menit tidak dicatat.

Atau gunakan HTTPS reverse proxy dengan domain dan sertifikat Anda. Sediakan `client_max_body_size` yang cukup untuk base64 media (default 16 MiB membutuhkan sekitar 22 MiB request JSON). Batasi akses jaringan dashboard. Jangan expose database atau token melalui HTTP publik.

Update dilakukan manual: hentikan layanan dengan `sudo systemctl stop whatsapp-bot`, pasang binary hasil build beserta konfigurasi/migrasi yang diperlukan, lalu jalankan `sudo systemctl start whatsapp-bot`. Aplikasi tidak lagi memiliki updater otomatis maupun perintah CLI `update`.

Backup database, session SQLCipher, dan encryption key sebelum upgrade. Proyek ini ditujukan untuk satu proses per akun/database, bukan cluster multi-server yang berbagi sesi.

Untuk MongoDB, isi `database.driver = "mongodb"`, nama database dalam URI, dan `media.max_bytes = 8388608` pada `config.toml`. Simpan URI berkredensial di `DATABASE_URL` dalam `.env`. User database membutuhkan izin baca/tulis dan membuat index pada database aplikasi; role `readWrite` pada database tersebut mencakup kebutuhan ini. Gunakan autentikasi, TLS untuk koneksi jaringan, dan backup `mongodump` beserta key. Sesi WhatsApp tetap berada pada file SQLCipher lokal, sehingga backup MongoDB saja belum mencakup sesi bot.
