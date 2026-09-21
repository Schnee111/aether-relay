# Technical Architecture Specification — AetherRelay

## 1. System Architecture Overview

AetherRelay mengadopsi arsitektur decoupling asinkronus antara lapisan Ingesti HTTP (Ingestion Layer) dan lapisan Pengiriman (Dispatch Layer). Komunikasi antar-layer diikat oleh Persistent Storage Engine berbasis SQLite WAL yang sangat cepat dan efisien.

```text
[ Webhook Sender: GitHub, Stripe, Midtrans, dll ]
                       │
                       ▼ (HTTP POST /v1/ingest/:endpoint_id)
      ┌────────────────────────────────────────────────────────┐
      │               AetherRelay Ingestion Layer              │
      │  1. Fastify Stream-Optimized HTTP Transport            │
      │  2. Raw Body Preservation (Byte-Exact Buffer)          │
      │  3. Cryptographic Signature Verification (Constant-Time)│
      │  4. Atomic Idempotency Check (BEGIN IMMEDIATE)         │
      └──────────────────────────┬─────────────────────────────┘
                                 │ (Event Enqueued: State = RECEIVED)
                                 ▼
      ┌────────────────────────────────────────────────────────┐
      │       Durable Storage Engine (SQLite WAL Mode)         │
      │  - PRAGMA journal_mode = WAL; synchronous = NORMAL     │
      │  - UUIDv7 Time-Ordered Primary Keys                    │
      │  - Tables: endpoints, incoming_events, attempts, dlq   │
      └──────────────────────────┬─────────────────────────────┘
                                 │ (Lease Acquisition / Poll)
                                 ▼
      ┌────────────────────────────────────────────────────────┐
      │               Dispatch Worker Engine                   │
      │  1. Circuit Breaker Evaluator (Closed/Open/Half-Open)  │
      │  2. Decorrelated Jitter Exponential Backoff Runner     │
      │  3. SSRF-Safe Outbound HTTP Client (Keep-Alive Pool)   │
      │  4. Dead-Letter Queue (DLQ) Eviction & Replay API      │
      └──────────────────────────┬─────────────────────────────┘
                                 │
                                 ▼ (HTTP POST)
                   [ Internal Downstream Microservices ]
```

## 2. Ingestion Layer & HTTP Transport

### 2.1 Framework Selection: Fastify (Node.js 22 LTS)
Fastify dipilih karena memiliki throughput tinggi (70.000 - 90.000 RPS raw), manajemen backpressure native melalui Node.js streams, memori RSS yang efisien (~40MB), dan ekosistem TypeScript kelas satu.

### 2.2 Raw Body Parsing Architecture
Untuk memverifikasi tanda tangan digital (HMAC-SHA256, Ed25519, SHA-512), gateway membutuhkan *byte-exact raw payload*. Fastify dikonfigurasi menggunakan custom content parser agar tidak membaca stream dua kali:
```typescript
fastify.addContentTypeParser('application/json', { parseAs: 'buffer' }, (req, body, done) => {
  try {
    const rawBody = body as Buffer;
    (req as any).rawBody = rawBody;
    const json = JSON.parse(rawBody.toString('utf-8'));
    done(null, json);
  } catch (err) {
    done(err as Error, undefined);
  }
});
```

### 2.3 Operating System Socket & Connection Tuning
- Socket limit: `net.core.somaxconn = 65535`
- TCP Reuse: `net.ipv4.tcp_tw_reuse = 1`
- File descriptors: `nofile = 1048576`
- Fastify server configuration: `connectionTimeout: 10000`, `keepAliveTimeout: 5000`.

## 3. Storage Engine & Database Schema

### 3.1 SQLite WAL Mode PRAGMA Optimization
Setiap koneksi database SQLite (`better-sqlite3`) wajib menjalankan PRAGMA berikut saat inisialisasi:
```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA busy_timeout = 5000;
PRAGMA cache_size = -64000; -- 64MB memory cache
PRAGMA mmap_size = 268435456; -- 256MB memory-mapped I/O
PRAGMA foreign_keys = ON;
PRAGMA temp_store = MEMORY;
```

### 3.2 Complete SQL Schema DDL

```sql
-- 1. Endpoints Table: Konfigurasi downstream target dan secret
CREATE TABLE IF NOT EXISTS endpoints (
    id TEXT PRIMARY KEY, -- UUIDv7
    name TEXT NOT NULL,
    target_url TEXT NOT NULL,
    provider_type TEXT NOT NULL, -- 'github' | 'stripe' | 'midtrans' | 'discord' | 'generic'
    secret_key TEXT NOT NULL,
    timeout_ms INTEGER NOT NULL DEFAULT 5000,
    max_retries INTEGER NOT NULL DEFAULT 5,
    circuit_state TEXT NOT NULL DEFAULT 'CLOSED', -- 'CLOSED' | 'OPEN' | 'HALF_OPEN'
    circuit_failures INTEGER NOT NULL DEFAULT 0,
    circuit_reset_at INTEGER, -- epoch ms
    created_at INTEGER NOT NULL
);

-- 2. Incoming Events Table: Rekam jejak seluruh event webhook yang diterima
CREATE TABLE IF NOT EXISTS incoming_events (
    id TEXT PRIMARY KEY, -- UUIDv7
    endpoint_id TEXT NOT NULL REFERENCES endpoints(id) ON DELETE CASCADE,
    idempotency_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'RECEIVED', -- 'RECEIVED' | 'PROCESSING' | 'DELIVERED' | 'FAILED' | 'DEAD'
    headers_json TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    raw_payload BLOB NOT NULL,
    attempts_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at INTEGER NOT NULL, -- epoch ms
    locked_until INTEGER, -- lease lock epoch ms
    created_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_events_idempotency 
ON incoming_events(endpoint_id, idempotency_key);

CREATE INDEX IF NOT EXISTS idx_events_dispatch_queue 
ON incoming_events(status, next_attempt_at) 
WHERE status IN ('RECEIVED', 'FAILED');

-- 3. Delivery Attempts Table: Histori log upaya pengiriman
CREATE TABLE IF NOT EXISTS delivery_attempts (
    id TEXT PRIMARY KEY, -- UUIDv7
    event_id TEXT NOT NULL REFERENCES incoming_events(id) ON DELETE CASCADE,
    attempt_number INTEGER NOT NULL,
    response_status INTEGER,
    response_body TEXT,
    error_message TEXT,
    execution_duration_ms INTEGER NOT NULL,
    attempted_at INTEGER NOT NULL
);

-- 4. Dead Letter Queue Table: Tempat isolasi event yang gagal tuntas
CREATE TABLE IF NOT EXISTS dead_letter_queue (
    id TEXT PRIMARY KEY, -- UUIDv7
    event_id TEXT NOT NULL REFERENCES incoming_events(id) ON DELETE CASCADE,
    endpoint_id TEXT NOT NULL REFERENCES endpoints(id) ON DELETE CASCADE,
    final_error TEXT NOT NULL,
    replayed_at INTEGER,
    replayed_by TEXT,
    created_at INTEGER NOT NULL
);
```

## 4. Idempotency State Machine & Race-Condition Defense

### 4.1 State Transitions
```text
[ RECEIVED ] ──(Worker Lease)──► [ PROCESSING ] ──(2xx Success)──► [ DELIVERED ]
                                        │
                               (Non-2xx / Network Fail)
                                        ▼
                                  [ FAILED ]
                                  │        ▲
               (attempts < max)   │        │ (Retry Tick)
               ───────────────────┘        │
                                           │
               (attempts >= max)           │
               ────────────────────────────┼──────────► [ DEAD (DLQ) ]
                                                              │
                                                        (Manual Replay)
```

### 4.2 Atomic CAS Transaction (`BEGIN IMMEDIATE`)
Untuk menjamin zero double-dispatch pada saat 50+ request duplikat masuk secara serentak, proses ingest menggunakan transaksi SQLite `BEGIN IMMEDIATE`:
```typescript
export function ingestEventAtomically(db: Database, event: NewEvent): IngestResult {
  const insertStmt = db.prepare(`
    INSERT INTO incoming_events (
      id, endpoint_id, idempotency_key, status, headers_json, payload_json, 
      raw_payload, attempts_count, next_attempt_at, created_at
    ) VALUES (
      @id, @endpoint_id, @idempotency_key, 'RECEIVED', @headers_json, @payload_json, 
      @raw_payload, 0, @next_attempt_at, @created_at
    )
  `);

  try {
    const tx = db.transaction(() => {
      insertStmt.run(event);
    });
    tx.immediate();
    return { status: 'ACCEPTED', eventId: event.id };
  } catch (err: any) {
    if (err.code === 'SQLITE_CONSTRAINT_UNIQUE' || err.message?.includes('UNIQUE constraint')) {
      return { status: 'DUPLICATE_CONFLICT', key: event.idempotency_key };
    }
    throw err;
  }
}
```

## 5. Dispatch Engine & Retry Topology

### 5.1 Advanced Retry Algorithm: Decorrelated Jitter Exponential Backoff
Sesuai standar AWS Architecture, AetherRelay menggunakan formula **Decorrelated Jitter** untuk mencegah fenomena thundering herd saat downstream pulih:
$$\text{sleep} = \min(\text{cap}, \text{random\_between}(\text{base}, \text{sleep}_{\text{previous}} \times 3))$$
Di mana:
- `base` = 1.000 ms (1 detik)
- `cap` = 300.000 ms (5 menit)
- `multiplier` = 3

### 5.2 Circuit Breaker State Machine per Endpoint
- **CLOSED**: Status normal. Semua request disalurkan ke downstream. Jika terjadi 5 kegagalan berturut-turut, beralih ke OPEN.
- **OPEN**: Endpoint downstream dianggap mati. Gateway langsung menjadwalkan ulang retry tanpa mengirimkan HTTP request keluar selama `recovery_timeout` (30 detik).
- **HALF-OPEN**: Setelah 30 detik, gateway mencoba mengirim 1 request uji (*canary probe*). Jika sukses, kembali ke CLOSED; jika gagal, kembali ke OPEN selama 60 detik.

### 5.3 Dead-Letter Queue (DLQ) Replay API
Endpoint: `POST /v1/dlq/:id/replay`
- Memvalidasi keberadaan event di tabel `dead_letter_queue`.
- Mengubah status event di `incoming_events` kembali ke `RECEIVED` dengan `attempts_count = 0` dan `next_attempt_at = Date.now()`.
- Menandai kolom `replayed_at` pada tabel DLQ.

## 6. Cryptographic Security & SSRF Protection

### 6.1 Constant-Time Signature Comparison
Untuk mencegah serangan *timing attack* dan kebocoran panjang buffer (*buffer length leakage*), komparasi tanda tangan selalu di-hash terlebih dahulu ke SHA-256 berukuran 32-byte tetap sebelum dibandingkan menggunakan `crypto.timingSafeEqual`:
```typescript
export function safeCompareSignatures(expected: string, actual: string): boolean {
  const hashA = crypto.createHash('sha256').update(expected).digest();
  const hashB = crypto.createHash('sha256').update(actual).digest();
  return crypto.timingSafeEqual(hashA, hashB);
}
```

### 6.2 Provider Signature Verification Adapters
- **GitHub**: Header `X-Hub-Signature-256`. Format: `sha256=<hex_hmac>`. Dihitung dari `crypto.createHmac('sha256', secret).update(rawBody).digest('hex')`.
- **Stripe**: Header `Stripe-Signature`. Format: `t=<timestamp>,v1=<signature>`. Toleransi drift timestamp maksimal 300 detik. Signature dihitung dari `HMAC-SHA256(secret, "${timestamp}.${rawBody}")`.
- **Midtrans**: Field `signature_key` pada JSON. Dihitung dari `crypto.createHash('sha512').update(orderId + statusCode + grossAmount + serverKey).digest('hex')`.
- **Generic HMAC**: Header `X-Signature-SHA256`.

### 6.3 Server-Side Request Forgery (SSRF) Defense
Untuk mencegah serangan pengiriman event ke internal network atau cloud metadata service, seluruh `target_url` downstream divalidasi saat pendaftaran dan sebelum dispatch:
- Memblokir IP privat RFC 1918 (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`).
- Memblokir Link-Local dan Loopback (`127.0.0.0/8`, `::1`).
- Memblokir Cloud Instance Metadata Service (`169.254.169.254`).
- Hanya mengizinkan protokol `http:` dan `https:`.

## 7. Observability & Telemetry

### 7.1 Zero-Overhead Pino Logging
Menggunakan Pino dengan asynchronous destination thread (`sonic-boom`). Setiap request wajib menyertakan context correlation ID (`x-request-id` / `traceparent`).

### 7.2 Prometheus Telemetry Metrics (`GET /metrics`)
- `aether_ingested_events_total{endpoint, provider, status}`
- `aether_ingest_duration_ms{quantile="0.5|0.9|0.95|0.99"}`
- `aether_dispatch_attempts_total{endpoint, response_code}`
- `aether_dispatch_duration_ms{endpoint, status}`
- `aether_circuit_breaker_state{endpoint}` (0=Closed, 1=Half-Open, 2=Open)
- `aether_dlq_events_total{endpoint}`
- `aether_active_lease_locks`
