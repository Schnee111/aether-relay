# The Hardening Crucible — 16 Rigorous Verification Scenarios

Dokumen ini mendefinisikan matriks pengujian ekstrem (*battleground matrix*) untuk memvalidasi arsitektur AetherRelay sekaligus menguji seluruh gerbang alur kerja rekayasa (*dev-workflow*) yang telah distandardisasi.

---

## Matriks 16 Skenario Pengujian

### 1. Pre-Commit Hook Gauntlet (Negative Git Testing)
- **Tujuan**: Membuktikan bahwa linter commit menolak pesan commit yang melanggar Conventional Commits.
- **Uji Kasus**:
  - `Feat: uppercase subject` -> Ditolak.
  - `fix(core): fixed past tense` -> Ditolak.
  - `refactor(db): trailing period.` -> Ditolak.
  - Subjek > 72 karakter -> Ditolak.
  - Commit atomik valid dengan body 3W -> Lolos.

### 2. Pre-Push Cleanliness Gate
- **Tujuan**: Memastikan tidak ada file debug atau unstaged temporary artifacts yang tertinggal saat push.
- **Eksekusi**: Jalankan perintah `test -z "$(git status --porcelain)"`. Jika terdapat sisa file, push diblokir seketika.

### 3. Multi-Issue Governance & Definition of Ready (DoR)
- **Tujuan**: Mencegah pengerjaan tiket dengan spesifikasi yang ambigu.
- **Uji Kasus**:
  - Issue cacat spesifikasi tanpa kriteria biner -> Ditandai `status:blocked` dan ditolak oleh gerbang DoR.
  - Issue Bug Report lengkap (MRE via curl) -> Lolos DoR (`status:ready-for-dev`).
  - Issue RFC Arsitektur -> Lolos DoR dan ditautkan ke ADR-0001.

### 4. Stacked PRs & Multi-Branch Rebase Dependency
- **Tujuan**: Menguji pola percabangan dependen enterprise.
- **Uji Kasus**:
  - PR 1: Schema & Idempotency Engine (`feat/db-idempotency` -> `main`).
  - PR 2: Ingest API & Signature Adapters (`feat/api-gateway` -> `feat/db-idempotency`).
  - Merge PR 1 ke `main`, lalu rebase PR 2 ke `main` (`git rebase --onto main ...`) tanpa konflik.

### 5. Adversarial Code Review & Outright Rejection (Changes Requested)
- **Tujuan**: Menguji ketegasan gerbang review independen.
- **Temuan Blocking**:
  - Timing attack vulnerability pada perbandingan signature.
  - Unhandled database connection lock contention.
- **Status**: PR diberi status `CHANGES REQUESTED` dan merge diblokir total.

### 6. The Empirical Pushback Scenario (Benchmark-Backed Refusal)
- **Tujuan**: Menunjukkan penolakan saran reviewer berbasis data empiris (anti-sycophancy).
- **Skenario**: Reviewer menyarankan `PRAGMA synchronous = FULL`. Author menyertakan laporan benchmark A/B yang membuktikan bahwa `NORMAL` menghasilkan 7.200 ops/s vs 95 ops/s pada `FULL` dengan crash safety setara. Reviewer menerima data dan menutup thread.

### 7. Defect Remediation & Thread Resolution Ownership
- **Tujuan**: Menguji kepatuhan resolusi thread.
- **Aturan**: Author memperbaiki issue via commit atomik dan merujuk SHA. Hanya reviewer yang berhak menutup thread blocking setelah verifikasi.

### 8. Byte-Exact Cryptographic Signature Verification
- **Tujuan**: Menguji verifikasi tanda tangan kriptografi nyata (bukan mock).
- **Uji Kasus**:
  - Valid GitHub HMAC-SHA256 payload -> 202 Accepted.
  - Valid Stripe v1 timestamped signature -> 202 Accepted.
  - Valid Midtrans SHA-512 payload -> 202 Accepted.
  - Corrupted 1-byte payload -> 401 Unauthorized.
  - Expired Stripe timestamp (> 300s) -> 401 Unauthorized (Anti-Replay).

### 9. Extreme Concurrency Flood (Race-Condition Guard)
- **Tujuan**: Membuktikan jaminan exactly-once processing.
- **Eksekusi**: Mengirimkan 50 request konkuren dengan `Idempotency-Key` yang sama persis secara simultan dalam 1ms.
- **Ekspektasi**: Tepat 1 request berhasil masuk (202 Accepted), 49 lainnya mengembalikan status 409 Conflict atau cached result. Nol baris duplikat di SQLite.

### 10. Downstream Failure & Exponential Backoff with Jitter
- **Tujuan**: Membuktikan ketahanan retry worker saat downstream down.
- **Eksekusi**: Simulasikan downstream service mengembalikan HTTP 500.
- **Ekspektasi**: Event dijadwalkan ulang dengan formula Decorrelated Jitter. Interval retry meningkat secara acak berbatas.

### 11. Downstream Circuit Breaker Transition
- **Tujuan**: Mencegah pemborosan resource saat downstream mati total.
- **Ekspektasi**: Setelah 5 kegagalan beruntun, state beralih dari CLOSED ke OPEN. Request ditahan secara lokal tanpa menembak jaringan downstream selama recovery timeout.

### 12. Dead-Letter Queue (DLQ) Isolation & Forensic Snapshot
- **Tujuan**: Memastikan event gagal tidak terbuang.
- **Ekspektasi**: Setelah percobaan maksimal habis, event otomatis dipindahkan ke tabel `dead_letter_queue` lengkap dengan headers, payload, dan error stack trace.

### 13. Atomic DLQ Replay API Verification
- **Tujuan**: Memulihkan event yang sempat masuk DLQ.
- **Eksekusi**: Panggil `POST /v1/dlq/:id/replay`.
- **Ekspektasi**: Event dikembalikan ke status `RECEIVED` dan berhasil dikirimkan setelah downstream service kembali hidup.

### 14. Crash Recovery & Durability Drill (The Kill -9 Test)
- **Tujuan**: Menguji durabilitas SQLite WAL saat proses dimatikan paksa.
- **Eksekusi**: Di tengah penulisan 500 event, kirim sinyal `kill -9` ke proses server.
- **Ekspektasi**: Setelah proses dihidupkan ulang, `PRAGMA integrity_check` bernilai `ok`, tidak ada data korup, dan worker melanjutkan pengiriman sisa event secara otomatis.

### 15. Memory Leak & Soak Load Benchmark
- **Tujuan**: Membuktikan stabilitas memori di bawah beban berat.
- **Eksekusi**: Menjalankan Autocannon dengan 10.000 request selama 30 detik.
- **Ekspektasi**: Throughput >= 5.000 QPS, latensi P99 < 15ms, RSS memory stabil < 100MB tanpa kenaikan linier (zero memory leak).

### 16. Production P0 Incident Hotfix & SemVer Propagation
- **Tujuan**: Menguji siklus perbaikan darurat dan rilis otomatis.
- **Eksekusi**: Branch `hotfix/stripe-timestamp-drift` -> targeted fix -> PR kilat -> full merge.
- **Ekspektasi**: SemVer dinaikkan otomatis (`0.1.0` -> `0.1.1` via PATCH), dan `CHANGELOG.md` terkompilasi rapi.
