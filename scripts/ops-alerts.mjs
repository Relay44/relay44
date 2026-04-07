#!/usr/bin/env node

export async function sendAlert(message, env = process.env) {
  const neynarApiKey = env.NEYNAR_API_KEY || '';
  const neynarSignerUuid = env.NEYNAR_SIGNER_UUID || '';
  const webhookUrl = env.ALERT_WEBHOOK_URL || '';

  if (webhookUrl) {
    try {
      await fetch(webhookUrl, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ text: message, content: message }),
        signal: AbortSignal.timeout(10_000),
      });
    } catch (error) {
      console.error('Failed to send webhook alert:', error.message);
    }
  }
}
