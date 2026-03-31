#!/usr/bin/env node

import { apiBase, buildHeaders, fetchJson, loginAdmin } from './external-runner-lib.mjs';

async function main() {
  const { accessToken } = await loginAdmin();

  // Fetch active hackathons
  const hackathonsRes = await fetchJson(`${apiBase}/hackathons?status=active`, {
    headers: buildHeaders(accessToken),
  });

  const hackathons = hackathonsRes?.hackathons || [];

  if (hackathons.length === 0) {
    console.log(JSON.stringify({ ok: true, message: 'no active hackathons' }, null, 2));
    return;
  }

  for (const h of hackathons) {
    console.log(`Snapshotting hackathon ${h.id} (${h.name})...`);
    try {
      const result = await fetchJson(`${apiBase}/hackathons/${h.id}/snapshot`, {
        method: 'POST',
        headers: buildHeaders(accessToken),
        body: JSON.stringify({}),
      });
      console.log(JSON.stringify({ ok: true, hackathonId: h.id, ...result }, null, 2));
    } catch (err) {
      console.error(
        JSON.stringify(
          { ok: false, hackathonId: h.id, message: err.message, status: err.status || null },
          null,
          2,
        ),
      );
    }
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
