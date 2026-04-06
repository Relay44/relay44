import 'dotenv/config';
import { join } from 'path';
import { loadConfig, validateConfig } from './config.js';
import { createDKGClient } from './dkg/client.js';
import { initDatabase } from './db/schema.js';
import { createServer } from './server.js';

process.on('unhandledRejection', (reason) => {
  console.error('[context-graph] Unhandled rejection:', reason);
});

process.on('uncaughtException', (err) => {
  console.error('[context-graph] Uncaught exception:', err);
  process.exit(1);
});

async function main() {
  const config = loadConfig();
  validateConfig(config);

  console.log('[context-graph] Starting Relay44 Context Graph Service');
  console.log(`[context-graph] Port: ${config.port}`);
  console.log(`[context-graph] DKG: ${config.features.dkgEnabled ? config.dkg.endpoint || 'mock' : 'disabled'}`);
  console.log(`[context-graph] LLM: ${config.features.llmEnabled ? config.llm.model : 'disabled'}`);

  const dkg = createDKGClient(
    config.features.dkgEnabled
      ? {
          endpoint: config.dkg.endpoint!,
          port: config.dkg.port,
          blockchain: {
            name: config.dkg.blockchain,
            privateKey: config.dkg.privateKey,
            rpc: config.dkg.rpc,
          },
        }
      : undefined,
    {
      debug: (msg, meta) => console.debug(`[dkg] ${msg}`, meta || ''),
      info: (msg, meta) => console.log(`[dkg] ${msg}`, meta || ''),
      warn: (msg, meta) => console.warn(`[dkg] ${msg}`, meta || ''),
      error: (msg, meta) => console.error(`[dkg] ${msg}`, meta || ''),
    },
  );

  const dbPath = join(config.dataDir, 'context-graph.db');
  const db = initDatabase(dbPath);
  console.log(`[context-graph] Database: ${dbPath}`);

  const app = createServer(dkg, db, config);

  const server = app.listen(config.port, () => {
    console.log(`[context-graph] Relay44 Context Graph Service running on :${config.port}`);
  });

  let shuttingDown = false;
  const shutdown = (signal: string) => {
    if (shuttingDown) return;
    shuttingDown = true;
    console.log(`[context-graph] ${signal} received, shutting down...`);

    server.close(() => {
      console.log('[context-graph] HTTP server closed');
      db.close();
      console.log('[context-graph] Database closed');
      process.exit(0);
    });

    // Force exit if graceful shutdown takes too long
    setTimeout(() => {
      console.error('[context-graph] Forced shutdown after timeout');
      db.close();
      process.exit(1);
    }, 10_000).unref();
  };

  process.on('SIGINT', () => shutdown('SIGINT'));
  process.on('SIGTERM', () => shutdown('SIGTERM'));
}

main().catch((err) => {
  console.error('[context-graph] Fatal error:', err);
  process.exit(1);
});
