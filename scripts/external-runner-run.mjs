#!/usr/bin/env node

import { apiPost, loginAdmin } from './external-runner-lib.mjs';

const intervalMs = Number(process.env.EXTERNAL_RUNNER_INTERVAL_MS || 60_000);
const limit = Number(process.env.EXTERNAL_RUNNER_LIMIT || 200);

async function tick(accessToken) {
  const payload = await apiPost('/external/agents/runner/tick', accessToken, { limit });
  console.log(JSON.stringify({ at: new Date().toISOString(), payload }, null, 2));
}

async function main() {
  const { accessToken } = await loginAdmin();
  for (;;) {
    await tick(accessToken);
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }
}

main().catch((error) => {
  console.error(JSON.stringify({ ok: false, message: error.message, status: error.status || null, details: error.payload || null }, null, 2));
  process.exit(1);
});
