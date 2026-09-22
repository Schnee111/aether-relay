import type Database from 'better-sqlite3';

export function runMigrations(db: Database.Database): void {
  db.exec(`
    CREATE TABLE IF NOT EXISTS endpoints (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      target_url TEXT NOT NULL,
      provider_type TEXT NOT NULL,
      secret_key TEXT NOT NULL,
      timeout_ms INTEGER NOT NULL DEFAULT 5000,
      max_retries INTEGER NOT NULL DEFAULT 5,
      circuit_state TEXT NOT NULL DEFAULT 'CLOSED',
      circuit_failures INTEGER NOT NULL DEFAULT 0,
      circuit_reset_at INTEGER,
      created_at INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS incoming_events (
      id TEXT PRIMARY KEY,
      endpoint_id TEXT NOT NULL REFERENCES endpoints(id) ON DELETE CASCADE,
      idempotency_key TEXT NOT NULL,
      status TEXT NOT NULL DEFAULT 'RECEIVED',
      headers_json TEXT NOT NULL,
      payload_json TEXT NOT NULL,
      raw_payload BLOB NOT NULL,
      attempts_count INTEGER NOT NULL DEFAULT 0,
      next_attempt_at INTEGER NOT NULL,
      locked_until INTEGER,
      created_at INTEGER NOT NULL
    );

    CREATE UNIQUE INDEX IF NOT EXISTS idx_events_idempotency 
    ON incoming_events(endpoint_id, idempotency_key);

    CREATE INDEX IF NOT EXISTS idx_events_dispatch_queue 
    ON incoming_events(status, next_attempt_at) 
    WHERE status IN ('RECEIVED', 'FAILED');

    CREATE TABLE IF NOT EXISTS delivery_attempts (
      id TEXT PRIMARY KEY,
      event_id TEXT NOT NULL REFERENCES incoming_events(id) ON DELETE CASCADE,
      attempt_number INTEGER NOT NULL,
      response_status INTEGER,
      response_body TEXT,
      error_message TEXT,
      execution_duration_ms INTEGER NOT NULL,
      attempted_at INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS dead_letter_queue (
      id TEXT PRIMARY KEY,
      event_id TEXT NOT NULL REFERENCES incoming_events(id) ON DELETE CASCADE,
      endpoint_id TEXT NOT NULL REFERENCES endpoints(id) ON DELETE CASCADE,
      final_error TEXT NOT NULL,
      replayed_at INTEGER,
      replayed_by TEXT,
      created_at INTEGER NOT NULL
    );
  `);
}
