# Product Requirements Document (PRD) — AetherRelay

## 1. Executive Summary & Problem Statement
AetherRelay adalah sistem gateway penerima (ingestion) dan penyalur (dispatch) webhook berkinerja tinggi, berkeandalan tinggi (high-reliability), dan minim jejak komputasi (lightweight). Dalam ekosistem komputasi modern, webhook dari pihak ketiga (Stripe, GitHub, Midtrans, Discord, Shopify) sering kali menjadi titik rentan sistem (single point of failure). Masalah-masalah yang diselesaikan oleh AetherRelay meliputi:
- Retry Storms & Duplicate Execution: Provider pihak ketiga secara agresif mengirimkan ulang payload saat jaringan lambat, yang berisiko memicu eksekusi ganda jika sistem downstream tidak memiliki idempotency guard yang atomik.
- Downstream Outages & Data Loss: Ketika backend utama mengalami downtime atau restart, webhook eksternal yang gagal terkirim dapat hilang permanen.
- Timing Attacks & Cryptographic Vulnerabilities: Verifikasi tanda tangan digital yang menggunakan komparasi string konvensional rentan terhadap serangan timing attack.
- System Overload (Cascading Failures): Mencoba menghubungi downstream service yang sedang down secara berulang tanpa backoff atau circuit breaker dapat memperparah kerusakan sistem hilir.

AetherRelay bertindak sebagai lapisan penyangga tangguh (shock absorber) di depan infrastruktur internal: menerima webhook dalam waktu < 10ms, memverifikasi tanda tangan secara konstan (constant-time), mencatat payload ke database persisten lokal berbasis SQLite WAL, lalu menyalurkan event ke downstream service secara terkontrol menggunakan jittered exponential backoff dan circuit breaker.

## 2. Target Persona & User Stories
- Backend / Platform Engineer: Ingin menerima webhook dari banyak provider tanpa harus menulis ulang logika signature verification, deduplikasi, dan retry queue di setiap microservice.
- System Reliability Engineer (SRE): Ingin memastikan tidak ada data webhook yang hilang saat backend internal down, serta memiliki visibilitas penuh melalui metrik Prometheus dan Dead-Letter Queue (DLQ) replay.
- Autonomous AI Coding Agent: Membutuhkan gateway event yang deterministik, memiliki API kontrak yang ketat, dan menyediakan log audit jejak eksekusi yang terstruktur.

## 3. Scope Boundaries & Non-Goals

### In-Scope:
- Ingestion HTTP endpoint dengan zero-copy raw body preservation untuk verifikasi signature kriptografis.
- Persistensi lokal berdaya tahan tinggi menggunakan SQLite dalam mode WAL (Write-Ahead Logging) dengan memory-mapped I/O.
- Jaminan pemrosesan tepat satu kali (exactly-once processing guarantee) menggunakan atomic transaction `BEGIN IMMEDIATE` pada level basis data.
- Built-in provider adapters: GitHub Webhook (HMAC-SHA256), Stripe Webhooks (v1 timestamped signature dengan drift tolerance), Midtrans Webhooks (SHA-512), Discord Interactions (Ed25519), dan Generic Bearer/HMAC.
- Dispatch engine asinkronus dengan Decorrelated Exponential Backoff with Jitter.
- Downstream Circuit Breaker per endpoint (Closed, Open, Half-Open).
- Dead-Letter Queue (DLQ) forensik dengan Replay API manual/otomatis (`POST /v1/dlq/:id/replay`).
- Observability: Zero-overhead JSON logging via Pino (asynchronous sonic-boom destination) dan metrik Prometheus (`/metrics`).

### Out-of-Scope (Non-Goals):
- Complex Event Processing (CEP) atau analisis data analitik berat (AetherRelay adalah transport & reliability gateway, bukan data warehouse).
- Message Broker terdistribusi berskala multi-datacenter (fokus AetherRelay adalah single-node durability yang efisien untuk VPS).
- GUI Web Dashboard kompleks (monitoring dilakukan via Prometheus/Grafana dan REST API terstandarisasi).

## 4. Non-Functional Requirements & Performance SLAs
- Ingestion Throughput: Mampu menangani >= 5.000 requests/detik pada VPS modern (2-4 vCPU, 4GB RAM) dengan penggunaan CPU < 60%.
- Ingestion Latency: P95 < 10ms, P99 < 15ms (mengembalikan status HTTP 202 Accepted).
- Memory Footprint: RSS memory stabil di bawah 100MB saat continuous ingestion load tanpa memory leak.
- Durabilitas Crash: Kebal terhadap terminasi paksa (kill -9); integritas SQLite WAL tetap utuh dan event yang berstatus pending langsung dilanjutkan saat proses restart.
- Security: Kebal timing attack pada komparasi signature, batasan payload maksimal 5MB (anti-payload bombing), dan proteksi SSRF terhadap private IP/cloud metadata.
