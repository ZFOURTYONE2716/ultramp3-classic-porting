# Pemulihan kode UltraMP3 1.52

Analisis berasal dari file SIS yang diberikan. Tidak ada kode dari prototipe lama yang dipakai. Biner Symbian tidak dijalankan.

## AppLoader

Header E32 dibaca sesuai [E32IMAGE.H dari Symbian](https://github.com/loociano/symbian-build-tools/blob/master/e32tools/INC/E32IMAGE.H). `main.dll` memiliki kode pada offset file `0x7c`, ukuran `0x1130`, serta relokasi di `0x12e0`. Relokasi berakhir di `0x1360`, diikuti ukuran terkompresi (194.585) dan ukuran hasil (375.864).

Offset fungsi di bawah relatif terhadap segmen kode E32. Fungsi Thumb di `0x3ac` menjumlahkan word 32-bit pada bagian kode. Pemanggil di `0x3cc` membaca lokasi dan ukuran segmen dari header, lalu memasok jumlah tersebut ke `0xa4`. Loop di `0x130–0x150` menerapkan XOR ke word utuh. Aliran zlib valid dimulai pada offset file `0x1368` setelah operasi itu. Nilai checksum pada sampel ini adalah `0xd9d0103a`.

```powershell
python unpack_apploader.py extracted/20_0_main.dll target/UltraMP3.lcg --elf target/UltraMP3-code.elf
```

Hasil `LCG!` versi 1 berukuran 375.864 byte, SHA-256 `3796faa4e0371913a93b63c4ae24da472a31b9912f06a069d2268518614671ed`. Segmen kode yang dipetakan mulai dari offset payload `0x38` ke alamat ARM `0x10001000`, panjang `0x58200`. Entry yang dideklarasikan adalah `0x1002bb2c` dan berisi prolog ARM. ELF hanya pembungkus analisis dari byte asli. Impor dan relokasi belum diterapkan.

REA membuka ELF sebagai ARM (SHA-256 `f48af5ba00da8a0586618e34e7bb9b8f5dbaafe871c210fdfddd0d19bd91ed62`). Analisis fungsi mengembalikan `provider_unavailable`. Profil Ghidra Windows pada sesi ini menolak ARM ELF. Pemeriksaan instruksi dilanjutkan dengan Capstone.

## Kontrol dan audio asli

- Fungsi ARM di `0x10004a58` memuat literal alamat `skin.pcx`, `skin.ini`, dan `spr.txt` di area `0x10004b94`. Ini memberi titik masuk untuk menelusuri pemuatan skin.
- `extracted/12_0_S60.dta` adalah ZIP yang memuat skin bawaan, font bitmap, logo, bahasa, sprite dan mask. `skin.ini` menempatkan komponen tombol di koordinat `(47,190)`. `spr.txt` menyimpan 7 sprite normal (ID 10–16), 7 sprite ditekan (20–26), dan outline (43). Posisi individual dan hubungan antara indeks sprite dengan perintah tombol perlu dikonfirmasi dari fungsi gambar dan penangan input.
- Tabel impor dalam payload memuat `MEDIACLIENTAUDIOSTREAM[10003996].DLL` di offset `0x5974c`. Ini mengidentifikasi dependensi audio Symbian yang perlu dipetakan ke Windows. String `MP3 decoder` ada di offset `0x44340`. Perilaku decoder, perintah tombol, dan tampilan runtime belum diverifikasi.
- Pada rilis rekonstruksi, MCI 277 muncul saat `open ... type mpegvideo` gagal menginisialisasi driver. Pesan itu tidak menentukan penyebab yang lebih rinci. Rilis sebelumnya belum diperbaiki atau diganti dalam tahap analisis ini.


## Rutinitas kontrol yang dipindahkan

- `0x10003598–0x100036f8` menggambar enam slot. Loop `0x10003610–0x1000369c` menggunakan posisi komponen 7, jarak 14 piksel, sprite dasar `10 + indeks`, sprite 16 untuk slot Play saat lagu berjalan, dan offset sprite +10 saat ditekan. Sprite 43 adalah bingkai fokus. Implementasi Rust ada di `src/skin.rs`. Kondisi asli untuk menyembunyikan bingkai sementara dan operasi latar di `0x10002290` belum ditranslasi.
- Tabel `0x10004310` menunjuk ke handler previous, rewind, play/pause, stop, forward, next. Perubahan pilihan melingkar terlihat di `0x10006618–0x10006654`. Pilihan awal 2 disetel di `0x100095a4`. Jalur waktu dalam seek memakai 10.000 ms. Jalur tracker yang memakai indeks posisi belum dipindahkan dan memakai seek berbasis waktu di build ini.
- `extract_skin_assets.py` mengambil skin, koordinat, sprite dan mask dari `S60.dta` serta tiga arsip skin asli. Mask menjadi kanal alpha tekstur. Tampilan playlist, font informasi lagu, dan visualisasi belum dipindahkan sepenuhnya.
- `src/audio.rs` adalah adapter Windows yang menggunakan WASAPI melalui Rodio dan CPAL, dengan dekoder MP3/WAV Symphonia serta libopenmpt untuk tracker. Ini menggantikan MCI yang menghasilkan error 277 pada build lama. Kode decoder Symbian masih dalam analisis.


## Playlist, font, dan osiloskop

- Playlist diterjemahkan dari `0x1000727c–0x10007734`. Panel berukuran 164 × 168 dan berpusat di layar. Tinggi baris 8 piksel dengan 18 entri maksimum pada satu layar, nomor di kiri, dan ukuran di kanan. Alur pemilih file Windows, menu yang disederhanakan, dan navigasi mouse adalah adaptasi. File sumber tidak berubah ketika playlist diurutkan.
- `0x1000612c` memajukan teks tetap 5 piksel per karakter. `0x10006048–0x1000607c` memilih glif 5 × 6 dari kisi 16 kolom di `font.pcx`. Port memakai glif ASCII asli. Di luar ASCII, glif cadangan saat ini adalah tanda tanya.
- Osiloskop mengikuti proyeksi sampel PCM bertanda di `0x100039fc–0x10003b58`, termasuk tinggi/warna dari skin dan kanal stereo terpisah. Adapter WASAPI meneruskan blok PCM dari dekoder ke UI. Penjadwalan buffer mengikuti Windows dan belum dibandingkan dengan ritme sampling Symbian. Mode Spectrum Bars belum dipindahkan.
- Mode Single, Continuous, Random, Repeat, dan tampilan waktu diacu dari `lang/english.txt`. Pemilihan urutan acak dan pembacaan ID3 adalah adaptasi Windows. Belum ada verifikasi kesetaraan terhadap runtime Symbian.


## Penyegaran osiloskop Windows

Loop ARM `0x100039fc–0x10003b58` memetakan setiap sampel PCM 16-bit bertanda ke satu titik. Rumusnya `y = ((sample * (height - 1)) >> 16) + height / 2`. Cabang stereo menulis kanal kanan lebih dulu, lalu kanal kiri, pada setiap koordinat X. Port mempertahankan rumus, urutan lapisan, koordinat, warna skin, serta titik tanpa interpolasi garis.

Pada ARM, pemanggil `0x100038d4` meminta pointer buffer melalui slot vtable `+0x38`. Vtable `0x10058b88` mengarahkan panggilan ke `0x10039584`, yang membaca dua pointer dari objek audio. Inisialisasi di `0x100054c0–0x100054d0` meminta 22.050 Hz dan dua kanal. Loader audio memiliki jalur penyesuaian perangkat, sehingga ini adalah nilai permintaan yang teramati, bukan jaminan laju semua perangkat Symbian.

Pada Windows, `src/scope.rs` mengambil sampel hingga 22.050 Hz untuk visualisasi dan menerbitkan jendela PCM terbaru. Laju keluaran WASAPI tidak berubah. Saat visualisasi terlihat dan pemutaran aktif, UI meminta refresh tiap 16 ms. Pemilihan sampel dan jadwal refresh Windows ini belum dibandingkan dengan rekaman runtime Symbian.


## Spectrum Bars

`lang/english.txt` mencantumkan Oscilloscope, Spectrum Bars, dan None. Cabang mode 0 pada `0x10003b5c–0x100040dc` mengambil FFT 512 sampel. `0x10009f38` merata-ratakan dua kanal, menskalakan PCM dengan 1/64, dan menjalankan FFT radix-2. Kolom membaca bin dengan langkah tetap 256 dibagi lebar skin. Rentang `0x10003d18–0x10003e0c` menaikkan batang langsung, menurunkan level seiring waktu, serta mempercepat jatuhnya penanda puncak. Warna batang mengikuti jalur biru, sian, hijau, kuning, dan merah di `0x10003eb8–0x10003f70`. Warna penanda berasal dari skin.

`src/spectrum.rs` menerjemahkan jalur tersebut dengan FFT float dan timing Windows. Buffer PCM memakai jendela 512 sampel terbaru. Osiloskop tetap menggunakan bagian terakhir sesuai lebar area skin. Pembulatan numerik dan timing belum dibandingkan dengan rekaman dari runtime Symbian.


## Penyimpanan playlist dan pengelolaan file

- Penulis ARM `0x10041ccc–0x10041e7c` membuka `playlist.ini`, menulis BOM UTF-16LE, indeks lagu berbasis nol dalam desimal, lalu daftar path dengan CRLF. Entri bertanda diawali `*`. Pembaca pada jalur `0x10005544–0x1000574c` memeriksa BOM dan mengurai indeks serta baris. Port memakai format yang sama untuk pemulihan otomatis serta impor dan ekspor. Path relatif diinterpretasikan terhadap lokasi playlist. Drive Symbian perlu disesuaikan agar mengarah ke file Windows yang ada.
- Penulis `config.bin` di `0x100049ac–0x10004a20` menyalin 0x21c byte dari struktur objek. Build Windows menyimpan preferensi di `windows.ini` terpisah, tanpa menjanjikan kompatibilitas dengan dump struktur Symbian.
- Rescan File System, Mark, Unmark, Sort, Move, Rename, Copy, Play Minimized, Idle Timeout dan About berasal dari label `lang/english.txt`. Dialog file, cakupan rescan, pengelolaan file, minimisasi jendela, dan penyimpanan atomik adalah adaptasi Windows. Rename, Move dan Copy bekerja pada satu entri yang dipilih. Penghapusan fisik masih belum tersedia. Hapus dari playlist tidak menghapus berkas musik.


## Palet dan koordinat HUD asli

- Menu utama di `0x10006d4c–0x10007270`: panel (6,28), 164 × 168, isi ARGB `0xC0000040`, bingkai `0xFF8080C0`. Judul pada Y=32. Baris mulai (11,48) dengan langkah 10 piksel, teks normal `0xFFC0C0C0`, fokus `0xFFFF0000`, dan deskripsi `0xFF808080` di Y=164/172/180. Urutan menu dibaca dari tabel `0x10043794`, dengan teks bantuan dari `lang/english.txt`.
- Playlist di `0x1000727c–0x10007734`: panel (6,20), 164 × 168, isi `0xC0400000`, bingkai `0xFFC08080`; versi redup di bawah menu FILE memakai `0xC0200000` dan `0xFF604040`. Teks bertanda putih dan entri biasa abu-abu `0xFF808080`. Sorotan entri bertanda `0xFF606060`, lainnya `0xFF303030`. Scroll menahan margin 3 baris di atas dan 3 di bawah dengan 18 baris terlihat.
- Menu FILE dari `0x10007b90–0x10007d00` dan `0x100080e8–0x10008298`: x=80, lebar 90, tinggi 60, mengikuti posisi baris terpilih. Move pada menu ini mengurutkan ulang playlist dengan panah. Remove menggantikan tindakan penghapusan file di port ini. Menu tambahan Windows tetap dapat dibuka lewat softkey Menu di bawah playlist.
- Jam digital di `0x1000291c–0x10002b8c` memakai posisi digit tetap, tanda minus untuk waktu tersisa dan titik dua berkedip. Volume di `0x10003500–0x10003544` menambah segmen mulai ID 9 ke bawah. Susunan informasi judul, artis, dan album di `0x10002d20–0x10003074` memakai baris 8 piksel dan indentasi 10 piksel.
- `ref/images (3).jpg` menunjukkan menu utama biru dengan fokus merah. `ref/UltraMP3-Basic-Music-Player-for-Old-Symbian-Phones-7.jpg` menunjukkan playlist. Keduanya berukuran 176 × 208. Tema klasik memakai aset persis dari SIS. Windows masih memakai komposit GPU, sedangkan biner Symbian menulis ke framebuffer RGB565. Kecocokan warna dan tata letak diuji terhadap referensi, tetapi kesetaraan pixel demi pixel dan animasi runtime belum terbukti.


## Pembandingan render 176 × 208

Menu dan playlist Windows diambil dari framebuffer eframe pada ukuran logis 176 × 208. Posisi panel, baris, fokus merah, dan bantuan menu utama dibandingkan dengan dua gambar referensi asli. Sampel panel menu pada area bersih menunjukkan campuran biru yang dekat dengan referensi JPEG. Render Windows mengikuti ARGB dari kode asli. JPEG, komposit RGB565, dan isi lagu yang berbeda menyisakan selisih piksel.

Perbandingan menemukan sprite transport yang masih terlihat saat panel terbuka. Jalur menu `0x10006a18` dan playlist `0x10007880` memanggil `0x10004264` dengan r2=0. Branch di `0x100042c8` melewati gambar transport dalam kondisi itu. Build rilis mengikuti branch tersebut dan membatasi judul lagu menjadi 20 glif sesuai `0x1000263c`.

