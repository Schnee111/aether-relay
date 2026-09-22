import type { FastifyInstance, FastifyRequest, FastifyReply } from 'fastify';
import type Database from 'better-sqlite3';
import { verifyWebhookSignature } from '../../crypto/adapters.js';
import { ingestEventAtomically } from '../../core/idempotency.js';
import type { EndpointRow } from '../../db/schema.js';
import { metrics } from '../../observability/metrics.js';

export function registerIngestRoute(fastify: FastifyInstance, db: Database.Database): void {
  const getEndpointStmt = db.prepare('SELECT * FROM endpoints WHERE id = ?');

  fastify.post(
    '/v1/ingest/:endpointId',
    async (
      req: FastifyRequest<{ Params: { endpointId: string }; Body: any }>,
      reply: FastifyReply
    ) => {
      const startTime = Date.now();
      const endpointId = req.params.endpointId;

      const endpoint = getEndpointStmt.get(endpointId) as EndpointRow | undefined;
      if (!endpoint) {
        metrics.ingestedEventsTotal.inc({ endpoint: endpointId, provider: 'unknown', status: 'NOT_FOUND' });
        return reply.status(404).send({ error: 'Endpoint not found', endpointId });
      }

      const rawBody = req.rawBody || Buffer.from(JSON.stringify(req.body || {}));

      // 1. Verify Cryptographic Signature
      const verification = verifyWebhookSignature({
        provider: endpoint.provider_type,
        secret: endpoint.secret_key,
        rawBody,
        headers: req.headers,
        jsonPayload: req.body,
      });

      if (!verification.isValid) {
        metrics.ingestedEventsTotal.inc({ endpoint: endpointId, provider: endpoint.provider_type, status: 'UNAUTHORIZED' });
        return reply.status(401).send({
          error: 'Unauthorized',
          reason: verification.reason || 'Cryptographic signature mismatch',
        });
      }

      // 2. Extract Idempotency Key
      const rawKey = req.headers['idempotency-key'] || req.headers['x-idempotency-key'];
      const idempotencyKey = Array.isArray(rawKey)
        ? rawKey[0]
        : rawKey || `auto-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;

      // 3. Atomic Ingest
      const outcome = ingestEventAtomically(db, {
        endpointId,
        idempotencyKey,
        headers: req.headers,
        payload: req.body,
        rawPayload: rawBody,
      });

      const duration = Date.now() - startTime;
      metrics.ingestDurationMs.observe({ endpoint: endpointId, status: outcome.status }, duration);

      if (outcome.status === 'DUPLICATE_CONFLICT') {
        metrics.ingestedEventsTotal.inc({ endpoint: endpointId, provider: endpoint.provider_type, status: 'CONFLICT' });
        return reply.status(409).send({
          status: 'DUPLICATE_CONFLICT',
          idempotencyKey: outcome.idempotencyKey,
          existingEventId: outcome.existingEventId,
          message: 'Webhook event with identical idempotency key already ingested',
        });
      }

      metrics.ingestedEventsTotal.inc({ endpoint: endpointId, provider: endpoint.provider_type, status: 'ACCEPTED' });
      return reply.status(202).send({
        status: 'ACCEPTED',
        eventId: outcome.eventId,
        idempotencyKey,
      });
    }
  );
}
