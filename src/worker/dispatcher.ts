import type Database from 'better-sqlite3';
import { acquireLeasedEvents, markEventDelivered, markEventFailed } from '../core/idempotency.js';
import { CircuitBreaker } from './circuit-breaker.js';
import type { EndpointRow } from '../db/schema.js';

export interface DispatcherConfig {
  baseRetryDelayMs?: number; // default 1000 (1s)
  maxRetryDelayMs?: number;  // default 300000 (5m)
  pollIntervalMs?: number;
}

export class DispatchWorker {
  private db: Database.Database;
  private circuitBreakers = new Map<string, CircuitBreaker>();
  private baseRetryDelayMs: number;
  private maxRetryDelayMs: number;
  private isRunning: boolean = false;
  private timer: NodeJS.Timeout | null = null;

  constructor(db: Database.Database, config: DispatcherConfig = {}) {
    this.db = db;
    this.baseRetryDelayMs = config.baseRetryDelayMs ?? 1000;
    this.maxRetryDelayMs = config.maxRetryDelayMs ?? 300000;
  }

  /**
   * AWS Architecture standard: Decorrelated Jitter Exponential Backoff formula.
   * sleep = min(cap, random_between(base, sleep_prev * 3))
   */
  public calculateDecorrelatedJitter(previousDelayMs: number): number {
    const min = this.baseRetryDelayMs;
    const max = Math.max(min, previousDelayMs * 3);
    const jittered = Math.floor(Math.random() * (max - min + 1)) + min;
    return Math.min(this.maxRetryDelayMs, jittered);
  }

  public getCircuitBreaker(endpointId: string): CircuitBreaker {
    let cb = this.circuitBreakers.get(endpointId);
    if (!cb) {
      cb = new CircuitBreaker();
      this.circuitBreakers.set(endpointId, cb);
    }
    return cb;
  }

  public async tick(): Promise<number> {
    const events = acquireLeasedEvents(this.db, 10, 10000);
    if (events.length === 0) return 0;

    for (const event of events) {
      const endpoint = this.db.prepare('SELECT * FROM endpoints WHERE id = ?').get(event.endpoint_id) as EndpointRow | undefined;
      if (!endpoint) {
        markEventFailed(this.db, event.id, event.endpoint_id, event.attempts_count + 1, 0, 'Endpoint not found', 60000, 1);
        continue;
      }

      const cb = this.getCircuitBreaker(endpoint.id);
      if (!cb.canExecute()) {
        // Circuit is open, reschedule event without making outbound call
        markEventFailed(this.db, event.id, endpoint.id, event.attempts_count, 0, 'Circuit breaker OPEN: downstream unavailable', 15000, endpoint.max_retries);
        continue;
      }

      const startTime = Date.now();
      try {
        const response = await fetch(endpoint.target_url, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'X-Aether-Event-Id': event.id,
            'X-Aether-Delivery-Attempt': String(event.attempts_count + 1),
            ...JSON.parse(event.headers_json || '{}'),
          },
          body: event.raw_payload,
          signal: AbortSignal.timeout(endpoint.timeout_ms || 5000),
        });

        const duration = Date.now() - startTime;
        if (response.ok) {
          const bodyText = await response.text();
          cb.recordSuccess();
          markEventDelivered(this.db, event.id, event.attempts_count + 1, duration, response.status, bodyText);
        } else {
          cb.recordFailure();
          const nextDelay = this.calculateDecorrelatedJitter((event.attempts_count + 1) * 1000);
          markEventFailed(this.db, event.id, endpoint.id, event.attempts_count + 1, duration, `HTTP ${response.status}`, nextDelay, endpoint.max_retries);
        }
      } catch (err: any) {
        const duration = Date.now() - startTime;
        cb.recordFailure();
        const nextDelay = this.calculateDecorrelatedJitter((event.attempts_count + 1) * 1000);
        markEventFailed(this.db, event.id, endpoint.id, event.attempts_count + 1, duration, err.message || 'Fetch error', nextDelay, endpoint.max_retries);
      }
    }

    return events.length;
  }

  public start(pollMs: number = 1000): void {
    if (this.isRunning) return;
    this.isRunning = true;

    const poll = async () => {
      if (!this.isRunning) return;
      try {
        await this.tick();
      } catch (err) {
        // Suppress tick error
      }
      this.timer = setTimeout(poll, pollMs);
    };

    poll();
  }

  public stop(): void {
    this.isRunning = false;
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }
}
