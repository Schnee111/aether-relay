import type { FastifyInstance, FastifyRequest } from 'fastify';

declare module 'fastify' {
  interface FastifyRequest {
    rawBody?: Buffer;
  }
}

/**
 * Configure Fastify to capture raw Buffer bytes for cryptographic signature verification.
 */
export function registerRawBodyParser(fastify: FastifyInstance): void {
  fastify.addContentTypeParser('application/json', { parseAs: 'buffer' }, (req, body, done) => {
    try {
      const rawBuffer = body as Buffer;
      req.rawBody = rawBuffer;

      if (rawBuffer.length === 0) {
        done(null, {});
        return;
      }

      const parsed = JSON.parse(rawBuffer.toString('utf-8'));
      done(null, parsed);
    } catch (err) {
      done(err as Error, undefined);
    }
  });
}
