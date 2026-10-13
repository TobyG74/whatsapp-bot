# Catatan verifikasi lokal

[English](../en/verification.md) | [README](../../README.md)

Tanggal: 12 Oktober 2026. Pengujian memakai Windows, Rust stable 1.99 target GNU, PostgreSQL 17.6, MongoDB 8.0.15 standalone, FFmpeg, dan Chromium melalui Playwright. Toolchain pengujian lokal memakai OpenSSL dari Strawberry Perl; build dengan OpenSSL vendored pada Linux/MSVC belum dijalankan lokal.

| Pemeriksaan | Hasil |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --locked --all-targets -- -D warnings` | PASS |
| `cargo test --locked -- --include-ignored` | PASS: 23 tes, tanpa kegagalan/tes diabaikan |
| `cargo build --locked` | PASS |
| `node --check src/web/assets/app.js` | PASS |
| Startup `run --offline` dengan MongoDB | PASS: `/health` merespons `ok` |
| CLI `send` melalui dashboard, kemudian cancel | PASS: job pending menjadi cancelled |
| Proses kedua pada data directory yang sama | PASS: ditolak oleh file lock |
| Restart proses MongoDB/offline | PASS: prefix, bahasa, mode, dan tiga job cancelled tetap tersimpan |

Tes MongoDB menggunakan database sementara `bot_test_<UUID>` yang dihapus sesudah tes. Cakupannya: pengaturan, kontak/grup, indeks JID privat, enkripsi payload, deduplikasi paralel, cooldown, limit lintas command, reset harian, owner dengan biaya nol, admission dari dua koneksi, klaim antrean paralel, pembatalan, restart saat status sending, payload rusak, cleanup record lama, media 8 MiB, dan penolakan payload berlebihan. SQLite/PostgreSQL serta SQLCipher juga diuji. Tes FFmpeg menghasilkan WebP dari gambar satu frame dan video pendek.

## Dashboard sebelum pembaruan aktivitas pesan

Tes browser menjalankan login token salah/benar, simpan konfigurasi, antrekan teks terjadwal dan gambar, batalkan kedua job, error reconnect offline, refresh, setiap tautan navigasi, logout, serta aktivasi tombol dan indikator fokus melalui keyboard. Tidak ada error JavaScript. Lebar viewport 320, 390, 768, 1024, dan 1280 px tidak menyebabkan overflow halaman; tabel panjang memakai area scroll. Screenshot desktop/mobile diperiksa lokal.

Palet menggunakan teks gelap dan satu aksen hijau untuk kontrol operator. System font memudahkan keterbacaan tanpa asset eksternal; bagian mengikuti kebutuhan koneksi, pengiriman, pengaturan, antrean, command, dan log. Tema terang memudahkan membaca tabel/log; tampilan ini tidak memiliki toggle tema. ENERGY 1 / RHYTHM 1 / MOTION 1, dengan gerak hanya pada tombol dan dukungan reduced motion.

- Hard Gate PASS: kontrol diuji di browser, state kosong/loading/error tersedia, navigasi keyboard dan viewport diperiksa; pasangan warna teks yang diperiksa memiliki kontras minimal 6.03:1, border kontrol 4.86:1.
- Purpose Gate PASS: aksen hijau menandai tindakan; panel berbatas memisahkan bagian operator; tidak ada ilustrasi, statistik pelanggan, atau testimonial.
- Liveliness PASS: status koneksi menjadi fokus pemantauan, whitespace memisahkan tugas, hierarki judul dan palet hijau konsisten dengan panel operator yang tenang.
- Craftsmanship PASS: formulir menyimpan data nyata ke MongoDB; antrean/cancel/refresh/logout menjalankan API; statistik berasal dari proses bot; screenshot dan tes browser menjadi bukti pemeriksaan.

## Pembaruan aktivitas pesan dan dashboard

Pembaruan berikutnya pada 12 Oktober 2026 lolos `cargo fmt --check`, Clippy tanpa warning, `cargo build --locked`, dan pemeriksaan sintaks JavaScript. `cargo test --locked` meloloskan **24 tes**, dengan tiga tes integrasi FFmpeg/PostgreSQL/MongoDB tetap diabaikan karena backend tersebut tidak berubah. Tes baru memeriksa command ber-prefix, pemotongan pratinjau Unicode, pembersihan karakter kontrol terminal, timestamp dan warna ANSI opsional, field yang boleh dicatat, serta buffer 200 entri.

Tes Chromium pada bot SQLite terpisah dalam **mode offline** meloloskan login salah/benar, penyimpanan konfigurasi, antrean/pembatalan teks terjadwal dan gambar, error reconnect offline, pencarian command, pencarian/filter jenis pesan dan chat, jeda/lanjut, state kosong, escaping injeksi HTML, persistensi tema, logout, menu mobile, Escape, fokus keyboard, dan reduced motion. Kedua tema dan lima halaman diperiksa pada lebar 320, 390, 768, 1024, dan 1280px tanpa overflow halaman; tabel lebar memiliki area scroll sendiri. Viewport 640px memeriksa layout yang setara dengan zoom desktop 200%. Data aktivitas pesan memakai fixture yang diintersep browser, bukan pesan produksi. Screenshot desktop/mobile diperiksa lokal.

- Hard Gate PASS: kontrol berfungsi, pemeriksaan keyboard/menu, kedua tema, 50 kombinasi halaman/viewport, kontras teks minimal **5.32:1**, dan border kontrol minimal **3.01:1**.
- Purpose Gate PASS: sidebar memisahkan pekerjaan operator; label command cyan, grup amber, dan private hijau membedakan aktivitas dengan teks pendamping. Tidak ada grafik dekoratif atau traffic rekaan.
- Liveliness PASS: `DESIGN.md` mencatat ENERGY 2 / RHYTHM 2 / MOTION 1; palet forest, fokus koneksi, dan baris pesan bertimestamp membentuk tampilan operator yang konsisten. Screenshot menunjukkan komposisi desktop/mobile.
- Craftsmanship PASS: konfigurasi, antrean/cancel, refresh, dan autentikasi memakai API offline nyata; fixture pesan dirender dengan `textContent`; token tetap di memori. Binary final tersambung kembali memakai sesi WhatsApp aktif dan menyediakan log aplikasi terautentikasi tanpa mengirim pesan tes ke kontak.

## Dashboard Inggris dan terminal foreground

Terjemahan dashboard lolos pemeriksaan sintaks JavaScript, `cargo fmt --check`, `cargo build --locked`, dan tiga tes logging terarah. Suite Chromium offline kembali lolos: bahasa HTML Inggris, notifikasi, deskripsi command, label/tanggal log, konfigurasi, pembatalan antrean teks/gambar, pencarian/filter/jeda, kedua tema, navigasi keyboard, dan 50 kombinasi halaman/viewport. Mengubah bahasa balasan menjadi Indonesia tetap mempertahankan dashboard Inggris. Fixture pesan asli berbahasa Indonesia tidak berubah dan konten HTML-nya tetap di-escape. Screenshot desktop/mobile diperiksa.

- Hard Gate PASS: label dan teks aksesibilitas Inggris diperiksa; keyboard, ukuran layar, autentikasi, dan escaping log lolos tes browser. Kontras tetap minimal 5.32:1 untuk teks dan 3.01:1 untuk border kontrol.
- Purpose Gate PASS: teks Inggris menjelaskan tindakan operator yang sama dan mempertahankan artinya; tidak ada kontrol atau layout baru.
- Liveliness PASS: arah ENERGY 2 / RHYTHM 2 / MOTION 1 serta warna aktivitas tetap konsisten pada kedua tema; screenshot menunjukkan label Inggris muat pada desktop/mobile.
- Craftsmanship PASS: bot hasil rebuild tersambung kembali dengan sesi aktif. Terminal terlihat berjudul `WhatsApp Bot - Live message logs` menjalankan bot tanpa pengalihan stdout; tes logging memastikan timestamp, label chat, warna opsional, dan pratinjau aman. Proses tes offline dihentikan setelah pengujian.

Event pesan masuk WhatsApp nyata tidak dibuat dalam pemeriksaan ini. Biarkan terminal foreground terbuka selama bot berjalan; menutupnya menghentikan proses.

## Format log terminal ringkas

Pembaruan logger ringkas lolos `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, dan `cargo build --locked`. `cargo test --locked` meloloskan **28 tes**, dengan tiga tes integrasi FFmpeg/PostgreSQL/MongoDB tetap diabaikan karena backend tersebut tidak berubah.

Tes memeriksa enam format pesan grup/private, command, info, dan error sesuai permintaan dengan timestamp `HH:mm:ss`. Tes juga mencakup warna opsional, pembersihan karakter kontrol, fallback media dan nama kosong, prioritas nomor telepon, serta LID yang belum terpetakan. Tes cache nama grup memeriksa kedaluwarsa, invalidasi, pencarian gagal, batas 512 entri, serta pencegahan hasil pencarian lama menimpa nama yang lebih baru.

Bot hasil rebuild tersambung kembali memakai sesi WhatsApp aktif pada satu terminal yang terlihat. Pesan masuk nyata dan pencarian nama grup langsung belum dibuat saat verifikasi; nama yang tidak tersedia memakai ID. Biarkan terminal terbuka selama bot berjalan.

## Koreksi fallback nama

Format sesuai klarifikasi memakai nama jika tersedia, lalu nomor telepon atau ID jika kosong, tanpa menggabungkan keduanya. Entri private tidak lagi menambahkan ID chat. Empat tes logging lolos, termasuk nama berisi whitespace, fallback pengirim dan grup, LID yang belum terpetakan, serta garis miring dalam nama grup asli. Pemeriksaan format, Clippy tanpa warning, dan build lolos. Bot hasil rebuild tersambung kembali pada terminal yang terlihat. Pesan masuk nyata tidak dibuat untuk pemeriksaan ini.

## Waktu respons ping dan runtime

Pembaruan command lolos pemeriksaan format, Clippy tanpa warning, sintaks JavaScript, serta `cargo build --locked`. `cargo test --locked` meloloskan **29 tes**, dengan tiga tes integrasi backend/media yang sudah ada tetap diabaikan. Tes runtime baru mencakup nol, batas menit/jam/hari, serta beberapa hari dalam kedua bahasa. Tes registry mencakup metadata command/alias dan terjemahan deskripsi; tes Pong memastikan nilai milidetik disubstitusi tanpa placeholder tersisa.

Bot hasil rebuild tersambung kembali memakai sesi aktif pada satu terminal yang terlihat. API command terautentikasi menampilkan `ping` dan `runtime`, termasuk alias `uptime` serta scope grup/private. Pengiriman command nyata dan edit pesan Pong belum diuji. Ping mengukur penyelesaian operasi kirim library, bukan waktu pesan diterima/dibaca; restart proses mereset runtime.

## Optimasi alur respons

Pembaruan lolos `cargo fmt --check`, Clippy tanpa warning, serta `cargo test --locked`: **30 tes lolos**, dengan tiga tes integrasi backend/media yang sudah ada tetap diabaikan. Tes cache baru memastikan pesan dapat memakai fallback ID tanpa menunggu pencarian nama grup yang sedang berlangsung, lalu memakai nama cache setelah pencarian selesai. Handler command kini berjalan bersamaan dengan logging, dan penyimpanan kontak/grup berjalan bersamaan dengan handler yang sudah diterima; izin, deduplikasi, cooldown, serta kuota tetap diperiksa sebelum eksekusi.

`cargo build --release --locked -j 4` lolos, dan executable optimasi menampilkan versi `0.1.0`. Launcher terminal lokal kini memakai executable release. Review persetujuan otomatis menolak percobaan pergantian proses dengan alasan `blocked by policy`; proses debug dibiarkan berjalan dan aktivasi membutuhkan penghentian lewat Ctrl+C sebelum menjalankan release. Waktu respons nyata di bawah 3 ms yang dilaporkan pengguna belum diukur, dan tidak ada pesan tes nyata yang dikirim. Ping tetap melaporkan durasi operasi kirim library yang sebenarnya; angka waktu tidak dimodifikasi.

## Penyembunyian warning linker Windows MSVC

`build.rs` kini menambahkan `/IGNORE:4099` pada pemanggilan linker MSVC paket ini untuk menyembunyikan diagnostik PDB dependensi yang hilang. Pemeriksaan format dan Clippy tanpa warning lolos. `cargo test --locked` memakai toolchain Windows MSVC yang terpasang berhasil melakukan link kedua executable tes tanpa `LNK4099` dan meloloskan **30 tes**, dengan tiga tes integrasi backend/media yang sudah ada tetap diabaikan. Pengecekan build script terpisah memastikan flag muncul untuk `CARGO_CFG_TARGET_ENV=msvc` dan tidak muncul untuk `gnu`.

Bot debug yang sedang berjalan dibiarkan aktif. Executable utamanya perlu dibuild ulang setelah dihentikan lewat Ctrl+C; tidak diperlukan cleanup seluruh dependensi. Penyembunyian warning tidak mengembalikan simbol PDB OpenSSL yang hilang. Nomor warning linker lainnya serta error tetap diaktifkan.

## Metadata stiker dan penghapusan updater

`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, dan `cargo test --locked` lolos: **34 tes berhasil**, dengan tiga tes integrasi diabaikan secara default. Tes integrasi FFmpeg juga dijalankan secara khusus dan lolos untuk stiker statis maupun animasi, memeriksa metadata pack/author serta decode hasil statis. Tes unit mencakup nama Unicode, penggantian EXIF lama, pelestarian data animasi/alpha, container terpotong, batas ukuran termasuk metadata, serta validasi/default konfigurasi. Tes CLI memastikan `run`, `secrets`, dan `send` tetap tersedia sementara `update` ditolak. Dependensi updater sudah dihapus dari manifest dan lockfile.

Konfigurasi aktif dan fixture offline lokal kini menggunakan `[sticker]` tanpa `[update]`. Bot yang sedang berjalan masih memakai binary sebelumnya; hentikan, build ulang, lalu restart untuk mengaktifkan perubahan. Tampilan pack/author pada aplikasi WhatsApp belum diuji dan tidak ada pesan nyata yang dikirim. Tes integrasi PostgreSQL dan MongoDB tidak dijalankan ulang untuk perubahan ini.

## Ukuran stiker animasi dan diagnostik error command

Video enam detik dengan noise berhasil mereproduksi kegagalan sebelumnya: konversi dengan kualitas 50/12 FPS menghasilkan 5.282.864 byte dan ditolak karena batas 500 KiB. Tes regresi gagal sebelum perbaikan dan berhasil setelahnya. Konversi animasi kini mengulang dengan kualitas/FPS lebih rendah jika hasil terlalu besar, menyisihkan 2 KiB untuk metadata pack/author yang sudah divalidasi, serta memakai satu batas waktu konversi untuk seluruh percobaan. Download tanpa metadata panjang file tetap dibatasi oleh writer dan pemeriksaan ukuran byte akhir.

Pemeriksaan format dan Clippy tanpa warning lolos. `cargo test --locked` meloloskan **35 tes**, dengan empat tes integrasi diabaikan secara default. `cargo test --locked bot::messaging::tests -- --ignored` meloloskan kedua tes FFmpeg: konversi statis/animasi pendek serta video enam detik dengan pack/author berisi karakter escaped sepanjang batas maksimal. Pengujian juga mencakup diagnostik video rusak, timeout konversi, serta detail `[ERR]` yang disanitasi pada terminal dan dashboard. Bot yang sedang berjalan tidak direstart dan video asli pengguna belum diuji melalui WhatsApp. Kompresi dapat menurunkan kualitas dan FPS; video yang sangat kompleks masih mungkin perlu dipersingkat. Tes integrasi PostgreSQL/MongoDB tidak dijalankan ulang.

## Pengelolaan command dan stiker pada dashboard

Formatting, pemeriksaan sintaks JavaScript, dan Clippy tanpa warning lolos. `cargo test --locked` meloloskan **39 tes**, dengan empat tes integrasi diabaikan secara default. Kedua tes integrasi FFmpeg juga dijalankan secara khusus dan lolos. Pemeriksaan baru mencakup perubahan nama/alias dan izin, resolusi command yang dihapus, benturan nama, nilai tidak valid, pembatasan owner, autentikasi endpoint, penyimpanan database, serta pengaturan umum yang tetap mempertahankan override command/stiker. JSON pengaturan lama tanpa field baru tetap dapat dibaca dengan nilai default.

Dashboard SQLite terisolasi dijalankan offline pada port 3400 dengan pengaturan sintetis, secret, dan data uji terpisah. Chromium menguji Edit/Save/Cancel, Escape, konfirmasi dan pembatalan penghapusan, pembatasan checkbox owner, escaping deskripsi, penyimpanan stiker, error validasi, logout/login, refresh otomatis, dan field yang belum disimpan. Restart proses mempertahankan nama command baru, command yang dihapus, prefix, serta pack/author; dashboard yang dibuka ulang menampilkan nilai tersimpan. Konfigurasi dan sesi produksi tidak diubah.

Desain yang ada tetap memakai ENERGY 2 / RHYTHM 2 / MOTION 1. Editor memakai gaya formulir/card yang sudah ada; Delete memakai warna error dengan konfirmasi nama command terkait. Layout lolos pada lebar 320, 390, 768, 1024, dan 1280 piksel dalam kedua tema, dengan scroll tabel lebar tetap berada di dalam tabel. Fokus keyboard, zoom CSS/reflow 200%, reduced motion, daftar command kosong, dan viewport pendek untuk simulasi ruang keyboard lolos. Kontras teks tombol baru minimal 6,57:1 dan batas komponen minimal 3,27:1. Screenshot command/stiker diperiksa; skrip dan screenshot sementara telah dihapus saat pembersihan repository. Tidak ada gambar atau tujuan navigasi baru.

Bot produksi yang berjalan perlu dibuild ulang/restart untuk mengaktifkan fitur ini. Editor mengubah konfigurasi dan mempertahankan handler Rust; penghapusan tersimpan pada registry aktif tanpa menghapus file sumber. Eksekusi melalui WhatsApp dan tampilan metadata stiker belum diuji. Tes integrasi PostgreSQL/MongoDB serta keyboard ponsel fisik tidak dijalankan/diuji ulang. Semua backend tetap memakai record pengaturan terenkripsi yang sudah tersedia.

## Generate bahasa otomatis

Formatting, pemeriksaan sintaks JavaScript, Clippy tanpa warning, dan `cargo build --locked -j 1` lolos. `cargo test --locked -j 1` meloloskan **43 tes**, dengan empat tes integrasi media/database yang sudah ada diabaikan. Tes baru mencakup filter bahasa dari provider, katalog lengkap, request bersamaan yang hanya generate sekali, pelestarian/interpolasi placeholder, cache tanpa provider, kode/URL/katalog tidak valid, fingerprint sumber kedaluwarsa, kegagalan tanpa file parsial, autentikasi daftar bahasa, dan kegagalan generate yang mempertahankan pengaturan database.

Binary hasil build dijalankan offline dalam directory SQLite terisolasi dengan mock lokal yang mengikuti kontrak HTTP LibreTranslate. Chromium menguji pemilihan/penyimpanan bahasa baru, kegagalan generate yang mempertahankan bahasa sebelumnya dalam database, tombol reload, dan status kesiapan bahasa. CLI `generate-locales --all` mempertahankan katalog yang berhasil ketika satu bahasa gagal, lalu berhasil saat diulang. Setelah mock dimatikan, pilihan bahasa dari cache tetap bekerja tanpa request terjemahan; restart bot offline memulihkan bahasa terpilih dan katalog. Hasil mock diberi label dan tidak dipasang dalam data produksi. Katalog mock sementara telah dihapus saat pembersihan repository.

Desain ENERGY 2 / RHYTHM 2 / MOTION 1 tetap dipertahankan. Kontrol, fokus keyboard, reduced motion, serta pemeriksaan overflow lolos dalam kedua tema pada lebar 320, 390, 768, dan 1280 piksel. Screenshot diperiksa setelah transisi tema selesai. Kontras teks bantuan bahasa minimal 6,16:1, teks kontrol minimal 6,57:1, dan batas komponen minimal 3,27:1. Tidak ada gambar atau tujuan navigasi baru. Validasi Compose untuk profil translation lolos, dan tag image resmi `v1.9.6` diverifikasi.

`.env` lokal sekarang menunjuk LibreTranslate pada port 5000; nilai environment lain dan data/sesi produksi dipertahankan. Engine Linux Docker tidak tersedia, sehingga container LibreTranslate asli/download model, kualitas terjemahan, serta balasan WhatsApp nyata belum diuji. PostgreSQL/MongoDB serta keyboard ponsel fisik tidak diuji ulang. Pilihan bahasa mengikuti target terjemahan bahasa Inggris pada server; segmen teks di sekitar placeholder dapat memerlukan pemeriksaan tata bahasa.

## Format pesan debugging

Kategori debugging mendaftarkan `testbutton`, `testlist`, `testinteractive`, dan `sendhtml` sebagai command owner untuk grup/private. `cargo test --locked -j1` lolos 46 tes (45 library, satu CLI), dengan empat tes FFmpeg/database yang sudah ada tetap diabaikan. Tes baru mencakup jenis native flow dan ID aksi JSON, node stanza grup/private, ID/payload/batas ukuran rich HTML, serta pembacaan respons button/list native dan legacy. JSON respons native yang rusak, terlalu besar, atau memakai jenis yang tidak didukung ditolak. Tes registry, deskripsi i18n bawaan, izin, dan API dashboard juga lolos. `cargo fmt --check` dan `cargo clippy --locked --all-targets -j1 -- -D warnings` lolos.

`cargo build --locked -j1` juga lolos dan menghasilkan binary Windows terbaru. Tidak ada pesan WhatsApp nyata yang dikirim. Pengiriman serta tampilan native flow/rich HTML belum diverifikasi; dukungan akun/klien dapat berbeda. Tes FFmpeg/PostgreSQL/MongoDB yang diabaikan tidak dijalankan ulang. Key terjemahan baru mengubah fingerprint katalog hasil generate sebelumnya, sehingga bahasa tersebut perlu dibuat ulang.

## Perbaikan stanza rich HTML

Dibandingkan relay Baileys pada Chisato, whatsapp-rust 0.7 mengklasifikasikan `botForwardedMessage` sebagai `media` tanpa jenis media. Pengirim debugging sekarang memilih `text` secara eksplisit untuk envelope ini. Tes regresi mereproduksi klasifikasi bawaan dan memeriksa override pada grup/private, mempertahankan routing native flow, serta memeriksa HTML dan metadata bot melalui encoding/decoding plaintext grup, penerima, dan perangkat sendiri. `cargo test --locked -j1` lolos 48 tes, dengan empat tes FFmpeg/database yang sudah ada tetap diabaikan. Pemeriksaan format dan Clippy lolos. Example sementara memakai `main.rs` aplikasi yang sama, berhasil dikompilasi dan menjalankan `--help`, sehingga executable Windows diperiksa tanpa mengganti bot yang sedang berjalan. Source sementara sudah dihapus. Setelah itu, teramati proses bot baru memakai binary aplikasi hasil build ulang. Tampilan Android setelah perbaikan belum diuji langsung.

## Demo HTML bawaan

`sendhtml` sekarang membentuk demo tetap WhatsApp BOT by TobyG74 tanpa menerima argumen HTML. Judul fallback memakai kredit yang sama. Deskripsi command bawaan dan petunjuk deployment diperbarui dalam bahasa Inggris dan Indonesia. Tiga tes HTML yang sudah ada sekarang memakai layout bawaan, termasuk override stanza text dan encoding grup/private. `cargo test --locked -j1 bot::commands` meloloskan seluruh 12 tes command; pemeriksaan format dan Clippy lolos. Demo diperiksa melalui Chromium dan screenshot mobile ditinjau. Tidak ada pesan WhatsApp nyata yang dikirim; Android dapat menampilkan style inline secara berbeda. Build ulang/restart untuk memakai demo baru. Perubahan katalog Inggris membatalkan cache bahasa hasil generate; buat ulang bahasa tersebut jika digunakan.

- Hard gate PASS: pemeriksaan Chromium pada 320, 390, dan 1280px tidak menemukan overflow; kontras teks memenuhi AA; demo statis tidak memiliki script, aset eksternal, atau kontrol interaktif.
- Purpose gate PASS: garis hijau mengikuti tema WhatsApp; jarak memisahkan nama bot, kredit pembuat, dan contoh format teks; tidak ada gradient, bayangan, atau ikon dekoratif tambahan.
- Liveliness PASS: ENERGY 1 / RHYTHM 1 / MOTION 1; judul menjadi fokus, dengan satu aksen garis hijau tanpa animasi.
- Craftsmanship PASS: judul/kredit sesuai teks yang diminta; screenshot dan pemeriksaan browser menunjukkan layout terbaca, serta 12 tes command dan Clippy lolos.

## Role menu dan saran typo otomatis

Menu sekarang mengelompokkan command aktif berdasarkan kategori, memakai penanda role/jenis chat, judul/legenda sesuai bahasa, deskripsi, alias, serta prefix/mode/role saat ini. Tes memeriksa visibilitas pengguna/owner, scope private/grup, kategori/deskripsi kustom, alias hasil edit, penghapusan command, dan kedua bahasa bawaan. Saran otomatis membandingkan nama command ASCII dengan panjang terbatas terhadap nama/alias saat ini, mendukung huruf tertukar, dan memberikan maksimal tiga nama utama dengan kecocokan terbaik. Tes mencakup huruf tambahan/hilang/salah/tertukar, kapitalisasi, prefix, input tidak relevan/tidak valid/terlalu panjang, urutan hasil setara, pembatasan owner dan debugging, mode self, jenis chat, edit dashboard, serta penghapusan. Placeholder balasan saran diperiksa dalam bahasa Indonesia dan Inggris.

Dispatcher menerapkan deduplikasi pesan dan cooldown terpisah lima detik per pengirim tanpa biaya kuota sebelum membalas. Tes SQLite terpisah memeriksa deduplikasi, admission cooldown bersamaan, dan kuota yang tetap utuh, termasuk saat kuota harian telah tercapai. `cargo test --locked -j1` meloloskan 53 tes (52 library, satu CLI), dengan empat tes FFmpeg/database yang sudah ada tetap diabaikan; `cargo fmt --check` dan `cargo clippy --locked --all-targets -j1 -- -D warnings` lolos. Konfigurasi, key, session, dan database produksi tidak diubah; tidak ada pesan WhatsApp nyata yang dikirim. Build ulang/restart untuk menerapkan perubahan. Tampilan menu dan balasan saran pada WhatsApp nyata belum diverifikasi. Key baru pada katalog Inggris memerlukan generate ulang cache bahasa hasil generate.

## Filter kategori menu

`menu [kategori]` sekarang meneruskan argumen ke pembentuk menu dan memfilter kategori setelah pemeriksaan izin. Tanpa argumen, seluruh kategori yang bisa diakses tetap ditampilkan. Filter menerima huruf besar/kecil, spasi di awal/akhir, serta nama kategori kustom dengan beberapa kata. Kategori yang tidak tersedia menampilkan pemberitahuan sesuai bahasa dan hanya mencantumkan kategori yang bisa diakses. Petunjuk penggunaan mengikuti prefix dan nama menu yang dikonfigurasi. Tes regresi mencakup filter, pembatasan owner/private/grup, kategori dashboard kustom, perubahan nama, penghapusan, alias, serta kedua bahasa bawaan. `cargo test --locked -j1 bot::commands` meloloskan seluruh 18 tes command; pemeriksaan format dan Clippy lolos. Tidak ada balasan menu WhatsApp nyata yang dikirim. Build ulang/restart untuk menerapkan perubahan, dan generate ulang bahasa dengan cache hasil generate setelah katalog sumber Inggris berubah.

## Bahasa pribadi dan pengaturan fitur

`setlang` menjadi command pengguna pada kategori general. Dispatcher memakai pilihan bahasa pengirim yang tersimpan terenkripsi untuk balasan, deskripsi, saran typo, dan error, dengan fallback bahasa default bot. SQLite/PostgreSQL membuat tabel preferensi saat startup; MongoDB memakai koleksi terenkripsi terpisah. Pilihan pribadi tidak mengganti pengaturan global. Command owner `setting`, `setlimit`, dan `setcooldown` menyimpan toggle fitur, kuota harian, dan cooldown global opsional. Penyimpanan dashboard mempertahankan nilai fitur yang tidak disertakan formulir. Baca otomatis pesan/status memakai API read receipt library dan berjalan bersamaan dengan handler; pesan sendiri, newsletter, dan event lama diabaikan.

Suite default meloloskan 61 tes, dan seluruh empat tes integrasi FFmpeg/PostgreSQL/MongoDB lolos memakai database sementara terisolasi. Tes mencakup default konfigurasi lama, akses owner, parsing toggle, bypass limit/cooldown tanpa menghapus riwayat, command gratis saat kuota habis, isolasi bahasa pengguna, preferensi terenkripsi yang bertahan setelah restart SQLite, penyimpanan preferensi PostgreSQL/MongoDB, dan pelestarian toggle dari dashboard. Formatting dan Clippy tanpa warning lolos. Tes penerjemahan kini membandingkan generate bersamaan dengan referensi generate tunggal, sehingga tidak bergantung pada jumlah batch katalog lama.

Fixture/screenshot browser sementara, arsip upstream hasil ekstraksi, folder kosong, installer, serta artefak launcher/log/linker uji sudah dihapus. Instalasi alat aktif, data produksi, konfigurasi, dan bot yang berjalan dipertahankan. Penghapusan paksa sisa clone Git upstream ditolak review persetujuan otomatis (`blocked by policy`); folder tersebut tetap diabaikan Git. Nilai secret aktif tidak ditemukan pada 62 file calon isi Git yang diperiksa. Commit/push tidak dilakukan.

Balasan command maupun pembacaan status pada WhatsApp nyata belum diuji. Build ulang/restart untuk menerapkan perubahan. Katalog bahasa hasil generate perlu dibuat ulang setelah sumber berubah; Inggris/Indonesia bawaan tetap tersedia.

## Keterbatasan pengujian awal

Pairing/pengiriman WhatsApp nyata, MongoDB Atlas/SRV/TLS, kredensial database produksi, Docker image, dan CI di GitHub belum diuji. Semua pesan runtime adalah fixture offline dan dibatalkan. Secret tes tidak digunakan untuk konfigurasi produksi.

MongoDB membatasi media sampai 8 MiB karena payload terenkripsi disimpan dalam satu dokumen; GridFS belum dipakai. Sesi WhatsApp tetap SQLite SQLCipher pada semua backend. Mengganti database tidak memigrasikan data otomatis.
