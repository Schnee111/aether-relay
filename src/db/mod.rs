pub mod migrations;
pub mod models;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub type DbPool = Pool<SqliteConnectionManager>;

/// Failure to bring the pool up. Migrations and file permissions are part of
/// startup: a database that cannot be initialised correctly must stop the
/// process rather than be silently tolerated.
#[derive(Debug)]
pub enum PoolInitError {
    Pool(r2d2::Error),
    Migration(rusqlite::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for PoolInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pool(e) => write!(f, "failed to build connection pool: {e}"),
            Self::Migration(e) => write!(f, "failed to run schema migrations: {e}"),
            Self::Io(e) => write!(f, "failed to restrict database file permissions: {e}"),
        }
    }
}

impl std::error::Error for PoolInitError {}

impl From<r2d2::Error> for PoolInitError {
    fn from(e: r2d2::Error) -> Self {
        Self::Pool(e)
    }
}

impl From<rusqlite::Error> for PoolInitError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Migration(e)
    }
}

impl From<std::io::Error> for PoolInitError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

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
) -> Result<DbPool, PoolInitError> {
    let db_path = db_path.as_ref();

    if let Some(parent) = db_path.parent()
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

    // Run migrations on a real connection. A schema that failed to apply used
    // to be swallowed here, which left the process running against a database
    // missing its tables and failing every request instead of failing startup.
    {
        let conn = pool.get()?;
        migrations::run_migrations(&conn)?;
    }

    // The endpoints table stores provider signing secrets verbatim (HMAC needs
    // the original key, so hashing is not an option). Keep the file readable
    // only by its owner.
    restrict_db_permissions(db_path)?;

    Ok(pool)
}

/// Tighten the database file to owner-only.
///
/// `-wal` and `-shm` carry the same pages as the main file, so they are
/// restricted too; the main file is restricted last so a partially applied
/// change still leaves the primary artefact protected.
#[cfg(unix)]
fn restrict_db_permissions(db_path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    if db_path.to_string_lossy() == ":memory:" {
        return Ok(());
    }

    for candidate in [db_path.to_path_buf(), wal_path(db_path), shm_path(db_path)] {
        if candidate.exists() {
            std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o600))?;
        }
    }

    Ok(())
}

#[cfg(not(unix))]
fn restrict_db_permissions(_db_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn wal_path(db_path: &Path) -> PathBuf {
    let mut s = db_path.as_os_str().to_os_string();
    s.push("-wal");
    PathBuf::from(s)
}

fn shm_path(db_path: &Path) -> PathBuf {
    let mut s = db_path.as_os_str().to_os_string();
    s.push("-shm");
    PathBuf::from(s)
}
