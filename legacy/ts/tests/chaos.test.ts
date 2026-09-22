import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import Database from 'better-sqlite3';
import { runMigrations } from '../src/db/migrations.js';
import { DispatchWorker } from '../src/worker/dispatcher.js';
import { generateUUIDv7 } from '../src/utils/uuid.js';
import { ingestEventAtomically } from '../src/core/idempotency.js';

describe('Chaos & Failure Injection: Circuit Breaker & DLQ Recovery', () => {
  let db: Database.Database;
  const endpointId = generateUUIDv7();

  beforeEach(() => {
    db = new Database(':memory:');
    runMigrations(db);

    // Target a dead port to simulate downstream network failure
    db.prepare(`
      INSERT INTO endpoints (
        id, name, target_url, provider_type, secret_key, timeout_ms, max_retries, created_at
      ) VALUES (?, 'Failing Downstream', 'http://127.0.0.1:54321/dead-service', 'generic', 'secret', 200, 3, ?)
    `).run(endpointId, Date.now());
  });

  afterEach(() => {
    db.close();
  });

  it('trips Circuit Breaker from CLOSED to OPEN after consecutive failures and evicts to DLQ', async () => {
    const worker = new DispatchWorker(db, { baseRetryDelayMs: 10, maxRetryDelayMs: 100 });
    const cb = worker.getCircuitBreaker(endpointId);
    expect(cb.state).toBe('CLOSED');

    // Ingest 1 event that will fail 3 times
    const outcome = ingestEventAtomically(db, {
      endpointId,
      idempotencyKey: 'chaos-key-01',
      headers: {},
      payload: { chaos: true },
      rawPayload: Buffer.from('{}'),
    });
    const eventId = (outcome as any).eventId;

    // Tick 1: Attempt 1 fails
    await worker.tick();
    let row = db.prepare('SELECT status, attempts_count FROM incoming_events WHERE id = ?').get(eventId) as any;
    expect(row.status).toBe('FAILED');
    expect(row.attempts_count).toBe(1);

    // Fast-forward next_attempt_at to simulate time passage
    db.prepare('UPDATE incoming_events SET next_attempt_at = 0 WHERE id = ?').run(eventId);

    // Tick 2: Attempt 2 fails
    await worker.tick();
    row = db.prepare('SELECT status, attempts_count FROM incoming_events WHERE id = ?').get(eventId) as any;
    expect(row.status).toBe('FAILED');
    expect(row.attempts_count).toBe(2);

    // Fast-forward next_attempt_at
    db.prepare('UPDATE incoming_events SET next_attempt_at = 0 WHERE id = ?').run(eventId);

    // Tick 3: Attempt 3 fails -> Max retries (3) reached! Event moves to DEAD (DLQ)
    await worker.tick();
    row = db.prepare('SELECT status, attempts_count FROM incoming_events WHERE id = ?').get(eventId) as any;
    expect(row.status).toBe('DEAD');
    expect(row.attempts_count).toBe(3);

    // Verify DLQ entry exists
    const dlqItem = db.prepare('SELECT * FROM dead_letter_queue WHERE event_id = ?').get(eventId) as any;
    expect(dlqItem).toBeDefined();
    expect(dlqItem.endpoint_id).toBe(endpointId);
    expect(dlqItem.final_error).toBeDefined();

    // Verify circuit breaker tripped
    // Fail 3 more events to reach threshold 5
    for (let i = 0; i < 3; i++) {
      cb.recordFailure();
    }
    expect(cb.state).toBe('OPEN');
    expect(cb.canExecute()).toBe(false);
  });
});
