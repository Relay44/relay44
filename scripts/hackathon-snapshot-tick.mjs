#!/usr/bin/env node

import { apiBase, buildHeaders, fetchJson, loginAdmin } from './external-runner-lib.mjs';
import { withErrorHandler } from './runner-framework.mjs';

const FETCH_TIMEOUT_MS = 60_000;

async function main() {
  const startMs = Date.now();
  const { accessToken } = await loginAdmin();

  const hackathonsRes = await fetchJson(`${apiBase}/hackathons?status=active`, {
    headers: buildHeaders(accessToken),
    signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
  });

  const hackathons = hackathonsRes?.hackathons || [];

  if (hackathons.length === 0) {
    console.log(JSON.stringify({ ok: true, message: 'no active hackathons' }, null, 2));
    return;
  }

  let failures = 0;

  for (const h of hackathons) {
    console.log(`Snapshotting hackathon ${h.id} (${h.name})...`);
    try {
      const result = await fetchJson(`${apiBase}/hackathons/${h.id}/snapshot`, {
        method: 'POST',
        headers: buildHeaders(accessToken),
        body: JSON.stringify({}),
        signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
      });
      console.log(JSON.stringify({ ok: true, hackathonId: h.id, ...result }, null, 2));
    } catch (err) {
      failures++;
      console.error(
        JSON.stringify(
          { ok: false, hackathonId: h.id, message: err.message, status: err.status || null },
          null,
          2,
        ),
      );
    }
  }

  const durationMs = Date.now() - startMs;
  console.log(JSON.stringify({ durationMs, hackathons: hackathons.length, failures }, null, 2));

  if (failures > 0) {
    console.error(`${failures}/${hackathons.length} snapshot(s) failed`);
    process.exit(1);
  }
}

withErrorHandler(main);
