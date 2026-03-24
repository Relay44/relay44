#!/usr/bin/env node

import { apiPost, loginAdmin } from './external-runner-lib.mjs';

function isEnabled(raw, fallback = true) {
  if (raw == null || raw === '') {
    return fallback;
  }

  return ['1', 'true', 'yes', 'on'].includes(String(raw).trim().toLowerCase());
}

async function main() {
  if (!isEnabled(process.env.EXTERNAL_RUNNER_ENABLED, true)) {
    console.log(JSON.stringify({ ok: true, skipped: true, reason: 'external runner disabled' }, null, 2));
    return;
  }

  const { accessToken } = await loginAdmin();
  const limit = Number(process.env.EXTERNAL_RUNNER_LIMIT || 200);
  const payload = await apiPost('/external/agents/runner/tick', accessToken, { limit });
  console.log(JSON.stringify(payload, null, 2));
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
