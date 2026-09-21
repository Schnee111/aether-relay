import type { FastifyInstance, FastifyRequest, FastifyReply } from 'fastify';
import type Database from 'better-sqlite3';

export function registerDlqRoutes(fastify: FastifyInstance, db: Database.Database): void {
  // List DLQ events
  fastify.get('/v1/dlq', async (req: FastifyRequest, reply: FastifyReply) => {
    const rows = db.prepare(`
      SELECT dlq.*, e.target_url, e.name as endpoint_name
      FROM dead_letter_queue dlq
      JOIN endpoints e ON dlq.endpoint_id = e.id
      ORDER BY dlq.created_at DESC
      LIMIT 100
    `).all();
    return reply.send({ data: rows });
  });

  // Replay a DLQ event
  fastify.post(
    '/v1/dlq/:id/replay',
    async (
      req: FastifyRequest<{ Params: { id: string }; Body?: { actor?: string } }>,
      reply: FastifyReply
    ) => {
      const dlqId = req.params.id;
      const actor = req.body?.actor || 'api-user';
      const now = Date.now();

      const dlqItem = db.prepare('SELECT * FROM dead_letter_queue WHERE id = ?').get(dlqId) as any;
      if (!dlqItem) {
        return reply.status(404).send({ error: 'DLQ record not found', id: dlqId });
      }

      const updateEvent = db.prepare(`
        UPDATE incoming_events
        SET status = 'RECEIVED',
            attempts_count = 0,
            next_attempt_at = ?,
            locked_until = NULL
        WHERE id = ?
      `);

      const updateDlq = db.prepare(`
        UPDATE dead_letter_queue
        SET replayed_at = ?,
            replayed_by = ?
        WHERE id = ?
      `);

      const tx = db.transaction(() => {
        updateEvent.run(now, dlqItem.event_id);
        updateDlq.run(now, actor, dlqId);
      });

      tx.immediate();

      return reply.status(200).send({
        status: 'REPLAY_QUEUED',
        dlqId,
        eventId: dlqItem.event_id,
        replayedAt: now,
      });
    }
  );
}
