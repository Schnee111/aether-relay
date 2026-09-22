use rusqlite::{Connection, Result};

/// Schema version, stored via `PRAGMA user_version`.
///
/// Bump this whenever a statement below changes, and add the corresponding
/// migration step: a schema change that is not guarded by the version will
/// silently apply to existing databases.
pub const SCHEMA_VERSION: i64 = 1;

pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS endpoints (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    provider TEXT NOT NULL,
    secret TEXT NOT NULL,
    target_url TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS incoming_events (
    id TEXT PRIMARY KEY,
    endpoint_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    raw_body BLOB NOT NULL,
    headers TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE(endpoint_id, idempotency_key),
    FOREIGN KEY(endpoint_id) REFERENCES endpoints(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_events_status ON incoming_events(status);
CREATE INDEX IF NOT EXISTS idx_events_endpoint ON incoming_events(endpoint_id);

CREATE TABLE IF NOT EXISTS delivery_attempts (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    attempt_number INTEGER NOT NULL,
    response_status INTEGER,
    response_body TEXT,
    error_message TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(event_id) REFERENCES incoming_events(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS dead_letter_queue (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    endpoint_id TEXT NOT NULL,
    error_reason TEXT NOT NULL,
    last_attempt_status INTEGER,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(event_id) REFERENCES incoming_events(id) ON DELETE CASCADE
);
"#;

pub fn run_migrations(conn: &Connection) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if current >= SCHEMA_VERSION {
        return Ok(());
    }

    conn.execute_batch(SCHEMA_SQL)?;
    conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;

    Ok(())
}
