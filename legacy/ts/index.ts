import { getDatabase } from './db/connection.js';
import { runMigrations } from './db/migrations.js';
import { buildServer } from './server.js';
import { DispatchWorker } from './worker/dispatcher.js';

async function main() {
  const db = getDatabase();
  runMigrations(db);

  const server = buildServer(db);
  const worker = new DispatchWorker(db);

  const port = parseInt(process.env.PORT || '8080', 10);
  const host = process.env.HOST || '0.0.0.0';

  await server.listen({ port, host });
  worker.start(1000);

  console.log(`[AetherRelay] Gateway running on http://${host}:${port}`);

  // Graceful shutdown
  const shutdown = async () => {
    console.log('[AetherRelay] Draining and shutting down gracefully...');
    worker.stop();
    await server.close();
    db.close();
    process.exit(0);
  };

  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);
}

if (process.env.NODE_ENV !== 'test') {
  main().catch((err) => {
    console.error('[AetherRelay] Fatal startup error:', err);
    process.exit(1);
  });
}
