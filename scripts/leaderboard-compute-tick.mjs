#!/usr/bin/env node

import { withErrorHandler } from './runner-framework.mjs';
import * as lib from './leaderboard-compute-lib.mjs';

withErrorHandler(async () => {
  const startMs = Date.now();
  const { accessToken } = await lib.loginAdmin();
  const payload = await lib.apiPost('/leaderboard/compute', accessToken);
  const durationMs = Date.now() - startMs;
  console.log(JSON.stringify({ ok: true, ...payload, durationMs }, null, 2));
});
