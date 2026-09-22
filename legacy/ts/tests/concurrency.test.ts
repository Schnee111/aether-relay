import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import Database from 'better-sqlite3';
import crypto from 'node:crypto';
import { runMigrations } from '../src/db/migrations.js';
import { buildServer } from '../src/server.js';
import { generateUUIDv7 } from '../src/utils/uuid.js';

describe('Extreme Concurrency Flood: Idempotency Race Guard', () => {
  let db: Database.Database;
  let server: any;
  const endpointId = generateUUIDv7();
  const secret = 'whsec_concurrency_race_guard_key';

  beforeEach(async () => {
    db = new Database(':memory:');
    db.pragma('journal_mode = WAL');
    db.pragma('synchronous = NORMAL');
    db.pragma('busy_timeout = 5000');
    runMigrations(db);

    db.prepare(`
      INSERT INTO endpoints (
        id, name, target_url, provider_type, secret_key, timeout_ms, max_retries, created_at
      ) VALUES (?, 'Concurrency Endpoint', 'http://localhost:9999/hook', 'github', ?, 5000, 3, ?)
    `).run(endpointId, secret, Date.now());

    server = buildServer(db);
    await server.ready();
  });

  afterEach(async () => {
    await server.close();
    db.close();
  });

  it('handles 50 concurrent requests with identical Idempotency-Key with exactly 1 accepted and 49 conflicts', async () => {
    const payload = JSON.stringify({ event: 'order.created', order_id: 'ORD-RACE-777', amount: 99000 });
    const hmac = 'sha256=' + crypto.createHmac('sha256', secret).update(Buffer.from(payload)).digest('hex');
    const sharedIdempotencyKey = 'shared-race-key-001';

    // Dispatch 50 concurrent requests simultaneously
    const requests = Array.from({ length: 50 }, () =>
      server.inject({
        method: 'POST',
        url: `/v1/ingest/${endpointId}`,
        headers: {
          'content-type': 'application/json',
          'idempotency-key': sharedIdempotencyKey,
          'x-hub-signature-256': hmac,
        },
        payload,
      })
    );

    const responses = await Promise.all(requests);

    const accepted = responses.filter((r) => r.statusCode === 202);
    const conflicts = responses.filter((r) => r.statusCode === 409);

    expect(accepted.length).toBe(1);
    expect(conflicts.length).toBe(49);

    // Verify database row count: MUST BE EXACTLY 1 ROW
    const rowCount = db.prepare('SELECT COUNT(*) as cnt FROM incoming_events WHERE idempotency_key = ?').get(sharedIdempotencyKey) as { cnt: number };
    expect(rowCount.cnt).toBe(1);

    // Verify the accepted response contains eventId
    const body = JSON.parse(accepted[0].body);
    expect(body.status).toBe('ACCEPTED');
    expect(body.eventId).toBeDefined();

    // Verify conflict responses contain clear error payload
    const conflictBody = JSON.parse(conflicts[0].body);
    expect(conflictBody.status).toBe('DUPLICATE_CONFLICT');
    expect(conflictBody.idempotencyKey).toBe(sharedIdempotencyKey);
  });
});
