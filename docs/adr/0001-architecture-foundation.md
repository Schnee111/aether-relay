# ADR-0001: Fastify, SQLite WAL, Kysely, and Embedded Durability Architecture

- **Tanggal**: 2026-09-22
- **Status**: Accepted
- **Deciders / Author**: @Schnee111, @Shorekeeper
- **Related Issue/PR**: #1

---

## 1. Konteks & Problem Statement
Dalam membangun AetherRelay sebagai gateway penerimaan dan penyaluran webhook yang tangguh, sistem membutuhkan:
- Latensi ingesti ultra-rendah (< 15ms) dengan kapasitas throughput ribuan requests per detik.
- Kemampuan penanganan raw body streaming untuk verifikasi kriptografis tanpa double memory buffering.
- Durabilitas penyimpanan lokal yang tahan banting terhadap crash proses (kill -9) tanpa membutuhkan dependensi eksternal yang berat (seperti cluster Redis atau Kafka multi-node) yang membebani memori VPS (RAM guard 3.6GB).
- Jaminan pemrosesan tepat satu kali (exactly-once processing) bebas race condition saat request duplikat datang bersamaan.

---

## 2. Decision Drivers
- **Resource Footprint Minim**: Wajib berjalan efisien di lingkungan VPS dengan batasan RAM 3.6GB.
- **Zero Data Loss**: Setiap event yang berstatus HTTP 202 wajib persisten di disk sebelum respons dikirim.
- **Type-Safety & Ergonomi**: End-to-end type safety dari skema database hingga HTTP handler.
- **Simplicity of Deployment**: Single-binary / single-process operational model tanpa manajemen multi-service terdistribusi yang rumit.

---

## 3. Opsi yang Dipertimbangkan

### Opsi 1: Fastify + SQLite WAL Mode via better-sqlite3 & Kysely (Terpilih)
- **Kelebihan**:
  - Fastify memiliki overhead terendah di ekosistem Node.js (~70k-90k RPS), native stream backpressure, dan mudah mengisolasi raw body buffer.
  - SQLite dalam mode WAL (Write-Ahead Logging) dengan `PRAGMA synchronous = NORMAL` memberikan performa ribuan write per detik dengan jaminan kebal crash OS/proses.
  - Kysely memberikan query builder berbasis SQL standar dengan static type-safety tanpa runtime overhead engine Rust (seperti Prisma).
  - Jejak memori sangat ramping (< 50MB RSS).
- **Kekurangan**: SQLite adalah single-writer; penulisan konkuren memerlukan tuning `busy_timeout` dan transaksi atomik `BEGIN IMMEDIATE`.

### Opsi 2: Express + Redis (BullMQ) + PostgreSQL
- **Kelebihan**: Arsitektur standar industri terdistribusi; Redis menangani antrean cepat dan PostgreSQL menangani histori.
- **Kekurangan**: Overhead memori tinggi (> 500MB untuk PostgreSQL + Redis daemon), memerlukan setup multi-container yang rentan terhadap kegagalan jaringan internal.

### Opsi 3: Go (Fiber/Gin) + BadgerDB
- **Kelebihan**: Performa puncak dan kompilasi single binary murni.
- **Kekurangan**: Ekosistem dynamic schema dan tooling migrasi relasional kurang ergonomis dibandingkan Kysely SQL; tim lebih cepat berekspansi di TypeScript/Node 22 LTS.

---

## 4. Keputusan yang Diambil (Decision Outcome)
Kami memilih **Opsi 1: Fastify + SQLite WAL Mode via better-sqlite3 & Kysely**.

### Justifikasi
Kombinasi ini memberikan rasio performa-terhadap-resource tertinggi untuk lingkungan VPS kita. Fastify menjamin ingesti non-blocking, sementara SQLite WAL bertindak sebagai persistent write buffer yang sangat cepat dan tidak memakan resource background daemon.

---

## 5. Konsekuensi

### Dampak Positif (+)
- Throughput lokal mampu mencapai > 5.000 QPS pada 1 proses Node.js.
- Zero external dependency (cukup file database lokal `data/aether.db`).
- Crash resilience mutlak berkat mekanisme WAL journal checkpointing.
- Biaya operasional dan konsumsi RAM sangat rendah (< 100MB).

### Kompromi / Trade-offs (-)
- Penulisan konkuren dibatasi oleh single-writer lock SQLite. Solusi: Menggunakan transaksi `BEGIN IMMEDIATE` dan membatasi waktu transaksi write agar sangat singkat (< 2ms).
- Backup berkala memerlukan perintah online backup SQLite (`VACUUM INTO`).
