#!/usr/bin/env node

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

const API_URL = process.env.API_URL || "https://relay44-api.onrender.com/v1";
const X402_RUNNER_NAME = "x402_smoke";
const ORDER_SMOKE_RUNNER_NAME = "order_smoke";
const EXTERNAL_RUNNER_NAME = "external_runner";
const BOOTSTRAP_RUNNER_NAME = "bootstrap_operator";

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
