import type { CircuitState } from '../db/schema.js';

export interface CircuitBreakerConfig {
  failureThreshold?: number; // default 5
  recoveryTimeoutMs?: number; // default 30000 (30s)
}

export class CircuitBreaker {
  public state: CircuitState = 'CLOSED';
  public failures: number = 0;
  public nextAttemptAt: number | null = null;

  private failureThreshold: number;
  private recoveryTimeoutMs: number;

  constructor(config: CircuitBreakerConfig = {}) {
    this.failureThreshold = config.failureThreshold ?? 5;
    this.recoveryTimeoutMs = config.recoveryTimeoutMs ?? 30000;
  }

  public canExecute(): boolean {
    const now = Date.now();

    if (this.state === 'CLOSED') {
      return true;
    }

    if (this.state === 'OPEN') {
      if (this.nextAttemptAt && now >= this.nextAttemptAt) {
        this.state = 'HALF_OPEN';
        return true; // Canary probe allowed
      }
      return false; // Circuit is open
    }

    if (this.state === 'HALF_OPEN') {
      return true;
    }

    return true;
  }

  public recordSuccess(): void {
    this.failures = 0;
    this.state = 'CLOSED';
    this.nextAttemptAt = null;
  }

  public recordFailure(): void {
    this.failures += 1;
    if (this.failures >= this.failureThreshold) {
      this.state = 'OPEN';
      this.nextAttemptAt = Date.now() + this.recoveryTimeoutMs;
    }
  }
}
