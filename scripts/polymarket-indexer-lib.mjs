import { createHmac } from 'node:crypto';

import { privateKeyToAccount } from 'viem/accounts';

import {
  closeOpsState,
  connectOpsState,
  reportRunnerFailure,
  reportRunnerStarted,
  reportRunnerSuccess,
} from './ops-state.mjs';

const DEFAULT_API_URL = 'http://localhost:8080/v1';
const DEFAULT_CHAIN_ID = 8453;
const DEFAULT_LIMIT = 25;
const DEFAULT_USER_STREAM_URL = 'wss://ws-subscriptions-clob.polymarket.com/ws/user';
const DEFAULT_RELAYER_URL = 'https://relayer-v2.polymarket.com/transactions';
const DEFAULT_USER_STREAM_WINDOW_MS = 4_000;
const DEFAULT_USER_STREAM_MAX_EVENTS = 500;
const RUNNER_NAME = 'polymarket_indexer';

function isEnabled(raw, fallback = false) {
  if (raw == null || raw === '') {
    return fallback;
  }
  return ['1', 'true', 'yes', 'on'].includes(String(raw).trim().toLowerCase());
}

function envOrThrow(key) {
  const value = String(process.env[key] || '').trim();
  if (!value) {
    throw new Error(`${key} is required`);
  }
  return value;
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

function parsePositiveInt(raw, fallback) {
  const parsed = Number.parseInt(String(raw || '').trim(), 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function buildHeaders({ accessToken, adminKey, hasBody = false } = {}) {
  const headers = {
    accept: 'application/json',
  };

  if (hasBody) {
    headers['content-type'] = 'application/json';
  }
  if (accessToken) {
    headers.authorization = `Bearer ${accessToken}`;
  } else if (adminKey) {
    headers['x-admin-key'] = adminKey;
  }

  return headers;
}

async function fetchJson(url, init = {}) {
  const response = await fetch(url, {
    signal: AbortSignal.timeout(60_000),
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

function normalizeTrackedMarkets(raw) {
  if (!Array.isArray(raw)) {
    return [];
  }
  return raw
    .map((market) => {
      const conditionId = String(
        market?.conditionId ?? market?.condition_id ?? '',
      ).trim();
      if (!conditionId) {
        return null;
      }
      return {
        marketId: String(market?.marketId ?? market?.market_id ?? '').trim(),
        providerMarketRef: String(
          market?.providerMarketRef ?? market?.provider_market_ref ?? '',
        ).trim(),
        conditionId,
      };
    })
    .filter(Boolean);
}

function parseLifecycleCounts(events) {
  const counts = {
    matched: 0,
    mined: 0,
    confirmed: 0,
    retrying: 0,
    failed: 0,
  };

  for (const event of events) {
    const status = String(event?.status || '').trim().toUpperCase();
    if (status === 'MATCHED') counts.matched += 1;
    if (status === 'MINED') counts.mined += 1;
    if (status === 'CONFIRMED') counts.confirmed += 1;
    if (status === 'RETRYING') counts.retrying += 1;
    if (status === 'FAILED') counts.failed += 1;
  }

  return counts;
}

function lifecycleEventKey(event) {
  const id =
    event?.id ||
    event?.trade_id ||
    event?.builderTradeId ||
    event?.builder_trade_id ||
    event?.taker_order_id ||
    event?.takerOrderId ||
    event?.takerOrderHash ||
    event?.transactionHash ||
    'unknown';
  const status = String(event?.status || '').trim().toUpperCase();
  const observedAt =
    event?.last_update ||
    event?.lastUpdate ||
    event?.matchtime ||
    event?.matchTime ||
    event?.updatedAt ||
    event?.transactionHash ||
    '0';
  return `${id}:${status}:${observedAt}`;
}

function extractTradeEvents(payload) {
  if (!payload) {
    return [];
  }
  if (Array.isArray(payload)) {
    return payload.flatMap((entry) => extractTradeEvents(entry));
  }
  if (Array.isArray(payload?.data)) {
    return payload.data.flatMap((entry) => extractTradeEvents(entry));
  }

  const type = String(payload?.type ?? payload?.event_type ?? '').trim().toLowerCase();
  if (type === 'trade') {
    return [payload];
  }

  return [];
}

function makeUserStreamFailure(code, message, extra = {}) {
  return {
    ok: false,
    code,
    status: code === 'user_stream_disconnected' ? 'disconnected' : 'failed',
    events: [],
    error: message,
    ...extra,
  };
}

export const apiBase = normalizeApiBase(
  process.env.POLYMARKET_INDEXER_API_URL || process.env.API_URL || DEFAULT_API_URL,
);
export const apiOrigin = apiBase.replace(/\/v1$/, '');
export const siweDomain = (
  process.env.POLYMARKET_INDEXER_SIWE_DOMAIN ||
  process.env.SIWE_DOMAIN ||
  'localhost:3000'
).trim();
export const chainId = Number(
  process.env.POLYMARKET_INDEXER_CHAIN_ID || process.env.BASE_CHAIN_ID || DEFAULT_CHAIN_ID,
);
export const enabled = isEnabled(process.env.POLYMARKET_INDEXER_ENABLED, false);
export const limit = Math.max(
  parsePositiveInt(process.env.POLYMARKET_INDEXER_LIMIT, DEFAULT_LIMIT),
  1,
);

async function requestApi(path, { method = 'GET', body, accessToken } = {}) {
  const adminKey = String(process.env.ADMIN_CONTROL_KEY || '').trim();
  return fetchJson(`${apiBase}${path}`, {
    method,
    headers: buildHeaders({
      accessToken,
      adminKey: accessToken ? '' : adminKey,
      hasBody: body != null,
    }),
    body: body == null ? undefined : JSON.stringify(body),
  });
}

function builderCredentialsFromEnv() {
  const apiKey = String(process.env.POLYMARKET_BUILDER_API_KEY || '').trim();
  const apiSecret = String(process.env.POLYMARKET_BUILDER_API_SECRET || '').trim();
  const apiPassphrase = String(
    process.env.POLYMARKET_BUILDER_API_PASSPHRASE || '',
  ).trim();

  if (!apiKey || !apiSecret || !apiPassphrase) {
    return null;
  }

  return { apiKey, apiSecret, apiPassphrase };
}

function polymarketBuilderSignature({ apiSecret, method, path, body = '', timestamp }) {
  const key = Buffer.from(apiSecret, 'base64');
  return createHmac('sha256', key)
    .update(`${timestamp}${method.toUpperCase()}${path}${body}`)
    .digest('base64url');
}

function buildPolymarketBuilderHeaders({ method, path, body = '' }) {
  const credentials = builderCredentialsFromEnv();
  if (!credentials) {
    return null;
  }

  const timestamp = String(Math.floor(Date.now() / 1000));
  return {
    'Content-Type': 'application/json',
    POLY_API_KEY: credentials.apiKey,
    POLY_PASSPHRASE: credentials.apiPassphrase,
    POLY_SIGNATURE: polymarketBuilderSignature({
      apiSecret: credentials.apiSecret,
      method,
      path,
      body,
      timestamp,
    }),
    POLY_TIMESTAMP: timestamp,
  };
}

async function fetchRelayerTransactions() {
  const headers = buildPolymarketBuilderHeaders({
    method: 'GET',
    path: '/transactions',
  });
  if (!headers) {
    return {
      ok: false,
      status: 'unauthorized',
      transactions: [],
      error: 'builder credentials are not configured',
    };
  }

  try {
    const transactions = await fetchJson(
      process.env.POLYMARKET_INDEXER_RELAYER_URL || DEFAULT_RELAYER_URL,
      { headers },
    );
    const rows = Array.isArray(transactions) ? transactions : [];
    return {
      ok: true,
      status: 'ready',
      transactions: rows,
      fetched: rows.length,
    };
  } catch (error) {
    return {
      ok: false,
      status: 'failed',
      transactions: [],
      error: error.message,
    };
  }
}

async function collectUserLifecycleEvents(trackedMarkets) {
  const credentials = builderCredentialsFromEnv();
  if (!trackedMarkets.length) {
    return {
      ok: true,
      status: 'idle',
      events: [],
      ...parseLifecycleCounts([]),
      marketCount: 0,
    };
  }
  if (!credentials) {
    return makeUserStreamFailure(
      'user_stream_credentials_failed',
      'builder credentials are not configured',
      { marketCount: trackedMarkets.length },
    );
  }

  const windowMs = parsePositiveInt(
    process.env.POLYMARKET_INDEXER_USER_STREAM_WINDOW_MS,
    DEFAULT_USER_STREAM_WINDOW_MS,
  );
  const maxEvents = parsePositiveInt(
    process.env.POLYMARKET_INDEXER_USER_STREAM_MAX_EVENTS,
    DEFAULT_USER_STREAM_MAX_EVENTS,
  );
  const wsUrl =
    process.env.POLYMARKET_INDEXER_USER_STREAM_URL || DEFAULT_USER_STREAM_URL;

  return new Promise((resolve) => {
    const events = [];
    const seen = new Set();
    const conditionIds = trackedMarkets.map((market) => market.conditionId);
    let opened = false;
    let resolved = false;
    let closeCode = null;
    let closeReason = '';
    let streamError = '';
    let timer = null;
    let pingTimer = null;
    let ws;

    const finish = (status, extra = {}) => {
      if (resolved) {
        return;
      }
      resolved = true;
      clearTimeout(timer);
      clearInterval(pingTimer);
      try {
        ws?.close?.();
      } catch {}

      const counts = parseLifecycleCounts(events);
      resolve({
        ok: !['failed', 'unauthorized', 'disconnected'].includes(status),
        status,
        events,
        marketCount: trackedMarkets.length,
        closeCode,
        closeReason,
        error: streamError || extra.error || '',
        lastEventAt:
          events.at(-1)?.last_update ||
          events.at(-1)?.lastUpdate ||
          events.at(-1)?.matchtime ||
          events.at(-1)?.matchTime ||
          null,
        ...counts,
      });
    };

    try {
      ws = new WebSocket(wsUrl);
    } catch (error) {
      finish('failed', { error: error.message });
      return;
    }

    ws.addEventListener('open', () => {
      opened = true;
      ws.send(
        JSON.stringify({
          auth: {
            apiKey: credentials.apiKey,
            secret: credentials.apiSecret,
            passphrase: credentials.apiPassphrase,
          },
          markets: conditionIds,
          type: 'user',
        }),
      );
      pingTimer = setInterval(() => {
        try {
          ws.send('PING');
        } catch {}
      }, 10_000);
      timer = setTimeout(() => finish('ready'), windowMs);
    });

    ws.addEventListener('message', (message) => {
      const text =
        typeof message.data === 'string'
          ? message.data
          : Buffer.from(message.data || '').toString('utf8');
      if (text === 'PING') {
        ws.send('PONG');
        return;
      }
      if (text === 'PONG' || text === 'pong') {
        return;
      }

      let payload;
      try {
        payload = JSON.parse(text);
      } catch {
        return;
      }

      const errorMessage = String(
        payload?.error || payload?.message || payload?.reason || '',
      ).trim();
      if (errorMessage) {
        streamError = errorMessage;
      }

      for (const event of extractTradeEvents(payload)) {
        const key = lifecycleEventKey(event);
        if (seen.has(key)) {
          continue;
        }
        seen.add(key);
        events.push(event);
        if (events.length >= maxEvents) {
          finish('ready');
          return;
        }
      }
    });

    ws.addEventListener('error', (event) => {
      streamError =
        streamError ||
        event?.message ||
        event?.error?.message ||
        'user stream error';
    });

    ws.addEventListener('close', (event) => {
      closeCode = event?.code ?? null;
      closeReason = String(event?.reason || '').trim();
      if (resolved) {
        return;
      }
      if (!opened) {
        finish('failed', { error: streamError || closeReason || 'user stream failed' });
        return;
      }
      finish('disconnected', {
        error: streamError || closeReason || 'user stream disconnected',
      });
    });
  });
}

function laneFailure(prefix, lane) {
  const message = lane?.lastError || `${prefix} status ${lane?.status || 'unknown'}`;
  return {
    code: prefix,
    message,
  };
}

function buildFailure({ health, userStream, relayer }) {
  if (!health) {
    return {
      code: 'indexer_health_missing',
      message: 'Polymarket indexer health payload missing',
    };
  }

  if (userStream && ['failed', 'unauthorized'].includes(userStream.status)) {
    return {
      code: 'user_stream_credentials_failed',
      message: userStream.error || 'user stream credentials or session failed',
    };
  }
  if (userStream?.status === 'disconnected') {
    return {
      code: 'stream_disconnected',
      message: userStream.error || 'user stream disconnected',
    };
  }
  if (relayer?.status === 'failed') {
    return {
      code: 'reconciliation_failed',
      message: relayer.error || 'relayer reconciliation failed',
    };
  }
  if (health.publicTape?.status === 'error') {
    return laneFailure('public_tape_failed', health.publicTape);
  }
  if (health.userFills?.status === 'error') {
    return laneFailure('user_fills_failed', health.userFills);
  }

  return null;
}

function normalizeMetadata(health, backfill, userStream, relayer) {
  const reconciliationUpdated =
    backfill?.userLifecycleEventsReconciled ??
    backfill?.user_lifecycle_events_reconciled ??
    0;
  const lifecycleCounts = {
    matched:
      health?.userFills?.matchedEvents ?? health?.userFills?.matched_events ?? userStream?.matched ?? 0,
    mined:
      health?.userFills?.minedEvents ?? health?.userFills?.mined_events ?? userStream?.mined ?? 0,
    confirmed:
      health?.userFills?.confirmedEvents ??
      health?.userFills?.confirmed_events ??
      userStream?.confirmed ??
      0,
    retrying:
      health?.userFills?.retryingEvents ??
      health?.userFills?.retrying_events ??
      userStream?.retrying ??
      0,
    failed:
      health?.userFills?.failedEvents ?? health?.userFills?.failed_events ?? userStream?.failed ?? 0,
  };

  return {
    apiUrl: apiBase,
    chainId,
    limit,
    trackedMarkets: health?.trackedMarkets ?? backfill?.trackedMarkets ?? 0,
    trackedMarketDetails:
      health?.trackedMarketDetails ?? backfill?.trackedMarketDetails ?? [],
    publicTape: health?.publicTape ?? null,
    userFills: health?.userFills ?? null,
    backfill: backfill ?? null,
    relayer: relayer ?? null,
    substates: {
      stream: {
        status:
          userStream?.status === 'ready'
            ? 'connected'
            : userStream?.status || 'idle',
        disconnected: userStream?.status === 'disconnected',
        marketCount: userStream?.marketCount ?? 0,
        collectedEvents: userStream?.events?.length ?? 0,
        error: userStream?.error || '',
      },
      userStream: {
        status: userStream?.status || 'idle',
        collectedEvents: userStream?.events?.length ?? 0,
        matchedEvents: userStream?.matched ?? 0,
        minedEvents: userStream?.mined ?? 0,
        confirmedEvents: userStream?.confirmed ?? 0,
        retryingEvents: userStream?.retrying ?? 0,
        failedEvents: userStream?.failed ?? 0,
        lastEventAt:
          health?.userFills?.lastEventAt ??
          health?.userFills?.last_event_at ??
          userStream?.lastEventAt ??
          null,
        error: userStream?.error || '',
      },
      backfill: {
        status:
          backfill?.userFills?.status === 'partial' || backfill?.publicTape?.status === 'partial'
            ? 'partial'
            : 'ready',
        publicTradesIngested:
          backfill?.publicTradesIngested ?? backfill?.public_trades_ingested ?? 0,
        userFillEventsIngested:
          backfill?.userFillEventsIngested ?? backfill?.user_fill_events_ingested ?? 0,
      },
      reconciliation: {
        status: relayer?.status === 'failed' ? 'failed' : 'ready',
        updated: reconciliationUpdated,
        fetchedTransactions: relayer?.fetched ?? 0,
        consecutiveFailures: relayer?.status === 'failed' ? 1 : 0,
        error: relayer?.error || '',
      },
      cursor: {
        status:
          health?.publicTape?.status === 'partial' || health?.userFills?.status === 'partial'
            ? 'partial'
            : 'ready',
        indexedThrough:
          health?.publicTape?.indexedThrough ??
          health?.publicTape?.indexed_through ??
          health?.userFills?.indexedThrough ??
          health?.userFills?.indexed_through ??
          null,
      },
    },
    lifecycle: lifecycleCounts,
  };
}

export async function loginAdmin() {
  const privateKey = String(process.env.POLYMARKET_INDEXER_ADMIN_PRIVATE_KEY || '').trim();
  if (!privateKey) {
    const adminKey = String(process.env.ADMIN_CONTROL_KEY || '').trim();
    if (!adminKey) {
      throw new Error('POLYMARKET_INDEXER_ADMIN_PRIVATE_KEY or ADMIN_CONTROL_KEY is required');
    }

    return {
      account: null,
      accessToken: null,
      authMode: 'admin_key',
    };
  }

  const account = privateKeyToAccount(privateKey);
  const noncePayload = await fetchJson(`${apiBase}/auth/siwe/nonce`);
  const nonce = noncePayload?.nonce;

  if (!nonce) {
    throw new Error('missing SIWE nonce');
  }

  const issuedAt = new Date().toISOString();
  const message = `${siweDomain} wants you to sign in with your Ethereum account:\n${account.address}\n\nSign in to relay44 polymarket indexer\n\nURI: ${apiOrigin}\nVersion: 1\nChain ID: ${chainId}\nNonce: ${nonce}\nIssued At: ${issuedAt}`;
  const signature = await account.signMessage({ message });
  const tokens = await fetchJson(`${apiBase}/auth/siwe/login`, {
    method: 'POST',
    headers: buildHeaders({ hasBody: true }),
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
    authMode: 'siwe',
  };
}

export async function fetchIndexerHealth(accessToken) {
  return requestApi('/external/indexers/polymarket/health', { accessToken });
}

export async function triggerIndexerBackfill(
  accessToken,
  { userEvents = [], relayerTransactions = [] } = {},
) {
  return requestApi('/external/indexers/polymarket/backfill', {
    method: 'POST',
    accessToken,
    body: {
      maxMarkets: limit,
      days: 90,
      publicTape: true,
      userFills: true,
      userEvents,
      relayerTransactions,
    },
  });
}

export async function inspectPolymarketIndexer(accessToken) {
  const health = await fetchIndexerHealth(accessToken);
  const failure = buildFailure({ health, userStream: null, relayer: null });

  return {
    ok: !failure,
    runnerName: RUNNER_NAME,
    failure,
    health,
    metadata: normalizeMetadata(health, null, null, null),
  };
}

export async function runPolymarketIndexerTick(env = process.env) {
  if (!enabled) {
    return {
      ok: true,
      skipped: true,
      reason: 'polymarket indexer disabled',
    };
  }

  const opsState = await connectOpsState(env).catch(() => null);
  const startedAt = new Date().toISOString();

  try {
    await reportRunnerStarted(opsState, RUNNER_NAME, {
      startedAt,
      apiUrl: apiBase,
      chainId,
      limit,
    });

    const { accessToken } = await loginAdmin();
    const healthBefore = await fetchIndexerHealth(accessToken);
    const trackedMarkets = normalizeTrackedMarkets(
      healthBefore?.trackedMarketDetails ?? healthBefore?.tracked_market_details,
    );
    const [userStream, relayer] = await Promise.all([
      collectUserLifecycleEvents(trackedMarkets),
      fetchRelayerTransactions(),
    ]);
    const backfill = await triggerIndexerBackfill(accessToken, {
      userEvents: userStream?.events ?? [],
      relayerTransactions: relayer?.transactions ?? [],
    });
    const health = await fetchIndexerHealth(accessToken);
    const failure = buildFailure({ health, userStream, relayer });
    const metadata = normalizeMetadata(health, backfill, userStream, relayer);

    if (failure) {
      await reportRunnerFailure(
        opsState,
        RUNNER_NAME,
        failure.code,
        failure.message,
        {
          startedAt,
          ...metadata,
        },
      );
      return {
        ok: false,
        startedAt,
        runnerName: RUNNER_NAME,
        backfill,
        health,
        userStream,
        relayer,
        failure,
      };
    }

    await reportRunnerSuccess(opsState, RUNNER_NAME, {
      startedAt,
      ...metadata,
    });

    return {
      ok: true,
      startedAt,
      runnerName: RUNNER_NAME,
      backfill,
      health,
      userStream,
      relayer,
    };
  } finally {
    await closeOpsState(opsState);
  }
}
