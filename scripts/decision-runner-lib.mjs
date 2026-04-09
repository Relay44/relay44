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

  const internalKey = (process.env.INTERNAL_SERVICE_KEY || '').trim();
  if (internalKey) {
    headers['x-internal-service-key'] = internalKey;
  }

  return headers;
}

const FETCH_TIMEOUT_MS = 60_000;
const RETRYABLE_STATUSES = new Set([429, 502, 503, 504]);

async function fetchJson(url, init = {}) {
  const response = await fetch(url, {
    signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
    ...init,
  });
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

async function fetchWithRetry(url, init = {}, { maxRetries = 3, baseDelay = 1000 } = {}) {
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      return await fetchJson(url, init);
    } catch (err) {
      const isTimeout = err.name === 'TimeoutError' || err.name === 'AbortError';
      const retryable = isTimeout || RETRYABLE_STATUSES.has(err.status);
      if (!retryable || attempt === maxRetries) throw err;
      const delay = baseDelay * Math.pow(2, attempt);
      await new Promise((r) => setTimeout(r, delay));
    }
  }
}

export async function loginAdmin() {
  const privateKey = envOrThrow('DECISION_RUNNER_ADMIN_PRIVATE_KEY');
  const account = privateKeyToAccount(privateKey);
  const noncePayload = await fetchWithRetry(`${apiBase}/auth/siwe/nonce`);
  const nonce = noncePayload?.nonce;

  if (!nonce) {
    throw new Error('missing SIWE nonce');
  }

  const issuedAt = new Date().toISOString();
  const message = `${siweDomain} wants you to sign in with your Ethereum account:\n${account.address}\n\nSign in to relay44 decision runner\n\nURI: ${apiOrigin}\nVersion: 1\nChain ID: ${chainId}\nNonce: ${nonce}\nIssued At: ${issuedAt}`;
  const signature = await account.signMessage({ message });
  const tokens = await fetchWithRetry(`${apiBase}/auth/siwe/login`, {
    method: 'POST',
    headers: buildHeaders(),
    body: JSON.stringify({
      wallet: account.address,
      message,
      signature,
    }),
  });

  if (!tokens?.access_token) {
    throw new Error('missing access token');
  }

  return {
    account,
    accessToken: tokens.access_token,
  };
}

export async function apiPost(pathname, token, body = {}) {
  return fetchWithRetry(`${apiBase}${pathname}`, {
    method: 'POST',
    headers: buildHeaders(token),
    body: JSON.stringify(body),
  });
}
