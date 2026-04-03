#!/usr/bin/env node

import { randomUUID } from 'node:crypto';
import { pathToFileURL } from 'node:url';

import { privateKeyToAccount } from 'viem/accounts';

import { sendAlert } from './ops-alerts.mjs';
import { isEnabled } from './runner-framework.mjs';
import {
  closeOpsState,
  connectOpsState,
  reportRunnerFailure,
  reportRunnerStarted,
  reportRunnerSuccess,
} from './ops-state.mjs';

const DEFAULT_API_URL = 'https://relay44-api.onrender.com/v1';
const DEFAULT_INTERVAL_HOURS = 1;
const DEFAULT_WINDOW_MINUTES = 5;
const DEFAULT_OVERDUE_GRACE_MINUTES = 10;
const DEFAULT_MARKET_LIMIT = 200;
const DEFAULT_QUANTITY = 1;
const DEFAULT_PRICE_BPS = 1;
const RUNNER_NAME = 'order_smoke';

class OrderSmokeError extends Error {
  constructor(code, message, details = {}) {
    super(message);
    this.name = 'OrderSmokeError';
    this.code = code;
    Object.assign(this, details);
  }
}

function normalizeApiBase(raw) {
  const trimmed = String(raw || '').trim().replace(/\/$/, '');
  if (!trimmed) {
    return DEFAULT_API_URL;
  }

  const withScheme =
    trimmed.startsWith('http://') || trimmed.startsWith('https://')
      ? trimmed
      : `http://${trimmed}`;

  return withScheme.endsWith('/v1') ? withScheme : `${withScheme}/v1`;
}

function buildHeaders(token) {
  const headers = {
    'content-type': 'application/json',
    accept: 'application/json',
  };

  if (token) {
    headers.authorization = `Bearer ${token}`;
  }

  return headers;
}

function parseScheduleConfig(env = process.env) {
  const minute = Number(env.ORDER_SMOKE_MINUTE ?? '0');
  const intervalHours = Number(env.ORDER_SMOKE_INTERVAL_HOURS ?? String(DEFAULT_INTERVAL_HOURS));
  const windowMinutes = Number(env.ORDER_SMOKE_WINDOW_MINUTES ?? String(DEFAULT_WINDOW_MINUTES));
  const overdueGraceMinutes = Number(
    env.ORDER_SMOKE_OVERDUE_GRACE_MINUTES ?? String(DEFAULT_OVERDUE_GRACE_MINUTES),
  );

  if (!Number.isInteger(minute) || minute < 0 || minute > 59) {
    throw new Error(`invalid ORDER_SMOKE_MINUTE: ${env.ORDER_SMOKE_MINUTE}`);
  }
  if (!Number.isInteger(intervalHours) || intervalHours < 1 || intervalHours > 24) {
    throw new Error(`invalid ORDER_SMOKE_INTERVAL_HOURS: ${env.ORDER_SMOKE_INTERVAL_HOURS}`);
  }
  if (!Number.isInteger(windowMinutes) || windowMinutes < 1 || windowMinutes > 60) {
    throw new Error(`invalid ORDER_SMOKE_WINDOW_MINUTES: ${env.ORDER_SMOKE_WINDOW_MINUTES}`);
  }
  if (
    !Number.isInteger(overdueGraceMinutes) ||
    overdueGraceMinutes < 0 ||
    overdueGraceMinutes > 120
  ) {
    throw new Error(
      `invalid ORDER_SMOKE_OVERDUE_GRACE_MINUTES: ${env.ORDER_SMOKE_OVERDUE_GRACE_MINUTES}`,
    );
  }

  return {
    minute,
    intervalHours,
    windowMinutes,
    overdueGraceMinutes,
  };
}

function parsePositiveInt(value, fallback) {
  const parsed = Number.parseInt(String(value || '').trim(), 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function parseOrderPriceBps(raw, fallback) {
  const parsed = Number(raw ?? fallback ?? DEFAULT_PRICE_BPS);
  if (!Number.isFinite(parsed) || parsed < 1 || parsed >= 10_000) {
    throw new Error(`invalid ORDER_SMOKE_PRICE_BPS: ${raw}`);
  }
  return Math.trunc(parsed);
}

function parseOrderQuantity(raw) {
  const parsed = Number(raw ?? DEFAULT_QUANTITY);
  if (!Number.isInteger(parsed) || parsed < 1) {
    throw new Error(`invalid ORDER_SMOKE_QUANTITY: ${raw}`);
  }
  return parsed;
}

function parseJson(text) {
  if (!text) {
    return null;
  }

  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

async function fetchJson(url, init = {}) {
  const response = await fetch(url, {
    signal: AbortSignal.timeout(30_000),
    ...init,
  });
  const text = await response.text();
  const payload = parseJson(text);

  if (!response.ok) {
    const message =
      payload?.error?.message ||
      payload?.message ||
      payload?.error ||
      `${response.status} ${response.statusText}`;
    const error = new Error(message);
    error.status = response.status;
    error.payload = payload ?? { raw: text };
    throw error;
  }

  return payload;
}

async function loginSmokeUser(apiBase, account, domain, chainId) {
  const noncePayload = await fetchJson(`${apiBase}/auth/siwe/nonce`);
  const nonce = noncePayload?.nonce;
  if (!nonce) {
    throw new Error('missing SIWE nonce');
  }

  const issuedAt = new Date().toISOString();
  const message = `${domain} wants you to sign in with your Ethereum account:\n${account.address}\n\nSign in to relay44 order smoke\n\nURI: ${apiBase.replace(/\/v1$/, '')}\nVersion: 1\nChain ID: ${chainId}\nNonce: ${nonce}\nIssued At: ${issuedAt}`;
  const signature = await account.signMessage({ message });
  const tokens = await fetchJson(`${apiBase}/auth/siwe/login`, {
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

  return tokens.access_token;
}

async function apiRequest(apiBase, pathname, token, init = {}) {
  return fetchJson(`${apiBase}${pathname}`, {
    ...init,
    headers: {
      ...buildHeaders(token),
      ...(init.headers || {}),
    },
  });
}

function targetMarketId(env) {
  const raw = String(env.ORDER_SMOKE_MARKET_ID || '').trim();
  if (!raw) {
    return null;
  }

  if (!/^\d+$/.test(raw)) {
    throw new Error(`invalid ORDER_SMOKE_MARKET_ID: ${raw}`);
  }

  return raw;
}

function selectPassiveOrder(market) {
  const yesPrice = Number(market?.yes_price);
  if (Number.isFinite(yesPrice) && yesPrice >= 0.5) {
    return {
      side: 'sell',
      outcome: 'yes',
    };
  }

  return {
    side: 'buy',
    outcome: 'yes',
  };
}

function runnerOverdueMessage(label, state, overdueMs) {
  const lastSucceededAt = state?.last_succeeded_at
    ? new Date(state.last_succeeded_at)
    : null;
  if (!lastSucceededAt) {
    return `${label}_missing: runner has never reported success`;
  }
  if (Date.now() - lastSucceededAt.getTime() <= overdueMs) {
    return null;
  }

  const errorCode = state?.last_error_code ? ` after ${state.last_error_code}` : '';
  return `${label}_overdue: last success ${lastSucceededAt.toISOString()}${errorCode}`;
}

function shouldRunScheduledSmoke(now, env = process.env) {
  if (!isEnabled(env.ORDER_SMOKE_ENABLED, false)) {
    return false;
  }

  const { minute, intervalHours, windowMinutes } = parseScheduleConfig(env);
  const currentMinute = now.getUTCMinutes();
  const inWindow =
    currentMinute >= minute && currentMinute < Math.min(minute + windowMinutes, 60);
  return inWindow && now.getUTCHours() % intervalHours === 0;
}

function scheduledSmokeOverdueMs(env = process.env) {
  const { intervalHours, windowMinutes, overdueGraceMinutes } = parseScheduleConfig(env);
  return (intervalHours * 60 + windowMinutes + overdueGraceMinutes) * 60_000;
}

export { shouldRunScheduledSmoke, scheduledSmokeOverdueMs };

async function resolveMarket(apiBase, token, env) {
  const forcedId = targetMarketId(env);
  const limit = parsePositiveInt(env.ORDER_SMOKE_MARKET_LIMIT, DEFAULT_MARKET_LIMIT);
  const source = String(env.ORDER_SMOKE_MARKET_SOURCE || 'internal').trim() || 'internal';
  const tradable = String(env.ORDER_SMOKE_TRADEABLE || 'user').trim() || 'user';
  const includeLowLiquidity = String(env.ORDER_SMOKE_INCLUDE_LOW_LIQUIDITY || 'true')
    .trim()
    .toLowerCase();
  const includeFlag = ['1', 'true', 'yes', 'on'].includes(includeLowLiquidity);

  const params = new URLSearchParams({
    source,
    tradable,
    limit: String(limit),
  });
  if (includeFlag) {
    params.set('includeLowLiquidity', 'true');
  }

  let payload = await apiRequest(apiBase, `/evm/markets?${params.toString()}`, token);
  let markets = Array.isArray(payload?.markets) ? payload.markets : [];
  let candidates = markets.filter(
    (market) => !market?.resolved && String(market?.status || '').toLowerCase() !== 'closed',
  );

  if (candidates.length === 0 && tradable) {
    params.delete('tradable');
    payload = await apiRequest(apiBase, `/evm/markets?${params.toString()}`, token);
    markets = Array.isArray(payload?.markets) ? payload.markets : [];
    candidates = markets.filter(
      (market) => !market?.resolved && String(market?.status || '').toLowerCase() !== 'closed',
    );
  }

  if (forcedId) {
    const market = candidates.find((entry) => String(entry.id) === forcedId);
    if (!market) {
      throw new OrderSmokeError('market_not_found', `market ${forcedId} is not available for smoke`, {
        targetMarketId: forcedId,
      });
    }
    return market;
  }

  const market = candidates[0];
  if (!market) {
    throw new OrderSmokeError('no_market_available', 'no eligible internal market was found for the order smoke');
  }

  return market;
}

async function placeOrder(apiBase, token, marketId, market, env) {
  const selected = selectPassiveOrder(market);
  const quantity = parseOrderQuantity(env.ORDER_SMOKE_QUANTITY);
  const priceBps = parseOrderPriceBps(
    env.ORDER_SMOKE_PRICE_BPS,
    selected.side === 'buy' ? 1 : 9_999,
  );
  const price = priceBps / 10_000;
  const expiresInSeconds = parsePositiveInt(env.ORDER_SMOKE_EXPIRES_IN_SECONDS, 15 * 60);
  const expiresAt = new Date(Date.now() + expiresInSeconds * 1_000).toISOString();
  const idempotencyKey = env.ORDER_SMOKE_IDEMPOTENCY_KEY || randomUUID();

  const body = {
    market_id: String(marketId),
    side: selected.side,
    outcome: selected.outcome,
    order_type: 'limit',
    price,
    quantity,
    expires_at: expiresAt,
    private: false,
  };

  const response = await apiRequest(apiBase, '/orders', token, {
    method: 'POST',
    headers: {
      'idempotency-key': idempotencyKey,
    },
    body: JSON.stringify(body),
  });

  const status = String(response?.status || '').toLowerCase();
  if (status !== 'open') {
    throw new OrderSmokeError(
      'place_not_open',
      `placed order was not left open: ${status || 'unknown'}`,
      {
        targetMarketId: String(marketId),
        orderId: response?.order_id || null,
        response,
        price,
        quantity,
      },
    );
  }

  return {
    selected,
    price,
    quantity,
    expiresAt,
    idempotencyKey,
    response,
  };
}

async function cancelOrder(apiBase, token, orderId) {
  const response = await apiRequest(apiBase, `/orders/${encodeURIComponent(orderId)}`, token, {
    method: 'DELETE',
  });

  const status = String(response?.status || '').toLowerCase();
  if (status !== 'cancelled') {
    throw new OrderSmokeError('cancel_failed', `cancelled order returned ${status || 'unknown'}`, {
      orderId,
      response,
    });
  }

  return response;
}

async function fetchOrder(apiBase, token, orderId) {
  return apiRequest(apiBase, `/orders/${encodeURIComponent(orderId)}`, token);
}

async function runSmoke(apiBase, token, env) {
  const market = await resolveMarket(apiBase, token, env);
  const marketId = String(market.id);
  const place = await placeOrder(apiBase, token, marketId, market, env);
  const orderId = String(place.response?.order_id || '');
  if (!orderId) {
    throw new OrderSmokeError('place_missing_order_id', 'place order response was missing order_id', {
      targetMarketId: marketId,
      response: place.response,
    });
  }

  let cancelled = null;
  let order = null;
  try {
    cancelled = await cancelOrder(apiBase, token, orderId);
    order = await fetchOrder(apiBase, token, orderId);
    const orderStatus = String(order?.status || '').toLowerCase();
    if (orderStatus !== 'cancelled') {
      throw new OrderSmokeError('verify_failed', `order ${orderId} did not persist as cancelled`, {
        targetMarketId: marketId,
        orderId,
        order,
      });
    }
  } catch (error) {
    if (orderId) {
      try {
        await cancelOrder(apiBase, token, orderId);
      } catch {
        // Best effort cleanup only.
      }
    }
    throw error;
  }

  return {
    ok: true,
    runner: RUNNER_NAME,
    targetMarketId: marketId,
    targetMarketStatus: market.status,
    targetMarketSource: market.source,
    liquidityMode: market.liquidity_mode || null,
    bootstrapStatus: market.bootstrap_status || null,
    orderId,
    selectedSide: place.selected.side,
    selectedOutcome: place.selected.outcome,
    price: place.price,
    quantity: place.quantity,
    placeResponse: place.response,
    cancelResponse: cancelled,
    verifiedOrderStatus: orderStatus,
    verifiedOrder: order,
  };
}

export async function runOrderSmoke(env = process.env) {
  if (!isEnabled(env.ORDER_SMOKE_ENABLED, false)) {
    return { ok: true, skipped: true, reason: 'order smoke disabled' };
  }

  const privateKey = String(env.ORDER_SMOKE_PRIVATE_KEY || '').trim();
  if (!privateKey) {
    throw new Error('ORDER_SMOKE_PRIVATE_KEY is required');
  }

  const apiBase = normalizeApiBase(env.ORDER_SMOKE_API_URL || env.API_URL || DEFAULT_API_URL);
  const siweDomain = (env.ORDER_SMOKE_SIWE_DOMAIN || env.SIWE_DOMAIN || 'localhost:3000').trim();
  const chainId = parsePositiveInt(env.ORDER_SMOKE_CHAIN_ID || env.BASE_CHAIN_ID, 8453);
  const account = privateKeyToAccount(privateKey);
  const accessToken = await loginSmokeUser(apiBase, account, siweDomain, chainId);
  const result = await runSmoke(apiBase, accessToken, env);
  return {
    ...result,
    payer: account.address,
    apiBase,
  };
}

function buildSuccessMetadata(result) {
  return {
    runner: result.runner,
    payer: result.payer,
    apiBase: result.apiBase,
    targetMarketId: result.targetMarketId,
    targetMarketStatus: result.targetMarketStatus,
    targetMarketSource: result.targetMarketSource,
    liquidityMode: result.liquidityMode,
    bootstrapStatus: result.bootstrapStatus,
    orderId: result.orderId,
    selectedSide: result.selectedSide,
    selectedOutcome: result.selectedOutcome,
    price: result.price,
    quantity: result.quantity,
    verifiedOrderStatus: result.verifiedOrderStatus,
  };
}

function buildFailureMetadata(error) {
  return {
    code: error.code || 'smoke_failed',
    phase: error.phase || null,
    targetMarketId: error.targetMarketId || null,
    orderId: error.orderId || null,
    status: error.status || null,
    response: error.response || null,
    order: error.order || null,
  };
}

async function main() {
  const opsState = await connectOpsState(process.env).catch(() => null);

  try {
    const startedAt = new Date().toISOString();
    await reportRunnerStarted(opsState, RUNNER_NAME, { startedAt });
    const result = await runOrderSmoke(process.env);
    await reportRunnerSuccess(opsState, RUNNER_NAME, buildSuccessMetadata(result));
    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    const msg = `[relay44 ALERT] order smoke failed: ${error.message}`;
    console.error(
      JSON.stringify(
        {
          ok: false,
          code: error.code || 'smoke_failed',
          message: msg,
          details: error,
        },
        null,
        2,
      ),
    );
    await reportRunnerFailure(
      opsState,
      RUNNER_NAME,
      error.code || 'smoke_failed',
      error.message,
      buildFailureMetadata(error),
    );
    await sendAlert(msg, process.env);
    await closeOpsState(opsState);
    process.exit(1);
  }

  await closeOpsState(opsState);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
