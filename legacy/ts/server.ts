import Fastify, { FastifyInstance } from 'fastify';
import cors from '@fastify/cors';
import type Database from 'better-sqlite3';
import { registerRawBodyParser } from './api/parser.js';
import { registerIngestRoute } from './api/routes/ingest.js';
import { registerDlqRoutes } from './api/routes/dlq.js';
import { registry } from './observability/metrics.js';

export function buildServer(db: Database.Database): FastifyInstance {
  const server = Fastify({
    logger: false,
    bodyLimit: 5 * 1024 * 1024, // 5MB limit
  });

  server.register(cors);
  registerRawBodyParser(server);

  // Health check route
  server.get('/health', async (req, reply) => {
    try {
      const row = db.prepare('SELECT 1 as healthy').get() as { healthy: number };
      return reply.send({ status: 'ok', db: row.healthy === 1, timestamp: Date.now() });
    } catch (err: any) {
      return reply.status(503).send({ status: 'error', error: err.message });
    }
  });

  // Prometheus Metrics route
  server.get('/metrics', async (req, reply) => {
    reply.header('Content-Type', registry.contentType);
    return reply.send(await registry.metrics());
  });

  // Ingest & DLQ routes
  registerIngestRoute(server, db);
  registerDlqRoutes(server, db);

  return server;
}
