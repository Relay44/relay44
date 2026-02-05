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

export const apiBase = normalizeApiBase(process.env.DECISION_RUNNER_API_URL);
export const apiOrigin = apiBase.replace(/\/v1$/, '');
export const siweDomain = (
  process.env.DECISION_RUNNER_SIWE_DOMAIN || process.env.SIWE_DOMAIN || 'localhost:3000'
).trim();
export const chainId = Number(process.env.DECISION_RUNNER_CHAIN_ID || process.env.BASE_CHAIN_ID || 8453);

function envOrThrow(key) {
  const value = process.env[key]?.trim();
  if (!value) {
    throw new Error(`${key} is required`);
  }
  return value;
}

function buildHeaders(token) {
  const headers = {
    'content-type': 'application/json',
  };

  if (token) {
    headers.authorization = `Bearer ${token}`;
  }

  return headers;
}

async function fetchJson(url, init = {}) {
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
    const error = new Error(message);
    error.status = response.status;
    error.payload = payload;
    throw error;
  }

  return payload;
}

export async function loginAdmin() {
  const privateKey = envOrThrow('DECISION_RUNNER_ADMIN_PRIVATE_KEY');
  const account = privateKeyToAccount(privateKey);
  const noncePayload = await fetchJson(`${apiBase}/auth/siwe/nonce`);
  const nonce = noncePayload?.nonce;

  if (!nonce) {
    throw new Error('missing SIWE nonce');
  }
