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

const API_URL = process.env.API_URL || "https://relay44-api.onrender.com/v1";
const X402_RUNNER_NAME = "x402_smoke";

const healthUrl = API_URL.replace(/\/v1$/, "/health/detailed");

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
const x402Enabled = String(process.env.X402_SMOKE_ENABLED || "")
  .trim()
  .toLowerCase();
const smokeEnabled = ["1", "true", "yes", "on"].includes(x402Enabled);
if (smokeEnabled) {
  const now = new Date();

  if (shouldRunScheduledSmoke(now, process.env)) {
    try {
      await reportRunnerStarted(opsState, X402_RUNNER_NAME, {
        scheduledAt: now.toISOString(),
      });
      x402Result = await runX402Smoke(process.env);
      await reportRunnerSuccess(
        opsState,
        X402_RUNNER_NAME,
        buildSuccessMetadata(x402Result),
      );

      if (x402Result.lowBalanceTriggered) {
        failures.push(
          `x402_wallet_low: ${x402Result.balanceAfterUsdc} USDC < ${x402Result.lowBalanceThresholdUsdc} USDC`,
        );
      }
    } catch (error) {
      await reportRunnerFailure(
        opsState,
        X402_RUNNER_NAME,
        error.code || "smoke_failed",
        error.message,
        buildFailureMetadata(error),
      );
      failures.push(`${formatFailureStage(error)}: failed (${error.message})`);
    }
  } else if (opsState) {
    const state = await getRunnerState(opsState, X402_RUNNER_NAME);
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
