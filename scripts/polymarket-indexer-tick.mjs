#!/usr/bin/env node

import { withErrorHandler } from './runner-framework.mjs';
import { runPolymarketIndexerTick } from './polymarket-indexer-lib.mjs';

withErrorHandler(async () => {
  const result = await runPolymarketIndexerTick(process.env);
  console.log(JSON.stringify(result, null, 2));
});
