#!/usr/bin/env node

import pg from "pg";

import {
  closeOpsState,
  connectOpsState,
  getRunnerState,
  reportRunnerFailure,
  reportRunnerStarted,
  reportRunnerSuccess,
} from "./ops-state.mjs";
import { sendAlert } from "./ops-alerts.mjs";
import {
  runX402Smoke,
  scheduledSmokeOverdueMs,
  shouldRunScheduledSmoke,
} from "./x402-smoke.mjs";
import {
  runOrderSmoke,
  scheduledSmokeOverdueMs as orderSmokeOverdueMs,
  shouldRunScheduledSmoke as shouldRunScheduledOrderSmoke,
} from "./order-smoke.mjs";

const { Client } = pg;

const API_URL = process.env.API_URL || "https://relay44-api.onrender.com/v1";
const X402_RUNNER_NAME = "x402_smoke";
const ORDER_SMOKE_RUNNER_NAME = "order_smoke";
const EXTERNAL_RUNNER_NAME = "external_runner";
const BOOTSTRAP_RUNNER_NAME = "bootstrap_operator";
const POLYMARKET_INDEXER_RUNNER_NAME = "polymarket_indexer";
const CREATOR_ECONOMICS_RUNNER_NAME = "creator_economics_materializer";
const CREATOR_ECONOMICS_MATERIALIZER_RUNNER_NAME =
  "creator_economics_materializer";

const healthUrl = API_URL.replace(/\/v1$/, "/health/detailed");

function isEnabled(raw) {
  return ["1", "true", "yes", "on"].includes(
    String(raw || "").trim().toLowerCase(),
  );
}

function parseNumber(value, fallback) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function buildDbConfig(connectionString) {
  const url = new URL(connectionString);
  const sslmode = (url.searchParams.get("sslmode") || "").trim().toLowerCase();
  const needsSsl =
    ["require", "verify-ca", "verify-full", "prefer"].includes(sslmode) ||
    /\.render\.com$/i.test(url.hostname);

  return needsSsl
    ? {
        connectionString,
        connectionTimeoutMillis: 10_000,
        keepAlive: true,
        ssl: { rejectUnauthorized: false },
      }
    : {
        connectionString,
        connectionTimeoutMillis: 10_000,
        keepAlive: true,
      };
}

async function fetchCreatorEconomicsFreshness(env) {
  const connectionString = String(env.DATABASE_URL || "").trim();
  if (!connectionString) {
    return null;
  }

  const client = new Client(buildDbConfig(connectionString));
  await client.connect();

  try {
    const maxRowAgeMinutes = parseNumber(
      env.CREATOR_ECONOMICS_MAX_ROW_AGE_MINUTES,
      180,
    );
    const summary = await client.query(
      `
        with expected as (
          select market_id, lower(creator) as creator
          from base_market_bootstrap_configs
          where liquidity_mode = 'bootstrap_hybrid'
            and seed_usdc > 0
        ),
        actual as (
          select market_id, lower(creator) as creator, updated_at
          from creator_market_economics_daily
          where day = current_date
        )
        select
          count(*)::bigint as expected_count,
          count(a.market_id)::bigint as materialized_count,
          count(*) filter (where a.market_id is null)::bigint as missing_count,
          count(*) filter (
            where a.updated_at is not null
              and a.updated_at < now() - make_interval(mins => $1::int)
          )::bigint as stale_count
        from expected e
        left join actual a
          on a.market_id = e.market_id
         and a.creator = e.creator
      `,
      [Math.max(1, Math.trunc(maxRowAgeMinutes))],
    );
    const preview = await client.query(
      `
        with expected as (
          select market_id, lower(creator) as creator
          from base_market_bootstrap_configs
          where liquidity_mode = 'bootstrap_hybrid'
            and seed_usdc > 0
        ),
        actual as (
          select market_id, lower(creator) as creator, updated_at
          from creator_market_economics_daily
          where day = current_date
        )
        select
          e.market_id,
          e.creator,
          a.updated_at
        from expected e
        left join actual a
          on a.market_id = e.market_id
         and a.creator = e.creator
        where a.market_id is null
           or (
             a.updated_at is not null
             and a.updated_at < now() - make_interval(mins => $1::int)
           )
        order by e.market_id asc
        limit 5
      `,
      [Math.max(1, Math.trunc(maxRowAgeMinutes))],
    );

    return {
      maxRowAgeMinutes,
      expectedCount: parseNumber(summary.rows[0]?.expected_count, 0),
      materializedCount: parseNumber(summary.rows[0]?.materialized_count, 0),
      missingCount: parseNumber(summary.rows[0]?.missing_count, 0),
      staleCount: parseNumber(summary.rows[0]?.stale_count, 0),
      preview: preview.rows.map((row) => ({
        marketId: parseNumber(row.market_id, 0),
        creator: row.creator,
        updatedAt: row.updated_at ? new Date(row.updated_at).toISOString() : null,
      })),
    };
  } finally {
    await client.end().catch(() => {});
  }
}

async function fetchApiJson(path) {
  const response = await fetch(`${API_URL}${path}`, {
    signal: AbortSignal.timeout(30_000),
    headers: { Accept: "application/json" },
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
    throw new Error(
      `${path} ${response.status}: ${payload?.message || payload?.error || response.statusText}`,
    );
  }
  return payload;
}

async function fetchApiJsonAdmin(path) {
  const adminKey = String(process.env.ADMIN_CONTROL_KEY || "").trim();
  if (!adminKey) {
    throw new Error("missing ADMIN_CONTROL_KEY");
  }

  const response = await fetch(`${API_URL}${path}`, {
    signal: AbortSignal.timeout(30_000),
    headers: {
      Accept: "application/json",
      "x-admin-key": adminKey,
    },
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
    throw new Error(
      `${path} ${response.status}: ${payload?.message || payload?.error || response.statusText}`,
    );
  }
  return payload;
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

  const errorCode = state?.last_error_code ? ` after ${state.last_error_code}` : "";
  return `${label}_overdue: last success ${lastSucceededAt.toISOString()}${errorCode}`;
}

function buildBootstrapStatusFailures(markets) {
  const problematic = markets.filter((market) =>
    market?.liquidity_mode === "bootstrap_hybrid" &&
    ["paused", "error", "pending_funding", "pending_authorization"].includes(
      String(market.bootstrap_status || "").trim().toLowerCase(),
    ),
  );

  if (problematic.length === 0) {
    return [];
  }

  const preview = problematic
    .slice(0, 5)
    .map(
      (market) =>
        `${market.id}:${market.bootstrap_status}${market.bootstrap_pause_reason ? `(${market.bootstrap_pause_reason})` : ""}`,
    )
    .join(", ");
  return [
    `bootstrap_markets: ${problematic.length} blocked or paused (${preview})`,
  ];
}

function asObject(value) {
  return value && typeof value === "object" ? value : {};
}

function pickStatus(...values) {
  for (const value of values) {
    if (value == null) {
      continue;
    }

    const normalized = String(value).trim().toLowerCase();
    if (normalized) {
      return normalized;
    }
  }

  return "";
}

function getPolymarketIndexerSubstate(state, name) {
  const metadata = asObject(state?.metadata);
  const substates = asObject(metadata.substates);
  const camelName = name.replace(/_([a-z])/g, (_, ch) => ch.toUpperCase());
  const candidates = [
    substates[name],
    substates[camelName],
    metadata[name],
    metadata[camelName],
  ];

  if (name === "user_stream" || name === "userStream") {
    candidates.push(asObject(metadata.credentials)?.userStream);
  }

  for (const candidate of candidates) {
    if (candidate && typeof candidate === "object" && Object.keys(candidate).length > 0) {
      return candidate;
    }
  }

  return {};
}

function buildPolymarketIndexerFailures(state, env = process.env) {
  if (!state) {
    return [];
  }

  const metadata = asObject(state.metadata);
  const failures = [];
  const lastErrorCode = pickStatus(state.last_error_code, metadata.lastErrorCode);
  const lastErrorMessage = String(
    state.last_error_message || metadata.lastErrorMessage || "",
  ).trim();

  const stream = getPolymarketIndexerSubstate(state, "stream");
  const streamStatus = pickStatus(stream.status, stream.state);
  if (streamStatus === "disconnected" || stream.disconnected === true) {
    failures.push(
      `polymarket_indexer_stream_disconnected: ${stream.error || "forwarder or stream disconnected"}`,
    );
  }

  const cursor = getPolymarketIndexerSubstate(state, "cursor");
  const cursorStatus = pickStatus(cursor.status, cursor.state);
  if (cursorStatus === "stalled" || cursor.stalled === true) {
    const block =
      cursor.cursorBlock ?? cursor.cursor_block ?? cursor.block ?? null;
    failures.push(
      `polymarket_indexer_cursor_stalled: ${
        block != null ? `cursor stalled at block ${block}` : "cursor stalled"
      }`,
    );
  }

  const backfill = getPolymarketIndexerSubstate(state, "backfill");
  const backfillStatus = pickStatus(backfill.status, backfill.state);
  const backfillLagBlocks = Number(
    backfill.lagBlocks ??
      backfill.lag_blocks ??
      metadata.backfillLagBlocks ??
      metadata.backfill_lag_blocks ??
      NaN,
  );
  const maxBackfillLagBlocks = parseNumber(
    env.POLYMARKET_INDEXER_MAX_BACKFILL_LAG_BLOCKS,
    250,
  );
  if (
    backfillStatus === "lagging" ||
    (Number.isFinite(backfillLagBlocks) && backfillLagBlocks > maxBackfillLagBlocks)
  ) {
    failures.push(
      `polymarket_indexer_backfill_lag: ${
        Number.isFinite(backfillLagBlocks)
          ? `${backfillLagBlocks} blocks > ${maxBackfillLagBlocks}`
          : "backfill lagging"
      }`,
    );
  }

  const reconciliation = getPolymarketIndexerSubstate(state, "reconciliation");
  const reconciliationStatus = pickStatus(
    reconciliation.status,
    reconciliation.state,
  );
  const consecutiveFailures = Number(
    reconciliation.consecutiveFailures ??
      reconciliation.consecutive_failures ??
      metadata.consecutiveReconciliationFailures ??
      metadata.consecutive_reconciliation_failures ??
      NaN,
  );
  const maxReconciliationFailures = parseNumber(
    env.POLYMARKET_INDEXER_MAX_RECONCILIATION_FAILURES,
    3,
  );
  if (
    reconciliationStatus === "failed" ||
    (Number.isFinite(consecutiveFailures) &&
      consecutiveFailures > maxReconciliationFailures)
  ) {
    failures.push(
      `polymarket_indexer_reconciliation_failed: ${
        Number.isFinite(consecutiveFailures)
          ? `${consecutiveFailures} consecutive failures > ${maxReconciliationFailures}`
          : "reconciliation failed repeatedly"
      }`,
    );
  }

  const userStream = getPolymarketIndexerSubstate(state, "user_stream");
  const userStreamStatus = pickStatus(
    userStream.status,
    userStream.state,
    metadata.credentials?.userStream?.status,
  );
  if (
    ["failed", "disconnected", "revoked", "expired", "unauthorized"].includes(
      userStreamStatus,
    )
  ) {
    failures.push(
      `polymarket_indexer_user_stream_failed: ${
        userStream.error || "user stream credentials or session failed"
      }`,
    );
  }
  if (userStreamStatus === "missing_credentials") {
    failures.push(
      `polymarket_indexer_user_stream_failed: ${
        userStream.error || "builder credentials are not configured"
      }`,
    );
  }

  if (state.last_status === "failed" && failures.length === 0) {
    let message = lastErrorMessage || "runner reported failure";
    if (lastErrorCode === "stream_disconnected") {
      message = lastErrorMessage || "forwarder or stream disconnected";
      failures.push(`polymarket_indexer_stream_disconnected: ${message}`);
      return failures;
    }
    if (lastErrorCode === "cursor_stalled") {
      message = lastErrorMessage || "cursor stalled";
      failures.push(`polymarket_indexer_cursor_stalled: ${message}`);
      return failures;
    }
    if (lastErrorCode === "backfill_lag") {
      message = lastErrorMessage || "backfill lagging";
      failures.push(`polymarket_indexer_backfill_lag: ${message}`);
      return failures;
    }
    if (lastErrorCode === "reconciliation_failed") {
      message = lastErrorMessage || "reconciliation failed repeatedly";
      failures.push(`polymarket_indexer_reconciliation_failed: ${message}`);
      return failures;
    }
    if (lastErrorCode === "user_stream_credentials_failed") {
      message = lastErrorMessage || "user stream credentials or session failed";
      failures.push(`polymarket_indexer_user_stream_failed: ${message}`);
      return failures;
    }
    failures.push(
      `polymarket_indexer_failed: ${lastErrorCode ? `${lastErrorCode}${lastErrorMessage ? ` (${lastErrorMessage})` : ""}` : message}`,
    );
  }

  return failures;
}

function buildCreatorEconomicsMaterializerFailures(health, env = process.env) {
  if (!health || typeof health !== "object") {
    return [];
  }

  const failures = [];
  const status = pickStatus(health.status);
  const maxLagDays = parseNumber(
    env.CREATOR_ECONOMICS_MATERIALIZER_MAX_LAG_DAYS,
    1,
  );
  const staleMarkets = Number(health.staleMarkets ?? health.stale_markets ?? 0);
  const maxLagObserved = Number(health.maxLagDays ?? health.max_lag_days ?? 0);

  if (status && !["healthy", "idle"].includes(status)) {
    failures.push(
      `creator_economics_materializer_health: ${status}${staleMarkets > 0 ? ` (${staleMarkets} stale markets)` : ""}`,
    );
  }

  if (Number.isFinite(maxLagObserved) && maxLagObserved > maxLagDays) {
    failures.push(
      `creator_economics_materializer_lag: ${maxLagObserved}d > ${maxLagDays}d`,
    );
  }

  return failures;
}

function formatFailureStage(error) {
  const code = String(error?.code || "smoke_failed")
    .trim()
    .toLowerCase();
  if (code === "verify_failed") {
    return "x402_verify";
  }
  if (code === "settle_failed") {
    return "x402_settle";
  }
  if (code === "wallet_underfunded") {
    return "x402_wallet";
  }
  return `x402_${code}`;
}

function buildSuccessMetadata(result) {
  return {
    payer: result.payer,
    routeCount: result.routeCount,
    routes: result.routes.map((route) => ({
      targetName: route.targetName,
      targetUrl: route.targetUrl,
      acceptedMicrousdc: route.acceptedMicrousdc,
      chargedMicrousdc: route.chargedMicrousdc,
      paymentResponseHeaderPresent: route.paymentResponseHeaderPresent,
      settlementTx: route.settlement?.transaction || null,
    })),
    balanceBeforeMicrousdc: result.balanceBeforeMicrousdc,
    balanceAfterMicrousdc: result.balanceAfterMicrousdc,
    chargedMicrousdc: result.chargedMicrousdc,
    lowBalanceThresholdMicrousdc: result.lowBalanceThresholdMicrousdc,
    lowBalanceTriggered: result.lowBalanceTriggered,
  };
}

function buildFailureMetadata(error) {
  return {
    code: error.code || "smoke_failed",
    targetName: error.targetName || null,
    targetUrl: error.targetUrl || null,
    status: error.status || null,
    failureContext: error.failureContext || null,
    routeResults: Array.isArray(error.routeResults) ? error.routeResults : [],
    payer: error.payer || null,
    balanceBeforeMicrousdc: error.balanceBeforeMicrousdc || null,
    balanceAfterMicrousdc: error.balanceAfterMicrousdc || null,
    lowBalanceThresholdMicrousdc: error.lowBalanceThresholdMicrousdc || null,
  };
}

let data;
try {
  const response = await fetch(healthUrl, {
    signal: AbortSignal.timeout(30_000),
  });
  data = await response.json();
} catch (err) {
  const msg = `[relay44 ALERT] API unreachable: ${err.message}`;
  console.error(msg);
  await sendAlert(msg);
  process.exit(1);
}

console.log(JSON.stringify(data, null, 2));

const failures = [];
let opsState = null;
let opsStateError = null;

try {
  opsState = await connectOpsState(process.env);
} catch (error) {
  opsStateError = error;
  console.warn(`ops_state unavailable: ${error.message}`);
}

async function withOpsState(action) {
  if (!opsState) {
    return null;
  }

  try {
    return await action();
  } catch (error) {
    if (!opsStateError) {
      opsStateError = error;
      console.warn(`ops_state unavailable: ${error.message}`);
    }
    await closeOpsState(opsState);
    opsState = null;
    return null;
  }
}

if (data.status !== "healthy") {
  failures.push(`status: ${data.status}`);
}

const checks = data.checks || {};
for (const [name, check] of Object.entries(checks)) {
  if (
    check.status !== "healthy" &&
    check.message !== "Solana integration disabled"
  ) {
    failures.push(
      `${name}: ${check.status} (${check.message || "no details"})`,
    );
  }
}

let x402Result = null;
const orderSmokeEnabled = isEnabled(process.env.ORDER_SMOKE_ENABLED);
const x402Enabled = String(process.env.X402_SMOKE_ENABLED || "")
  .trim()
  .toLowerCase();
const smokeEnabled = ["1", "true", "yes", "on"].includes(x402Enabled);
if (orderSmokeEnabled) {
  const now = new Date();

  if (shouldRunScheduledOrderSmoke(now, process.env)) {
    try {
      await withOpsState(() =>
        reportRunnerStarted(opsState, ORDER_SMOKE_RUNNER_NAME, {
          scheduledAt: now.toISOString(),
        }),
      );
      const orderSmokeResult = await runOrderSmoke(process.env);
      await withOpsState(() =>
        reportRunnerSuccess(opsState, ORDER_SMOKE_RUNNER_NAME, orderSmokeResult),
      );
    } catch (error) {
      await withOpsState(() =>
        reportRunnerFailure(
          opsState,
          ORDER_SMOKE_RUNNER_NAME,
          error.code || "smoke_failed",
          error.message,
          {
            code: error.code || "smoke_failed",
            phase: error.phase || null,
            targetMarketId: error.targetMarketId || null,
            orderId: error.orderId || null,
            status: error.status || null,
            response: error.response || null,
          },
        ),
      );
      failures.push(`order_smoke: failed (${error.message})`);
    }
  }

  if (opsState) {
    const state = await withOpsState(() =>
      getRunnerState(opsState, ORDER_SMOKE_RUNNER_NAME),
    );
    const overdueMs = orderSmokeOverdueMs(process.env);
    const message = runnerOverdueMessage("order_smoke", state, overdueMs);
    if (state?.last_succeeded_at && message) {
      failures.push(message);
    }
  }
}

if (smokeEnabled) {
  const now = new Date();

  if (shouldRunScheduledSmoke(now, process.env)) {
    try {
      await withOpsState(() =>
        reportRunnerStarted(opsState, X402_RUNNER_NAME, {
          scheduledAt: now.toISOString(),
        }),
      );
      x402Result = await runX402Smoke(process.env);
      await withOpsState(() =>
        reportRunnerSuccess(
          opsState,
          X402_RUNNER_NAME,
          buildSuccessMetadata(x402Result),
        ),
      );

      if (x402Result.lowBalanceTriggered) {
        failures.push(
          `x402_wallet_low: ${x402Result.balanceAfterUsdc} USDC < ${x402Result.lowBalanceThresholdUsdc} USDC`,
        );
      }
    } catch (error) {
      await withOpsState(() =>
        reportRunnerFailure(
          opsState,
          X402_RUNNER_NAME,
          error.code || "smoke_failed",
          error.message,
          buildFailureMetadata(error),
        ),
      );
      failures.push(`${formatFailureStage(error)}: failed (${error.message})`);
    }
  } else if (opsState) {
    const state = await withOpsState(() =>
      getRunnerState(opsState, X402_RUNNER_NAME),
    );
    const lastSucceededAt = state?.last_succeeded_at
      ? new Date(state.last_succeeded_at)
      : null;
    const overdueMs = scheduledSmokeOverdueMs(process.env);

    if (lastSucceededAt && Date.now() - lastSucceededAt.getTime() > overdueMs) {
      const errorCode = state?.last_error_code
        ? ` after ${state.last_error_code}`
        : "";
      failures.push(
        `x402_overdue: last success ${lastSucceededAt.toISOString()}${errorCode}`,
      );
    }
  }
}

if (opsState) {
  const externalEnabled = isEnabled(process.env.EXTERNAL_RUNNER_ENABLED);
  if (externalEnabled) {
    const state = await withOpsState(() =>
      getRunnerState(opsState, EXTERNAL_RUNNER_NAME),
    );
    const overdueMs = parseNumber(
      process.env.EXTERNAL_RUNNER_OVERDUE_MS,
      parseNumber(process.env.EXTERNAL_RUNNER_INTERVAL_MS, 60_000) * 3,
    );
    const message = runnerOverdueMessage("external_runner", state, overdueMs);
    if (message) {
      failures.push(message);
    }
  }

  const polymarketIndexerEnabled = isEnabled(
    process.env.POLYMARKET_INDEXER_ENABLED,
  );
  if (polymarketIndexerEnabled) {
    const state = await withOpsState(() =>
      getRunnerState(opsState, POLYMARKET_INDEXER_RUNNER_NAME),
    );
    const overdueMs = parseNumber(
      process.env.POLYMARKET_INDEXER_OVERDUE_MS,
      15 * 60_000,
    );
    const message = runnerOverdueMessage(
      "polymarket_indexer",
      state,
      overdueMs,
    );
    if (message) {
      failures.push(message);
    }
    failures.push(...buildPolymarketIndexerFailures(state, process.env));
  }

  const creatorMaterializerEnabled = isEnabled(
    process.env.CREATOR_ECONOMICS_MATERIALIZER_ENABLED,
  );
  if (creatorMaterializerEnabled) {
    const state = await withOpsState(() =>
      getRunnerState(opsState, CREATOR_ECONOMICS_MATERIALIZER_RUNNER_NAME),
    );
    const overdueMs = parseNumber(
      process.env.CREATOR_ECONOMICS_MATERIALIZER_OVERDUE_MS,
      6 * 60 * 60_000,
    );
    const message = runnerOverdueMessage(
      "creator_economics_materializer",
      state,
      overdueMs,
    );
    if (message) {
      failures.push(message);
    }
  }

  const bootstrapEnabled = isEnabled(process.env.BASE_AGENT_OPERATOR_ENABLED);
  if (bootstrapEnabled) {
    const state = await withOpsState(() =>
      getRunnerState(opsState, BOOTSTRAP_RUNNER_NAME),
    );
    const overdueMs = parseNumber(
      process.env.BASE_AGENT_OPERATOR_OVERDUE_MS,
      15 * 60_000,
    );
    const message = runnerOverdueMessage("bootstrap_operator", state, overdueMs);
    if (message) {
      failures.push(message);
    }
  }

  const creatorEconomicsEnabled = isEnabled(
    process.env.CREATOR_ECONOMICS_MATERIALIZER_ENABLED,
  );
  if (creatorEconomicsEnabled) {
    const state = await withOpsState(() =>
      getRunnerState(opsState, CREATOR_ECONOMICS_RUNNER_NAME),
    );
    const overdueMs = parseNumber(
      process.env.CREATOR_ECONOMICS_MATERIALIZER_OVERDUE_MS,
      6 * 60 * 60_000,
    );
    const message = runnerOverdueMessage(
      "creator_economics_materializer",
      state,
      overdueMs,
    );
    if (message) {
      failures.push(message);
    }

    try {
      const freshness = await fetchCreatorEconomicsFreshness(process.env);
      if (freshness) {
        if (freshness.missingCount > 0) {
          const preview = freshness.preview
            .map((entry) => `${entry.marketId}:${entry.creator}`)
            .join(", ");
          failures.push(
            `creator_economics_missing: ${freshness.missingCount}/${freshness.expectedCount} bootstrap creator markets missing today's row${preview ? ` (${preview})` : ""}`,
          );
        }
        if (freshness.staleCount > 0) {
          const preview = freshness.preview
            .filter((entry) => entry.updatedAt)
            .map(
              (entry) =>
                `${entry.marketId}:${entry.creator}@${entry.updatedAt}`,
            )
            .join(", ");
          failures.push(
            `creator_economics_stale: ${freshness.staleCount} rows older than ${freshness.maxRowAgeMinutes}m${preview ? ` (${preview})` : ""}`,
          );
        }
      }
    } catch (error) {
      failures.push(`creator_economics_health: ${error.message}`);
    }
  }
}

try {
  const matcherHealth = await fetchApiJson("/evm/matcher/health");
  const maxMatcherBacklog = parseNumber(process.env.MATCHER_MAX_BACKLOG, 0);
  const matcherUpdatedAt = matcherHealth?.updated_at
    ? new Date(matcherHealth.updated_at)
    : null;
  const matcherStaleMs = parseNumber(
    process.env.MATCHER_MAX_STALE_MS,
    10 * 60_000,
  );

  if (matcherHealth?.paused) {
    failures.push(
      `matcher_paused: ${matcherHealth.reason || "matcher is paused"}`,
    );
  } else if (Number(matcherHealth?.backlog || 0) > maxMatcherBacklog) {
    failures.push(
      `matcher_backlog: ${matcherHealth.backlog} > ${maxMatcherBacklog}`,
    );
  } else if (
    matcherUpdatedAt &&
    Date.now() - matcherUpdatedAt.getTime() > matcherStaleMs
  ) {
    failures.push(
      `matcher_stale: last cycle ${matcherUpdatedAt.toISOString()}`,
    );
  }
} catch (error) {
  failures.push(`matcher_health: ${error.message}`);
}

try {
  const payoutHealth = await fetchApiJson("/evm/payouts/health");
  const maxFailed = parseNumber(process.env.PAYOUT_MAX_FAILED, 0);
  const maxOldestPending = parseNumber(
    process.env.PAYOUT_MAX_OLDEST_PENDING_SECONDS,
    15 * 60,
  );

  if (Number(payoutHealth?.failed || 0) > maxFailed) {
    failures.push(`payout_failed: ${payoutHealth.failed} > ${maxFailed}`);
  }
  if (Number(payoutHealth?.oldest_pending_seconds || 0) > maxOldestPending) {
    failures.push(
      `payout_overdue: oldest pending ${payoutHealth.oldest_pending_seconds}s > ${maxOldestPending}s`,
    );
  }
} catch (error) {
  failures.push(`payout_health: ${error.message}`);
}

try {
  const indexerHealth = await fetchApiJson("/evm/indexer/health");
  const maxLagBlocks = parseNumber(process.env.INDEXER_MAX_LAG_BLOCKS, 25);
  if (Number(indexerHealth?.lag_blocks || 0) > maxLagBlocks) {
    failures.push(
      `indexer_lag: ${indexerHealth.lag_blocks} > ${maxLagBlocks} blocks`,
    );
  }
} catch (error) {
  failures.push(`indexer_health: ${error.message}`);
}

try {
  const markets = await fetchApiJson("/evm/markets?source=internal&limit=200");
  failures.push(...buildBootstrapStatusFailures(markets?.markets || []));
} catch (error) {
  failures.push(`bootstrap_markets: ${error.message}`);
}

if (isEnabled(process.env.CREATOR_ECONOMICS_MATERIALIZER_ENABLED)) {
  try {
    const health = await fetchApiJsonAdmin("/evm/creator/materializer/health");
    failures.push(
      ...buildCreatorEconomicsMaterializerFailures(health, process.env),
    );
  } catch (error) {
    failures.push(`creator_economics_materializer: ${error.message}`);
  }
}

if (failures.length > 0) {
  const msg = `[relay44 ALERT] Health check failed:\n${failures.join("\n")}`;
  console.error(msg);
  await sendAlert(msg, process.env);
  await closeOpsState(opsState);
  process.exit(1);
}

if (x402Result) {
  console.log(JSON.stringify({ x402Smoke: x402Result }, null, 2));
}

if (opsStateError) {
  console.log(
    JSON.stringify(
      {
        opsState: {
          status: "degraded",
          message: opsStateError.message,
        },
      },
      null,
      2,
    ),
  );
}

await closeOpsState(opsState);
console.log("All checks passed.");
