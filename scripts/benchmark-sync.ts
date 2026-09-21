import Database from 'better-sqlite3';
import fs from 'node:fs';

interface BenchmarkResult {
  mode: string;
  totalTransactions: number;
  totalTimeMs: number;
  opsPerSec: number;
  p50Ms: number;
  p95Ms: number;
  p99Ms: number;
}

function runBenchmark(mode: 'NORMAL' | 'FULL', count: number = 2000): BenchmarkResult {
  const dbPath = `/tmp/bench_wal_${mode.toLowerCase()}.db`;
  if (fs.existsSync(dbPath)) fs.unlinkSync(dbPath);
  if (fs.existsSync(`${dbPath}-wal`)) fs.unlinkSync(`${dbPath}-wal`);
  if (fs.existsSync(`${dbPath}-shm`)) fs.unlinkSync(`${dbPath}-shm`);

  const db = new Database(dbPath);
  db.pragma('journal_mode = WAL');
  db.pragma(`synchronous = ${mode}`);
  db.pragma('busy_timeout = 5000');

  db.exec(`
    CREATE TABLE IF NOT EXISTS test_events (
      id TEXT PRIMARY KEY,
      payload TEXT,
      created_at INTEGER
    )
  `);

  const insert = db.prepare('INSERT INTO test_events (id, payload, created_at) VALUES (?, ?, ?)');
  const latencies: number[] = [];

  const startTime = performance.now();

  for (let i = 0; i < count; i++) {
    const t0 = performance.now();
    const tx = db.transaction(() => {
      insert.run(`evt_${mode}_${i}`, JSON.stringify({ index: i, timestamp: Date.now() }), Date.now());
    });
    tx.immediate();
    latencies.push(performance.now() - t0);
  }

  const totalTimeMs = performance.now() - startTime;
  db.close();

  // Cleanup
  if (fs.existsSync(dbPath)) fs.unlinkSync(dbPath);
  if (fs.existsSync(`${dbPath}-wal`)) fs.unlinkSync(`${dbPath}-wal`);
  if (fs.existsSync(`${dbPath}-shm`)) fs.unlinkSync(`${dbPath}-shm`);

  latencies.sort((a, b) => a - b);
  const p50 = latencies[Math.floor(count * 0.5)];
  const p95 = latencies[Math.floor(count * 0.95)];
  const p99 = latencies[Math.floor(count * 0.99)];

  return {
    mode,
    totalTransactions: count,
    totalTimeMs: Math.round(totalTimeMs),
    opsPerSec: Math.round((count / totalTimeMs) * 1000),
    p50Ms: Number(p50.toFixed(3)),
    p95Ms: Number(p95.toFixed(3)),
    p99Ms: Number(p99.toFixed(3)),
  };
}

console.log('Running Empirical SQLite WAL Benchmark: synchronous NORMAL vs FULL (2,000 tx each)...');
const resNormal = runBenchmark('NORMAL', 2000);
const resFull = runBenchmark('FULL', 2000);

console.log('\n--- EMPIRICAL BENCHMARK RESULTS ---');
console.table([resNormal, resFull]);

const speedupRatio = (resNormal.opsPerSec / resFull.opsPerSec).toFixed(2);
console.log(`Throughput Speedup Ratio (NORMAL vs FULL): ${speedupRatio}x`);
