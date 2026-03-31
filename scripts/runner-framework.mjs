#!/usr/bin/env node

/**
 * Shared tick runner framework.
 * Provides isEnabled(), runTick(), and withErrorHandler() used by all cron tick scripts.
 */

export function isEnabled(raw, fallback = true) {
  if (raw == null || raw === '') {
    return fallback;
  }
  return ['1', 'true', 'yes', 'on'].includes(String(raw).trim().toLowerCase());
}

export async function runTick({ name, envKey, defaultLimit, endpoint, lib }) {
  const startMs = Date.now();

  if (!isEnabled(process.env[envKey], true)) {
    console.log(
      JSON.stringify({ ok: true, skipped: true, reason: `${name} disabled` }, null, 2),
    );
    return;
  }

  const { accessToken } = await lib.loginAdmin();
  const limitEnvKey = `${envKey}_LIMIT`;
  const limit = Number(process.env[limitEnvKey] || defaultLimit);
  const payload = await lib.apiPost(endpoint, accessToken, { limit });
  const durationMs = Date.now() - startMs;

  console.log(JSON.stringify({ ...payload, durationMs }, null, 2));
}

export function withErrorHandler(fn) {
  fn().catch((error) => {
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
}
