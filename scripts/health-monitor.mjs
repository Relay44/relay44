#!/usr/bin/env node

const API_URL = process.env.API_URL || 'https://relay44-api.onrender.com/v1';
const NEYNAR_API_KEY = process.env.NEYNAR_API_KEY || '';
const NEYNAR_SIGNER_UUID = process.env.NEYNAR_SIGNER_UUID || '';
const ALERT_WEBHOOK_URL = process.env.ALERT_WEBHOOK_URL || '';

const healthUrl = API_URL.replace(/\/v1$/, '/health/detailed');

async function sendAlert(message) {
  // Farcaster DM via Neynar (posts as the bot)
  if (NEYNAR_API_KEY && NEYNAR_SIGNER_UUID) {
    try {
      await fetch('https://api.neynar.com/v2/farcaster/cast', {
        method: 'POST',
        headers: {
          'content-type': 'application/json',
          'x-api-key': NEYNAR_API_KEY,
        },
        body: JSON.stringify({
          signer_uuid: NEYNAR_SIGNER_UUID,
          text: message,
        }),
        signal: AbortSignal.timeout(10_000),
      });
    } catch (err) {
      console.error('Failed to send Farcaster alert:', err.message);
    }
  }

  // Generic webhook (Slack, Discord, etc.)
  if (ALERT_WEBHOOK_URL) {
    try {
      await fetch(ALERT_WEBHOOK_URL, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ text: message, content: message }),
        signal: AbortSignal.timeout(10_000),
      });
    } catch (err) {
      console.error('Failed to send webhook alert:', err.message);
    }
  }
}

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

if (failures.length > 0) {
  const msg = `[relay44 ALERT] Health check failed:\n${failures.join('\n')}`;
  console.error(msg);
  await sendAlert(msg);
  process.exit(1);
}

console.log('All checks passed.');
