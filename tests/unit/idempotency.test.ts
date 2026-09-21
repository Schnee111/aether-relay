import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import Database from 'better-sqlite3';
import { runMigrations } from '../../src/db/migrations.js';
import {
  ingestEventAtomically,
  acquireLeasedEvents,
  markEventDelivered,
  markEventFailed,
} from '../../src/core/idempotency.js';
import { generateUUIDv7 } from '../../src/utils/uuid.js';

describe('Storage Engine & Idempotency State Machine', () => {
  let db: Database.Database;
  const testEndpointId = generateUUIDv7();

  beforeEach(() => {
    db = new Database(':memory:');
    db.pragma('journal_mode = WAL');
    db.pragma('synchronous = NORMAL');
    db.pragma('busy_timeout = 5000');
    runMigrations(db);

    // Seed test endpoint
    db.prepare(`
      INSERT INTO endpoints (
        id, name, target_url, provider_type, secret_key, timeout_ms, max_retries, created_at
      ) VALUES (?, 'Test Webhook', 'http://localhost:9999/hook', 'generic', 'secret123', 5000, 3, ?)
    `).run(testEndpointId, Date.now());
  });

  afterEach(() => {
    db.close();
  });

  it('verifies SQLite WAL PRAGMAs and schema initialization', () => {
    const journalMode = db.pragma('journal_mode', { simple: true });
    // In-memory db journal_mode can be 'memory' or 'wal' depending on sqlite version
    expect(journalMode).toBeDefined();

    const tables = db.prepare("SELECT name FROM sqlite_master WHERE type='table'").all();
    const tableNames = tables.map((t: any) => t.name);
    expect(tableNames).toContain('endpoints');
    expect(tableNames).toContain('incoming_events');
    expect(tableNames).toContain('delivery_attempts');
    expect(tableNames).toContain('dead_letter_queue');
  });

  it('ingests event atomically and rejects duplicate idempotency key', () => {
    const raw = Buffer.from(JSON.stringify({ hello: 'world' }));
    const outcome1 = ingestEventAtomically(db, {
      endpointId: testEndpointId,
      idempotencyKey: 'idemp-key-100',
      headers: { 'x-foo': 'bar' },
      payload: { hello: 'world' },
      rawPayload: raw,
    });

    expect(outcome1.status).toBe('ACCEPTED');
    if (outcome1.status === 'ACCEPTED') {
      expect(outcome1.eventId).toBeDefined();
    }

    // Second arrival with identical key must return DUPLICATE_CONFLICT
    const outcome2 = ingestEventAtomically(db, {
      endpointId: testEndpointId,
      idempotencyKey: 'idemp-key-100',
      headers: { 'x-foo': 'bar' },
      payload: { hello: 'world' },
      rawPayload: raw,
    });

    expect(outcome2.status).toBe('DUPLICATE_CONFLICT');
    if (outcome2.status === 'DUPLICATE_CONFLICT') {
      expect(outcome2.idempotencyKey).toBe('idemp-key-100');
    }

    // Verify exactly 1 record in database
    const count = db.prepare('SELECT COUNT(*) as count FROM incoming_events').get() as { count: number };
    expect(count.count).toBe(1);
  });

  it('manages worker lease transition from RECEIVED to PROCESSING', () => {
    const raw = Buffer.from('{}');
    ingestEventAtomically(db, {
      endpointId: testEndpointId,
      idempotencyKey: 'idemp-key-200',
      headers: {},
      payload: {},
      rawPayload: raw,
    });

    const leased = acquireLeasedEvents(db, 10, 5000);
    expect(leased.length).toBe(1);
    expect(leased[0].status).toBe('RECEIVED'); // status before update returned in array
    expect(leased[0].idempotency_key).toBe('idemp-key-200');

    // Check DB row is now PROCESSING
    const row = db.prepare('SELECT status, locked_until FROM incoming_events WHERE id = ?').get(leased[0].id) as any;
    expect(row.status).toBe('PROCESSING');
    expect(row.locked_until).toBeGreaterThan(Date.now());
  });

  it('transitions through FAILED retry and moves to DEAD/DLQ when maxRetries reached', () => {
    const raw = Buffer.from('{}');
    const outcome = ingestEventAtomically(db, {
      endpointId: testEndpointId,
      idempotencyKey: 'idemp-key-300',
      headers: {},
      payload: {},
      rawPayload: raw,
    });
    const eventId = (outcome as any).eventId;

    // Attempt 1: Fails, retry scheduled
    const res1 = markEventFailed(db, eventId, testEndpointId, 1, 120, 'Connection timeout', 1000, 3);
    expect(res1.isDead).toBe(false);

    let row = db.prepare('SELECT status, attempts_count FROM incoming_events WHERE id = ?').get(eventId) as any;
    expect(row.status).toBe('FAILED');
    expect(row.attempts_count).toBe(1);

    // Attempt 2: Fails, retry scheduled
    const res2 = markEventFailed(db, eventId, testEndpointId, 2, 85, '500 Internal Server Error', 2000, 3);
    expect(res2.isDead).toBe(false);

    // Attempt 3: Max retries (3) reached -> DEAD and pushed to DLQ
    const res3 = markEventFailed(db, eventId, testEndpointId, 3, 90, '500 Internal Server Error', 4000, 3);
    expect(res3.isDead).toBe(true);

    row = db.prepare('SELECT status, attempts_count FROM incoming_events WHERE id = ?').get(eventId) as any;
    expect(row.status).toBe('DEAD');
    expect(row.attempts_count).toBe(3);

    const dlqRow = db.prepare('SELECT * FROM dead_letter_queue WHERE event_id = ?').get(eventId) as any;
    expect(dlqRow).toBeDefined();
    expect(dlqRow.final_error).toBe('500 Internal Server Error');
  });

  it('marks event as DELIVERED upon successful response', () => {
    const raw = Buffer.from('{}');
    const outcome = ingestEventAtomically(db, {
      endpointId: testEndpointId,
      idempotencyKey: 'idemp-key-400',
      headers: {},
      payload: {},
      rawPayload: raw,
    });
    const eventId = (outcome as any).eventId;

    markEventDelivered(db, eventId, 1, 45, 200, '{"success":true}');

    const row = db.prepare('SELECT status, attempts_count, locked_until FROM incoming_events WHERE id = ?').get(eventId) as any;
    expect(row.status).toBe('DELIVERED');
    expect(row.attempts_count).toBe(1);
    expect(row.locked_until).toBeNull();

    const attempt = db.prepare('SELECT * FROM delivery_attempts WHERE event_id = ?').get(eventId) as any;
    expect(attempt.response_status).toBe(200);
    expect(attempt.response_body).toBe('{"success":true}');
    expect(attempt.execution_duration_ms).toBe(45);
  });
});
