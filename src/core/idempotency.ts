import type Database from 'better-sqlite3';
import { generateUUIDv7 } from '../utils/uuid.js';
import type { IncomingEventRow, EventStatus } from '../db/schema.js';

export interface NewEventInput {
  endpointId: string;
  idempotencyKey: string;
  headers: Record<string, any>;
  payload: any;
  rawPayload: Buffer;
}

export type IngestOutcome =
  | { status: 'ACCEPTED'; eventId: string }
  | { status: 'DUPLICATE_CONFLICT'; existingEventId?: string; idempotencyKey: string };

/**
 * Atomically ingest an incoming webhook event.
 * Uses SQLite 'BEGIN IMMEDIATE' transaction to eliminate SELECT-then-INSERT race conditions.
 */
export function ingestEventAtomically(
  db: Database.Database,
  input: NewEventInput
): IngestOutcome {
  const eventId = generateUUIDv7();
  const now = Date.now();
  const headersJson = JSON.stringify(input.headers);
  const payloadJson = typeof input.payload === 'string' ? input.payload : JSON.stringify(input.payload);

  const insertStmt = db.prepare(`
    INSERT INTO incoming_events (
      id, endpoint_id, idempotency_key, status, headers_json, payload_json, 
      raw_payload, attempts_count, next_attempt_at, locked_until, created_at
    ) VALUES (
      ?, ?, ?, 'RECEIVED', ?, ?, ?, 0, ?, NULL, ?
    )
  `);

  const findExistingStmt = db.prepare(`
    SELECT id, status FROM incoming_events 
    WHERE endpoint_id = ? AND idempotency_key = ?
  `);

  try {
    const tx = db.transaction(() => {
      insertStmt.run(
        eventId,
        input.endpointId,
        input.idempotencyKey,
        headersJson,
        payloadJson,
        input.rawPayload,
        now,
        now
      );
    });

    tx.immediate();
    return { status: 'ACCEPTED', eventId };
  } catch (err: any) {
    if (err.code === 'SQLITE_CONSTRAINT_UNIQUE' || err.message?.includes('UNIQUE constraint')) {
      const existing = findExistingStmt.get(input.endpointId, input.idempotencyKey) as { id: string; status: string } | undefined;
      return {
        status: 'DUPLICATE_CONFLICT',
        existingEventId: existing?.id,
        idempotencyKey: input.idempotencyKey,
      };
    }
    throw err;
  }
}

/**
 * Acquire batch lease for worker dispatch using atomic transaction.
 */
export function acquireLeasedEvents(
  db: Database.Database,
  limit: number = 10,
  leaseMs: number = 15000
): IncomingEventRow[] {
  const now = Date.now();
  const lockedUntil = now + leaseMs;

  const selectStmt = db.prepare(`
    SELECT * FROM incoming_events
    WHERE status IN ('RECEIVED', 'FAILED')
      AND next_attempt_at <= ?
      AND (locked_until IS NULL OR locked_until < ?)
    ORDER BY next_attempt_at ASC
    LIMIT ?
  `);

  const updateStmt = db.prepare(`
    UPDATE incoming_events
    SET status = 'PROCESSING',
        locked_until = ?
    WHERE id = ?
  `);

  const tx = db.transaction(() => {
    const rows = selectStmt.all(now, now, limit) as IncomingEventRow[];
    for (const row of rows) {
      updateStmt.run(lockedUntil, row.id);
    }
    return rows;
  });

  return tx.immediate();
}

/**
 * Mark event as successfully delivered.
 */
export function markEventDelivered(
  db: Database.Database,
  eventId: string,
  attemptNumber: number,
  durationMs: number,
  responseStatus: number,
  responseBody?: string
): void {
  const attemptId = generateUUIDv7();
  const now = Date.now();

  const insertAttempt = db.prepare(`
    INSERT INTO delivery_attempts (
      id, event_id, attempt_number, response_status, response_body, 
      error_message, execution_duration_ms, attempted_at
    ) VALUES (?, ?, ?, ?, ?, NULL, ?, ?)
  `);

  const updateEvent = db.prepare(`
    UPDATE incoming_events
    SET status = 'DELIVERED',
        attempts_count = attempts_count + 1,
        locked_until = NULL
    WHERE id = ?
  `);

  const tx = db.transaction(() => {
    insertAttempt.run(attemptId, eventId, attemptNumber, responseStatus, responseBody ?? null, durationMs, now);
    updateEvent.run(eventId);
  });

  tx.immediate();
}

/**
 * Record a failed attempt and reschedule or evict to Dead Letter Queue (DLQ).
 */
export function markEventFailed(
  db: Database.Database,
  eventId: string,
  endpointId: string,
  attemptNumber: number,
  durationMs: number,
  errorMessage: string,
  nextAttemptDelayMs: number,
  maxRetries: number
): { isDead: boolean } {
  const attemptId = generateUUIDv7();
  const now = Date.now();
  const nextAttemptAt = now + nextAttemptDelayMs;
  const isDead = attemptNumber >= maxRetries;

  const insertAttempt = db.prepare(`
    INSERT INTO delivery_attempts (
      id, event_id, attempt_number, response_status, response_body, 
      error_message, execution_duration_ms, attempted_at
    ) VALUES (?, ?, ?, NULL, NULL, ?, ?, ?)
  `);

  const updateEvent = db.prepare(`
    UPDATE incoming_events
    SET status = ?,
        attempts_count = ?,
        next_attempt_at = ?,
        locked_until = NULL
    WHERE id = ?
  `);

  const insertDlq = db.prepare(`
    INSERT INTO dead_letter_queue (
      id, event_id, endpoint_id, final_error, replayed_at, replayed_by, created_at
    ) VALUES (?, ?, ?, ?, NULL, NULL, ?)
  `);

  const tx = db.transaction(() => {
    insertAttempt.run(attemptId, eventId, attemptNumber, errorMessage, durationMs, now);
    if (isDead) {
      updateEvent.run('DEAD', attemptNumber, nextAttemptAt, eventId);
      insertDlq.run(generateUUIDv7(), eventId, endpointId, errorMessage, now);
    } else {
      updateEvent.run('FAILED', attemptNumber, nextAttemptAt, eventId);
    }
  });

  tx.immediate();
  return { isDead };
}
