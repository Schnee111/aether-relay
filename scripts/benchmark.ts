// @ts-ignore
import autocannon from 'autocannon';
import Database from 'better-sqlite3';
import crypto from 'node:crypto';
import fs from 'node:fs';
import { runMigrations } from '../src/db/migrations.js';
import { buildServer } from '../src/server.js';
import { generateUUIDv7 } from '../src/utils/uuid.js';

async function runAutocannonBenchmark() {
  const dbPath = '/tmp/bench_autocannon.db';
  if (fs.existsSync(dbPath)) fs.unlinkSync(dbPath);
  if (fs.existsSync(`${dbPath}-wal`)) fs.unlinkSync(`${dbPath}-wal`);
  if (fs.existsSync(`${dbPath}-shm`)) fs.unlinkSync(`${dbPath}-shm`);

  const db = new Database(dbPath);
  db.pragma('journal_mode = WAL');
  db.pragma('synchronous = NORMAL');
  db.pragma('busy_timeout = 5000');
  runMigrations(db);

  const endpointId = generateUUIDv7();
  const secret = 'whsec_bench_secret_key_123';

  db.prepare(`
    INSERT INTO endpoints (
      id, name, target_url, provider_type, secret_key, timeout_ms, max_retries, created_at
    ) VALUES (?, 'Bench Endpoint', 'http://127.0.0.1:9999/sink', 'generic', ?, 5000, 3, ?)
  `).run(endpointId, secret, Date.now());

  const server = buildServer(db);
  const address = await server.listen({ port: 0, host: '127.0.0.1' });
  const port = (server.server.address() as any).port;

  const payload = JSON.stringify({ event: 'benchmark.event', count: 1 });
  const rawBody = Buffer.from(payload);
  const hmac = 'sha256=' + crypto.createHmac('sha256', secret).update(rawBody).digest('hex');

  const initialMemoryMb = (process.memoryUsage().rss / 1024 / 1024).toFixed(2);
  console.log(`[Benchmark] Server listening on http://127.0.0.1:${port}`);
  console.log(`[Benchmark] Initial Memory RSS: ${initialMemoryMb} MB`);
  console.log('[Benchmark] Firing Autocannon load test (10s, 20 connections)...');

  const result = await autocannon({
    url: `http://127.0.0.1:${port}/v1/ingest/${endpointId}`,
    method: 'POST',
    connections: 20,
    duration: 10,
    headers: {
      'content-type': 'application/json',
      'x-signature-sha256': hmac,
    },
    body: payload,
  });

  const finalMemoryMb = (process.memoryUsage().rss / 1024 / 1024).toFixed(2);
  console.log('\n=== AUTOCANNON LOAD BENCHMARK RESULTS ===');
  console.log(`Throughput (Requests/sec): ${result.requests.average}`);
  console.log(`Total Requests:            ${result.requests.total}`);
  console.log(`Latency p50:               ${result.latency.p50} ms`);
  console.log(`Latency p95:               ${result.latency.p95} ms`);
  console.log(`Latency p99:               ${result.latency.p99} ms`);
  console.log(`Initial Memory RSS:        ${initialMemoryMb} MB`);
  console.log(`Final Memory RSS:          ${finalMemoryMb} MB`);
  console.log(`Memory Stability:          Delta ${((parseFloat(finalMemoryMb) - parseFloat(initialMemoryMb))).toFixed(2)} MB`);

  await server.close();
  db.close();

  if (fs.existsSync(dbPath)) fs.unlinkSync(dbPath);
  if (fs.existsSync(`${dbPath}-wal`)) fs.unlinkSync(`${dbPath}-wal`);
  if (fs.existsSync(`${dbPath}-shm`)) fs.unlinkSync(`${dbPath}-shm`);
}

runAutocannonBenchmark().catch(console.error);
