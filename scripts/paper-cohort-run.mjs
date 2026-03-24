#!/usr/bin/env node

import { apiPost, loginAdmin } from "./paper-cohort-lib.mjs";

async function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function main() {
  const intervalMs = Number(process.env.PAPER_COHORT_RUN_INTERVAL_MS || 60_000);
  const limit = Number(process.env.PAPER_RUNNER_TICK_LIMIT || 200);
  const { accessToken, account } = await loginAdmin();

  console.log(
    JSON.stringify(
      {
        ok: true,
        mode: "paper",
        runner: account.address,
        intervalMs,
        limit,
        startedAt: new Date().toISOString(),
      },
      null,
      2,
    ),
  );

  while (true) {
    try {
      const payload = await apiPost(
        "/external/agents/runner/tick",
        accessToken,
        { limit },
      );
      console.log(
        JSON.stringify(
          {
            at: new Date().toISOString(),
            ...payload,
          },
          null,
          2,
        ),
      );
    } catch (error) {
      console.error(
        JSON.stringify(
          {
            ok: false,
            at: new Date().toISOString(),
            message: error.message,
            status: error.status || null,
            details: error.payload || null,
          },
          null,
          2,
        ),
      );
    }

    await sleep(intervalMs);
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
