#!/usr/bin/env node

const API_URL = process.env.API_URL || 'https://relay44-api.onrender.com/v1';
const healthUrl = API_URL.replace(/\/v1$/, '/health/detailed');

const response = await fetch(healthUrl, { signal: AbortSignal.timeout(30_000) });
const data = await response.json();

console.log(JSON.stringify(data, null, 2));

if (data.status !== 'healthy') {
  console.error('API is unhealthy!');
  process.exit(1);
}

const checks = data.checks || {};
for (const [name, check] of Object.entries(checks)) {
  if (check.status !== 'healthy' && check.message !== 'Solana integration disabled') {
    console.error(`Check ${name} is ${check.status}: ${check.message}`);
    process.exit(1);
  }
}

console.log('All checks passed.');
