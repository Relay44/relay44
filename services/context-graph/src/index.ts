import 'dotenv/config';
import { mkdirSync } from 'fs';
import { join } from 'path';
import { loadConfig, validateConfig } from './config.js';
import { createDKGClient } from './dkg/client.js';
import { initDatabase } from './db/schema.js';
import { createServer } from './server.js';
import { TickAnalyzer } from './workers/tick-analyzer.js';

process.on('unhandledRejection', (reason) => {
  console.error('[context-graph] Unhandled rejection:', reason);
});

process.on('uncaughtException', (err) => {
  console.error('[context-graph] Uncaught exception:', err);
  process.exit(1);
});

function resolveDataDir(dataDir: string): string {
  try {
    mkdirSync(dataDir, { recursive: true });
    return dataDir;
  } catch (error) {
    const err = error as NodeJS.ErrnoException;
    if (err.code !== 'EACCES' && err.code !== 'EPERM') {
      throw err;
    }

    const fallback = '/tmp/context-graph';
    console.warn(
      `[context-graph] Data dir ${dataDir} is not writable, falling back to ${fallback}`,
    );
    mkdirSync(fallback, { recursive: true });
    return fallback;
  }
}

async function main() {
  const config = loadConfig();
  validateConfig(config);

  console.log('[context-graph] Starting Relay44 Context Graph Service');
  console.log(`[context-graph] Host: ${config.host}`);
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

  const dataDir = resolveDataDir(config.dataDir);
  const dbPath = join(dataDir, 'context-graph.db');
  const db = initDatabase(dbPath);
  console.log(`[context-graph] Database: ${dbPath}`);

  const tickAnalyzer = new TickAnalyzer(dkg, db, config);
  const app = createServer(dkg, db, config, tickAnalyzer);

  const server = app.listen(config.port, config.host, () => {
    console.log(`[context-graph] Relay44 Context Graph Service running on ${config.host}:${config.port}`);
    tickAnalyzer.start();
  });

  let shuttingDown = false;
  const shutdown = (signal: string) => {
    if (shuttingDown) return;
    shuttingDown = true;
    console.log(`[context-graph] ${signal} received, shutting down...`);
    tickAnalyzer.stop();

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
