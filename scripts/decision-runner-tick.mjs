#!/usr/bin/env node

import { apiPost, loginAdmin } from './decision-runner-lib.mjs';

function isEnabled(raw, fallback = true) {
  if (raw == null || raw === '') {
    return fallback;
  }

  return ['1', 'true', 'yes', 'on'].includes(String(raw).trim().toLowerCase());
}

async function main() {
  if (!isEnabled(process.env.DECISION_RUNNER_ENABLED, true)) {
    console.log(JSON.stringify({ ok: true, skipped: true, reason: 'decision runner disabled' }, null, 2));
    return;
  }

  const { accessToken } = await loginAdmin();
  const limit = Number(process.env.DECISION_RUNNER_LIMIT || 100);
  const payload = await apiPost('/decisions/runner/tick', accessToken, { limit });
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
