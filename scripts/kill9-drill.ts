import Database from 'better-sqlite3';
import { fork } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const dbPath = '/tmp/aether_kill9_drill.db';

// Clean old files
if (fs.existsSync(dbPath)) fs.unlinkSync(dbPath);
if (fs.existsSync(`${dbPath}-wal`)) fs.unlinkSync(`${dbPath}-wal`);
if (fs.existsSync(`${dbPath}-shm`)) fs.unlinkSync(`${dbPath}-shm`);

// Child writer script
const childWorkerCode = `
import Database from 'better-sqlite3';
const db = new Database('${dbPath}');
db.pragma('journal_mode = WAL');
db.pragma('synchronous = NORMAL');
db.pragma('busy_timeout = 5000');

db.exec(\`
  CREATE TABLE IF NOT EXISTS test_wal_events (
    id INTEGER PRIMARY KEY,
    data TEXT,
    timestamp INTEGER
  )
\`);

const insert = db.prepare('INSERT INTO test_wal_events (id, data, timestamp) VALUES (?, ?, ?)');

for (let i = 1; i <= 2000; i++) {
  const tx = db.transaction(() => {
    insert.run(i, 'crash_durability_payload_' + i, Date.now());
  });
  tx.immediate();
  if (i === 200) {
    if (process.send) process.send({ status: 'READY_FOR_KILL', count: i });
  }
}
`;

const workerFile = path.resolve(process.cwd(), 'scripts/kill9-worker.ts');
fs.writeFileSync(workerFile, childWorkerCode);

async function runKill9Drill() {
  console.log('=== RUNNING THE KILL -9 CRASH DURABILITY DRILL ===');
  console.log('Spawning worker child process writing 2,000 transactions to WAL...');

  const child = fork(workerFile, [], {
    execArgv: ['--import', 'tsx'],
  });

  await new Promise<void>((resolve) => {
    child.on('message', (msg: any) => {
      if (msg.status === 'READY_FOR_KILL') {
        console.log(`Worker wrote ${msg.count} transactions. Firing SIGKILL (kill -9) NOW!`);
        child.kill('SIGKILL');
        resolve();
      }
    });
  });

  // Give OS a moment to release file lock
  await new Promise((r) => setTimeout(r, 200));

  console.log('Worker terminated violently via SIGKILL.');
  console.log('Re-opening database and executing PRAGMA integrity checks...');

  const db = new Database(dbPath);
  db.pragma('journal_mode = WAL');
  db.pragma('synchronous = NORMAL');

  const integrityCheck = db.pragma('integrity_check', { simple: true });
  const quickCheck = db.pragma('quick_check', { simple: true });
  const countRow = db.prepare('SELECT COUNT(*) as count FROM test_wal_events').get() as { count: number };

  console.log(`PRAGMA integrity_check: ${integrityCheck}`);
  console.log(`PRAGMA quick_check: ${quickCheck}`);
  console.log(`Committed transactions recovered cleanly: ${countRow.count}`);

  db.close();

  // Cleanup
  fs.unlinkSync(workerFile);
  if (fs.existsSync(dbPath)) fs.unlinkSync(dbPath);
  if (fs.existsSync(`${dbPath}-wal`)) fs.unlinkSync(`${dbPath}-wal`);
  if (fs.existsSync(`${dbPath}-shm`)) fs.unlinkSync(`${dbPath}-shm`);

  if (integrityCheck === 'ok' && quickCheck === 'ok' && countRow.count >= 200) {
    console.log('DRILL RESULT: PASS! Zero corruption detected, SQLite WAL fully crash-durable.');
  } else {
    console.error('DRILL RESULT: FAIL!');
    process.exit(1);
  }
}

runKill9Drill().catch((err) => {
  console.error(err);
  process.exit(1);
});
