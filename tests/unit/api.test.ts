import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import Database from 'better-sqlite3';
import crypto from 'node:crypto';
import { runMigrations } from '../../src/db/migrations.js';
import { buildServer } from '../../src/server.js';
import { generateUUIDv7 } from '../../src/utils/uuid.js';

describe('Fastify Ingestion API & DLQ Replay', () => {
  let db: Database.Database;
  let server: any;
  const testEndpointId = generateUUIDv7();
  const testSecret = 'secret_wh_key_999';

  beforeEach(async () => {
    db = new Database(':memory:');
    runMigrations(db);

    db.prepare(`
      INSERT INTO endpoints (
        id, name, target_url, provider_type, secret_key, timeout_ms, max_retries, created_at
      ) VALUES (?, 'GitHub Endpoint', 'http://localhost:9999/dest', 'github', ?, 5000, 3, ?)
    `).run(testEndpointId, testSecret, Date.now());

    server = buildServer(db);
    await server.ready();
  });

  afterEach(async () => {
    await server.close();
    db.close();
  });

  it('GET /health returns status ok', async () => {
    const res = await server.inject({
      method: 'GET',
      url: '/health',
    });
    expect(res.statusCode).toBe(200);
    const body = JSON.parse(res.body);
    expect(body.status).toBe('ok');
    expect(body.db).toBe(true);
  });

  it('POST /v1/ingest/:endpointId rejects invalid signature with 401 Unauthorized', async () => {
    const payload = JSON.stringify({ action: 'ping' });
    const res = await server.inject({
      method: 'POST',
      url: `/v1/ingest/${testEndpointId}`,
      headers: {
        'content-type': 'application/json',
        'x-hub-signature-256': 'sha256=invalidhex00000000000000000000000000000000',
      },
      payload,
    });
    expect(res.statusCode).toBe(401);
  });

  it('POST /v1/ingest/:endpointId ingests valid signature with 202 Accepted', async () => {
    const payload = JSON.stringify({ action: 'ping', zen: 'Practicality beats purity' });
    const validHmac = 'sha256=' + crypto.createHmac('sha256', testSecret).update(Buffer.from(payload)).digest('hex');

    const res = await server.inject({
      method: 'POST',
      url: `/v1/ingest/${testEndpointId}`,
      headers: {
        'content-type': 'application/json',
        'idempotency-key': 'test-idemp-key-555',
        'x-hub-signature-256': validHmac,
      },
      payload,
    });

    expect(res.statusCode).toBe(202);
    const body = JSON.parse(res.body);
    expect(body.status).toBe('ACCEPTED');
    expect(body.eventId).toBeDefined();

    // Replay same request -> 409 Conflict
    const resConflict = await server.inject({
      method: 'POST',
      url: `/v1/ingest/${testEndpointId}`,
      headers: {
        'content-type': 'application/json',
        'idempotency-key': 'test-idemp-key-555',
        'x-hub-signature-256': validHmac,
      },
      payload,
    });
    expect(resConflict.statusCode).toBe(409);
  });

  it('POST /v1/dlq/:id/replay resets dead letter event back to RECEIVED', async () => {
    const eventId = generateUUIDv7();
    const dlqId = generateUUIDv7();
    const now = Date.now();

    db.prepare(`
      INSERT INTO incoming_events (
        id, endpoint_id, idempotency_key, status, headers_json, payload_json, 
        raw_payload, attempts_count, next_attempt_at, created_at
      ) VALUES (?, ?, 'dlq-test-key', 'DEAD', '{}', '{}', X'7b7d', 3, ?, ?)
    `).run(eventId, testEndpointId, now, now);

    db.prepare(`
      INSERT INTO dead_letter_queue (
        id, event_id, endpoint_id, final_error, created_at
      ) VALUES (?, ?, ?, 'Simulated 500 downstream failure', ?)
    `).run(dlqId, eventId, testEndpointId, now);

    const res = await server.inject({
      method: 'POST',
      url: `/v1/dlq/${dlqId}/replay`,
      payload: { actor: 'admin' },
    });

    expect(res.statusCode).toBe(200);
    const body = JSON.parse(res.body);
    expect(body.status).toBe('REPLAY_QUEUED');

    const updatedEvent = db.prepare('SELECT status, attempts_count FROM incoming_events WHERE id = ?').get(eventId) as any;
    expect(updatedEvent.status).toBe('RECEIVED');
    expect(updatedEvent.attempts_count).toBe(0);
  });
});
