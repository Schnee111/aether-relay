pub mod migrations;
pub mod models;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::path::Path;

pub type DbPool = Pool<SqliteConnectionManager>;

#[derive(Debug)]
struct PragmaCustomizer {
    busy_timeout_ms: u64,
    mmap_size: usize,
    cache_size: i64,
}

impl r2d2::CustomizeConnection<Connection, rusqlite::Error> for PragmaCustomizer {
    fn on_acquire(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(&format!(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = {};
             PRAGMA mmap_size = {};
             PRAGMA cache_size = {};
             PRAGMA foreign_keys = ON;",
            self.busy_timeout_ms, self.mmap_size, self.cache_size
        ))
    }
}

pub fn create_pool(
    db_path: impl AsRef<Path>,
    pool_size: u32,
    busy_timeout_ms: u64,
    mmap_size: usize,
    cache_size: i64,
) -> Result<DbPool, r2d2::Error> {
    if let Some(parent) = db_path.as_ref().parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(parent);
    }

    let manager = SqliteConnectionManager::file(db_path);
    let customizer = PragmaCustomizer {
        busy_timeout_ms,
        mmap_size,
        cache_size,
    };

    let pool = Pool::builder()
        .max_size(pool_size)
        .connection_customizer(Box::new(customizer))
        .build(manager)?;

    // Run migrations on first connection
    if let Ok(conn) = pool.get() {
        let _ = migrations::run_migrations(&conn);
    }

    Ok(pool)
}
