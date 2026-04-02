#!/usr/bin/env node

import { sendAlert } from './ops-alerts.mjs';
import { runX402Smoke, shouldRunScheduledSmoke } from './x402-smoke.mjs';

const API_URL = process.env.API_URL || 'https://relay44-api.onrender.com/v1';

const healthUrl = API_URL.replace(/\/v1$/, '/health/detailed');

let data;
try {
  const response = await fetch(healthUrl, { signal: AbortSignal.timeout(30_000) });
  data = await response.json();
} catch (err) {
  const msg = `[relay44 ALERT] API unreachable: ${err.message}`;
  console.error(msg);
  await sendAlert(msg);
  process.exit(1);
}

console.log(JSON.stringify(data, null, 2));

const failures = [];

if (data.status !== 'healthy') {
  failures.push(`status: ${data.status}`);
}

const checks = data.checks || {};
for (const [name, check] of Object.entries(checks)) {
  if (check.status !== 'healthy' && check.message !== 'Solana integration disabled') {
    failures.push(`${name}: ${check.status} (${check.message || 'no details'})`);
  }
}

let x402Result = null;
if (shouldRunScheduledSmoke(new Date(), process.env)) {
  try {
    x402Result = await runX402Smoke(process.env);
  } catch (err) {
    failures.push(`x402_smoke: failed (${err.message})`);
  }
}

if (failures.length > 0) {
  const msg = `[relay44 ALERT] Health check failed:\n${failures.join('\n')}`;
  console.error(msg);
  await sendAlert(msg, process.env);
  process.exit(1);
}

if (x402Result) {
  console.log(JSON.stringify({ x402Smoke: x402Result }, null, 2));
}

console.log('All checks passed.');
