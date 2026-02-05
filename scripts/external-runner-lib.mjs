import { privateKeyToAccount } from 'viem/accounts';

function normalizeApiBase(raw) {
  const trimmed = String(raw || '').trim().replace(/\/$/, '');
  if (!trimmed) {
    return 'http://localhost:8080/v1';
  }

  const withScheme =
    trimmed.startsWith('http://') || trimmed.startsWith('https://')
      ? trimmed
      : `http://${trimmed}`;

  return withScheme.endsWith('/v1') ? withScheme : `${withScheme}/v1`;
}

export const apiBase = normalizeApiBase(process.env.EXTERNAL_RUNNER_API_URL);
export const apiOrigin = apiBase.replace(/\/v1$/, '');
export const siweDomain = (
  process.env.EXTERNAL_RUNNER_SIWE_DOMAIN || process.env.SIWE_DOMAIN || 'localhost:3000'
).trim();
export const chainId = Number(process.env.EXTERNAL_RUNNER_CHAIN_ID || process.env.BASE_CHAIN_ID || 8453);
export const runnerCountryCode = String(process.env.EXTERNAL_RUNNER_COUNTRY_CODE || '')
  .trim()
  .toUpperCase();

export function envOrThrow(key) {
  const value = process.env[key]?.trim();
  if (!value) {
    throw new Error(`${key} is required`);
  }
  return value;
}

export function buildHeaders(token) {
  const headers = {
    'content-type': 'application/json',
  };

  if (token) {
    headers.authorization = `Bearer ${token}`;
  }

  if (runnerCountryCode) {
    headers['x-country-code'] = runnerCountryCode;
  }

  return headers;
}

export async function fetchJson(url, init = {}) {
  const response = await fetch(url, init);
  const text = await response.text();
  let payload = null;

  if (text) {
    try {
      payload = JSON.parse(text);
    } catch {
      payload = { raw: text };
    }
  }

  if (!response.ok) {
    const message =
      payload?.error?.message ||
      payload?.message ||
      payload?.error ||
      `${response.status} ${response.statusText}`;
    const err = new Error(message);
    err.status = response.status;
    err.payload = payload;
    throw err;
  }

  return payload;
}

export async function loginAdmin() {
  const privateKey = envOrThrow('EXTERNAL_RUNNER_ADMIN_PRIVATE_KEY');
  const account = privateKeyToAccount(privateKey);
  const noncePayload = await fetchJson(`${apiBase}/auth/siwe/nonce`);
  const nonce = noncePayload?.nonce;

