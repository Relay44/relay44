#!/usr/bin/env node

import { runPolymarketIndexerTick } from './polymarket-indexer-lib.mjs';

const intervalMs = Number(process.env.POLYMARKET_INDEXER_INTERVAL_MS || 60_000);

async function main() {
  for (;;) {
    const result = await runPolymarketIndexerTick(process.env);
    console.log(JSON.stringify({ at: new Date().toISOString(), ...result }, null, 2));

    if (result?.skipped) {
      return;
    }

    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }
}

main().catch((error) => {
  console.error(
    JSON.stringify(
      {
        ok: false,
        message: error.message,
        status: error.status || null,
        details: error.payload || null,
      },
      null,
      2,
    ),
  );
  process.exit(1);
});
