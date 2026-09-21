import client from 'prom-client';

// Initialize Prometheus registry
export const registry = new client.Registry();
client.collectDefaultMetrics({ register: registry, prefix: 'aether_' });

export const metrics = {
  ingestedEventsTotal: new client.Counter({
    name: 'aether_ingested_events_total',
    help: 'Total incoming webhook events ingested',
    labelNames: ['endpoint', 'provider', 'status'] as const,
    registers: [registry],
  }),

  ingestDurationMs: new client.Histogram({
    name: 'aether_ingest_duration_ms',
    help: 'Latency of webhook ingestion endpoint in ms',
    labelNames: ['endpoint', 'status'] as const,
    buckets: [1, 2, 5, 10, 15, 25, 50, 100, 250],
    registers: [registry],
  }),

  dispatchAttemptsTotal: new client.Counter({
    name: 'aether_dispatch_attempts_total',
    help: 'Total delivery attempts dispatched to downstream services',
    labelNames: ['endpoint', 'status'] as const,
    registers: [registry],
  }),

  circuitBreakerState: new client.Gauge({
    name: 'aether_circuit_breaker_state',
    help: 'Current state of endpoint circuit breaker (0=CLOSED, 1=HALF_OPEN, 2=OPEN)',
    labelNames: ['endpoint'] as const,
    registers: [registry],
  }),

  dlqEventsTotal: new client.Counter({
    name: 'aether_dlq_events_total',
    help: 'Total events evicted to Dead Letter Queue',
    labelNames: ['endpoint'] as const,
    registers: [registry],
  }),
};
