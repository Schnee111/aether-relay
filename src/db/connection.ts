import Database from 'better-sqlite3';
import path from 'node:path';
import fs from 'node:fs';

export interface DbConfig {
  dbPath?: string;
  synchronous?: 'NORMAL' | 'FULL' | 'OFF';
  busyTimeout?: number;
}

let dbInstance: Database.Database | null = null;

export function getDatabase(config: DbConfig = {}): Database.Database {
  if (dbInstance) {
    return dbInstance;
  }

  const dbPath = config.dbPath || process.env.DATABASE_PATH || path.resolve(process.cwd(), 'data/aether.db');
  
  // Ensure directory exists
  if (dbPath !== ':memory:') {
    const dir = path.dirname(dbPath);
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }
  }

  const db = new Database(dbPath);

  // WAL and concurrency pragmas
  db.pragma('journal_mode = WAL');
  const syncMode = config.synchronous || (process.env.SQLITE_SYNCHRONOUS as any) || 'NORMAL';
  db.pragma(`synchronous = ${syncMode}`);
  const busyTimeout = config.busyTimeout ?? 5000;
  db.pragma(`busy_timeout = ${busyTimeout}`);
  db.pragma('cache_size = -64000'); // 64MB cache
  db.pragma('mmap_size = 268435456'); // 256MB memory map
  db.pragma('foreign_keys = ON');
  db.pragma('temp_store = MEMORY');

  dbInstance = db;
  return db;
}

export function closeDatabase(): void {
  if (dbInstance) {
    try {
      dbInstance.close();
    } finally {
      dbInstance = null;
    }
  }
}

/**
 * Reset singleton (useful for testing)
 */
export function resetDatabaseInstance(): void {
  closeDatabase();
}
