import { randomUUID } from 'node:crypto';
import { privateKeyToAccount } from 'viem/accounts';

const FETCH_TIMEOUT_MS = 60_000;
const RETRYABLE_STATUSES = new Set([429, 502, 503, 504]);
const GAMMA_API_BASE = (
  process.env.POLYMARKET_GAMMA_API_BASE || 'https://gamma-api.polymarket.com'
).trim().replace(/\/$/, '');

function normalizeApiBase(raw) {
  const trimmed = String(raw || '').trim().replace(/\/$/, '');
  if (!trimmed) return 'http://localhost:8080/v1';
  const withScheme =
    trimmed.startsWith('http://') || trimmed.startsWith('https://')
      ? trimmed
      : `http://${trimmed}`;
  return withScheme.endsWith('/v1') ? withScheme : `${withScheme}/v1`;
}

export const apiBase = normalizeApiBase(
  process.env.EDGE_SCANNER_API_URL || process.env.API_URL,
);
export const apiOrigin = apiBase.replace(/\/v1$/, '');
export const siweDomain = (
  process.env.EDGE_SCANNER_SIWE_DOMAIN || process.env.SIWE_DOMAIN || 'localhost:3000'
).trim();
export const chainId = Number(
  process.env.EDGE_SCANNER_CHAIN_ID || process.env.BASE_CHAIN_ID || 8453,
);

function envOrThrow(key) {
  const value = process.env[key]?.trim();
  if (!value) throw new Error(`${key} is required`);
  return value;
}

function buildHeaders(token) {
  const headers = { 'content-type': 'application/json' };
  if (token) headers.authorization = `Bearer ${token}`;
  return headers;
}

async function fetchJson(url, init = {}) {
  const response = await fetch(url, {
    signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
    ...init,
  });
  const text = await response.text();
  let payload = null;
  if (text) {
    try { payload = JSON.parse(text); } catch { payload = { raw: text }; }
  }
  if (!response.ok) {
    const message =
      payload?.error?.message || payload?.message || payload?.error ||
      `${response.status} ${response.statusText}`;
    const err = new Error(message);
    err.status = response.status;
    err.payload = payload;
    throw err;
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
  const privateKey = envOrThrow('EDGE_SCANNER_ADMIN_PRIVATE_KEY');
  const account = privateKeyToAccount(privateKey);
  const noncePayload = await fetchWithRetry(`${apiBase}/auth/siwe/nonce`);
  const nonce = noncePayload?.nonce;
  if (!nonce) throw new Error('missing SIWE nonce');

  const issuedAt = new Date().toISOString();
  const message = `${siweDomain} wants you to sign in with your Ethereum account:\n${account.address}\n\nSign in to relay44 edge scanner\n\nURI: ${apiOrigin}\nVersion: 1\nChain ID: ${chainId}\nNonce: ${nonce}\nIssued At: ${issuedAt}`;
  const signature = await account.signMessage({ message });
  const tokens = await fetchWithRetry(`${apiBase}/auth/siwe/login`, {
    method: 'POST',
    headers: buildHeaders(),
    body: JSON.stringify({ wallet: account.address, message, signature }),
  });
  if (!tokens?.access_token) throw new Error('missing access token');
  return { account, accessToken: tokens.access_token };
}

export async function apiPost(pathname, token, body = {}) {
  return fetchWithRetry(`${apiBase}${pathname}`, {
    method: 'POST',
    headers: buildHeaders(token),
    body: JSON.stringify(body),
  });
}

export async function apiGet(pathname, token) {
  return fetchWithRetry(`${apiBase}${pathname}`, {
    headers: buildHeaders(token),
  });
}

// ============================================================
// Polymarket gamma API helpers
// ============================================================

async function fetchGammaMarkets(params) {
  const qs = new URLSearchParams(params).toString();
  return fetchWithRetry(`${GAMMA_API_BASE}/markets?${qs}`);
}

export async function fetchResolvedMarkets({ limit = 100, offset = 0 } = {}) {
  return fetchGammaMarkets({
    limit: String(limit),
    offset: String(offset),
    closed: 'true',
    order: 'endDate',
    ascending: 'false',
  });
}

export async function fetchActiveMarkets({ limit = 100, offset = 0 } = {}) {
  return fetchGammaMarkets({
    limit: String(limit),
    offset: String(offset),
    active: 'true',
    closed: 'false',
  });
}

// ============================================================
// Calibration curve
// ============================================================

const BUCKETS = [
  [0, 500],
  [500, 1000],
  [1000, 1500],
  [1500, 2000],
  [2000, 2500],
  [2500, 3000],
  [3000, 3500],
  [3500, 4000],
  [4000, 4500],
  [4500, 5000],
  [5000, 5500],
  [5500, 6000],
  [6000, 6500],
  [6500, 7000],
  [7000, 7500],
  [7500, 8000],
  [8000, 8500],
  [8500, 9000],
  [9000, 9500],
  [9500, 10000],
];

function priceToOutcomeBps(market) {
  const outcomePrices = market.outcomePrices || market.outcome_prices;
  if (!outcomePrices) return null;

  let prices;
  if (typeof outcomePrices === 'string') {
    try { prices = JSON.parse(outcomePrices); } catch { return null; }
  } else {
    prices = outcomePrices;
  }

  if (!Array.isArray(prices) || prices.length < 1) return null;
  const yesPrice = Number(prices[0]);
  if (!Number.isFinite(yesPrice) || yesPrice < 0 || yesPrice > 1) return null;
  return Math.round(yesPrice * 10000);
}

function resolvedYes(market) {
  const outcomes = market.outcomes || market.resolvedOutcome;
  if (typeof outcomes === 'string') {
    return outcomes.toLowerCase().includes('yes');
  }
  const resolved = market.resolvedOutcome ?? market.resolved_outcome;
  if (resolved === 'Yes' || resolved === 'yes') return true;
  if (resolved === 'No' || resolved === 'no') return false;

  const winner = market.winnerOutcome ?? market.winner_outcome;
  if (winner === 'Yes' || winner === 'yes') return true;
  if (winner === 'No' || winner === 'no') return false;

  return null;
}

export function buildCalibrationCurve(resolvedMarkets) {
  const buckets = BUCKETS.map(([low, high]) => ({
    low,
    high,
    total: 0,
    yesCount: 0,
  }));

  for (const market of resolvedMarkets) {
    const priceBps = priceToOutcomeBps(market);
    if (priceBps == null) continue;

    const yes = resolvedYes(market);
    if (yes == null) continue;

    const bucket = buckets.find((b) => priceBps >= b.low && priceBps < b.high);
    if (!bucket) continue;

    bucket.total += 1;
    if (yes) bucket.yesCount += 1;
  }

  return buckets
    .filter((b) => b.total >= 5)
    .map((b) => {
      const actualRateBps = Math.round((b.yesCount / b.total) * 10000);
      const expectedMidpointBps = Math.round((b.low + b.high) / 2);
      return {
        bucketLowBps: b.low,
        bucketHighBps: b.high,
        sampleCount: b.total,
        resolvedYes: b.yesCount,
        actualRateBps,
        expectedMidpointBps,
        edgeBps: expectedMidpointBps - actualRateBps,
      };
    });
}

// ============================================================
// Calibration signal generation
// ============================================================

const MIN_CALIBRATION_EDGE_BPS = Number(process.env.EDGE_SCANNER_MIN_CALIBRATION_EDGE_BPS || 500);
const MIN_BUCKET_SAMPLES = Number(process.env.EDGE_SCANNER_MIN_BUCKET_SAMPLES || 20);

export function generateCalibrationSignals(curve, activeMarkets) {
  const edgeMap = new Map();
  for (const bucket of curve) {
    if (bucket.sampleCount < MIN_BUCKET_SAMPLES) continue;
    if (Math.abs(bucket.edgeBps) < MIN_CALIBRATION_EDGE_BPS) continue;
    edgeMap.set(`${bucket.bucketLowBps}-${bucket.bucketHighBps}`, bucket);
  }

  if (edgeMap.size === 0) return [];

  const signals = [];
  const now = new Date();
  const expiresAt = new Date(now.getTime() + 24 * 60 * 60 * 1000).toISOString();

  for (const market of activeMarkets) {
    const priceBps = priceToOutcomeBps(market);
    if (priceBps == null) continue;

    const bucket = [...edgeMap.values()].find(
      (b) => priceBps >= b.bucketLowBps && priceBps < b.bucketHighBps,
    );
    if (!bucket) continue;

    const edge = bucket.edgeBps;
    const direction = edge > 0 ? 'no' : 'yes';
    const absEdge = Math.abs(edge);
    const fairValue = bucket.actualRateBps / 10000;
    const marketPrice = priceBps / 10000;

    const p = direction === 'yes' ? fairValue : 1 - fairValue;
    const q = 1 - p;
    const odds = direction === 'yes' ? 1 / marketPrice : 1 / (1 - marketPrice);
    const kellyRaw = (p * odds - q) / odds;
    const kelly = Math.max(0, Math.min(kellyRaw * 0.25, 0.05));

    const marketId = market.conditionId || market.condition_id || market.id;
    const question = market.question || market.title || marketId;

    signals.push({
      id: randomUUID(),
      strategy: 'calibration_arb',
      provider: 'polymarket',
      marketId,
      direction,
      edgeBps: absEdge,
      confidenceBps: Math.min(absEdge * 10, 9000),
      kellyFraction: Math.round(kelly * 10000) / 10000,
      marketPrice,
      fairValue,
      deadline: market.endDate || market.end_date || null,
      daysRemaining: null,
      rationale: `calibration bias: bucket ${bucket.bucketLowBps / 100}%-${bucket.bucketHighBps / 100}% resolves YES ${bucket.actualRateBps / 100}% of the time (n=${bucket.sampleCount}), market prices ${question} at ${(marketPrice * 100).toFixed(1)}%`,
      metadata: {
        question,
        bucketLow: bucket.bucketLowBps,
        bucketHigh: bucket.bucketHighBps,
        sampleCount: bucket.sampleCount,
        actualRate: bucket.actualRateBps,
      },
      expiresAt,
    });
  }

  return signals;
}

// ============================================================
// Time decay scoring
// ============================================================

const MIN_DECAY_EDGE_BPS = Number(process.env.EDGE_SCANNER_MIN_DECAY_EDGE_BPS || 400);
const MIN_DAYS_REMAINING = Number(process.env.EDGE_SCANNER_MIN_DAYS_REMAINING || 7);

export function generateTimeDecaySignals(activeMarkets, curve) {
  const now = new Date();
  const signals = [];
  const expiresAt = new Date(now.getTime() + 24 * 60 * 60 * 1000).toISOString();

  const calibrationMap = new Map();
  for (const bucket of curve) {
    if (bucket.sampleCount < 10) continue;
    calibrationMap.set(`${bucket.bucketLowBps}-${bucket.bucketHighBps}`, bucket);
  }

  for (const market of activeMarkets) {
    const endDate = market.endDate || market.end_date;
    if (!endDate) continue;

    const deadline = new Date(endDate);
    const daysRemaining = Math.ceil((deadline - now) / (1000 * 60 * 60 * 24));
    if (daysRemaining < MIN_DAYS_REMAINING) continue;

    const priceBps = priceToOutcomeBps(market);
    if (priceBps == null || priceBps < 1500) continue;

    let baseRateBps = null;
    const bucket = [...calibrationMap.values()].find(
      (b) => priceBps >= b.bucketLowBps && priceBps < b.bucketHighBps,
    );
    if (bucket) {
      baseRateBps = bucket.actualRateBps;
    }

    const fairEstimate = baseRateBps != null ? baseRateBps : priceBps;
    const inflationBps = priceBps - fairEstimate;
    if (inflationBps < MIN_DECAY_EDGE_BPS) continue;

    const dailyDecayBps = Math.round(inflationBps / daysRemaining);
    if (dailyDecayBps < 10) continue;

    const marketPrice = priceBps / 10000;
    const fairValue = fairEstimate / 10000;
    const marketId = market.conditionId || market.condition_id || market.id;
    const question = market.question || market.title || marketId;

    const p = 1 - fairValue;
    const q = fairValue;
    const odds = 1 / (1 - marketPrice);
    const kellyRaw = (p * odds - q) / odds;
    const kelly = Math.max(0, Math.min(kellyRaw * 0.25, 0.05));

    signals.push({
      id: randomUUID(),
      strategy: 'time_decay',
      provider: 'polymarket',
      marketId,
      direction: 'no',
      edgeBps: inflationBps,
      confidenceBps: Math.min(Math.round(inflationBps * 5), 8000),
      kellyFraction: Math.round(kelly * 10000) / 10000,
      marketPrice,
      fairValue,
      deadline: endDate,
      daysRemaining,
      rationale: `time decay: ${question} priced at ${(marketPrice * 100).toFixed(1)}% with ${daysRemaining}d remaining, fair estimate ${(fairValue * 100).toFixed(1)}%, daily decay ~${(dailyDecayBps / 100).toFixed(2)}%`,
      metadata: {
        question,
        dailyDecayBps,
        inflationBps,
        baseRateSource: baseRateBps != null ? 'calibration_curve' : 'market_implied',
      },
      expiresAt,
    });
  }

  return signals.sort((a, b) => b.edgeBps - a.edgeBps);
}

// ============================================================
// Signal persistence via API
// ============================================================

export async function persistSignals(token, signals) {
  if (signals.length === 0) return { persisted: 0 };

  const result = await apiPost('/external/edge-scanner/signals', token, { signals });
  return result;
}

export async function persistCalibrationCurve(token, curve) {
  if (curve.length === 0) return { persisted: 0 };

  const result = await apiPost('/external/edge-scanner/calibration', token, {
    provider: 'polymarket',
    buckets: curve,
  });
  return result;
}
