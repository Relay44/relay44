#!/usr/bin/env node

import { pathToFileURL } from "node:url";

import { sendAlert } from "./ops-alerts.mjs";
import { isEnabled } from "./runner-framework.mjs";

const USDC_DECIMALS = 6;
const DEFAULT_API_URL = "https://relay44-api.onrender.com/v1";
const DEFAULT_TARGETS = [
  "/evm/markets/12/orderbook?outcome=yes&depth=5",
  "/evm/markets/12/trades?outcome=yes&limit=5",
];
const DEFAULT_RPC_URL = "https://mainnet.base.org";
const DEFAULT_USDC_ADDRESS = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";
const DEFAULT_LOW_BALANCE_USDC = "2";
const DEFAULT_OVERDUE_GRACE_MINUTES = 10;

class X402SmokeError extends Error {
  constructor(code, message, details = {}) {
    super(message);
    this.name = "X402SmokeError";
    this.code = code;
    Object.assign(this, details);
  }
}

function parseUsdc(value) {
  const normalized = String(value || "0").trim();
  if (!/^\d+(\.\d+)?$/.test(normalized)) {
    throw new Error(`invalid USDC amount: ${value}`);
  }

  const [whole, fraction = ""] = normalized.split(".");
  return (
    BigInt(whole) * 1_000_000n +
    BigInt((fraction + "000000").slice(0, USDC_DECIMALS))
  );
}

function parseJson(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function buildApiUrl(env) {
  return (env.X402_SMOKE_API_URL || DEFAULT_API_URL).trim().replace(/\/+$/, "");
}

function parseTargetEntries(env) {
  const rawTargets = String(env.X402_SMOKE_TARGETS || "").trim();
  if (rawTargets) {
    return rawTargets
      .split(/[\n,]/)
      .map((entry) => entry.trim())
      .filter(Boolean);
  }

  const explicitUrl = String(env.X402_SMOKE_TARGET_URL || "").trim();
  if (explicitUrl) {
    return [explicitUrl];
  }

  const legacyPath = String(env.X402_SMOKE_PATH || "").trim();
  if (legacyPath) {
    return [legacyPath];
  }

  return [...DEFAULT_TARGETS];
}

function buildTargetUrls(env) {
  const apiUrl = buildApiUrl(env);
  return parseTargetEntries(env).map((entry) =>
    /^https?:\/\//i.test(entry)
      ? entry
      : `${apiUrl}${entry.startsWith("/") ? entry : `/${entry}`}`,
  );
}

function readScheduleConfig(env = process.env) {
  const minute = Number(env.X402_SMOKE_MINUTE ?? "0");
  const intervalHours = Number(env.X402_SMOKE_INTERVAL_HOURS ?? "1");
  const windowMinutes = Number(env.X402_SMOKE_WINDOW_MINUTES ?? "5");
  const overdueGraceMinutes = Number(
    env.X402_SMOKE_OVERDUE_GRACE_MINUTES ??
      String(DEFAULT_OVERDUE_GRACE_MINUTES),
  );

  if (!Number.isInteger(minute) || minute < 0 || minute > 59) {
    throw new Error(`invalid X402_SMOKE_MINUTE: ${env.X402_SMOKE_MINUTE}`);
  }
  if (
    !Number.isInteger(intervalHours) ||
    intervalHours < 1 ||
    intervalHours > 24
  ) {
    throw new Error(
      `invalid X402_SMOKE_INTERVAL_HOURS: ${env.X402_SMOKE_INTERVAL_HOURS}`,
    );
  }
  if (
    !Number.isInteger(windowMinutes) ||
    windowMinutes < 1 ||
    windowMinutes > 60
  ) {
    throw new Error(
      `invalid X402_SMOKE_WINDOW_MINUTES: ${env.X402_SMOKE_WINDOW_MINUTES}`,
    );
  }
  if (
    !Number.isInteger(overdueGraceMinutes) ||
    overdueGraceMinutes < 0 ||
    overdueGraceMinutes > 120
  ) {
    throw new Error(
      `invalid X402_SMOKE_OVERDUE_GRACE_MINUTES: ${env.X402_SMOKE_OVERDUE_GRACE_MINUTES}`,
    );
  }

  return {
    minute,
    intervalHours,
    windowMinutes,
    overdueGraceMinutes,
  };
}

function targetName(targetUrl) {
  try {
    const url = new URL(targetUrl);
    const segments = url.pathname.split("/").filter(Boolean);
    return segments.at(-1) || "target";
  } catch {
    return "target";
  }
}

function classifyPaidFailure(status, body) {
  const error = body?.error || null;
  const context = error?.details?.context || null;

  if (context?.verify) {
    return {
      code: "verify_failed",
      reason:
        context.verify.invalidReason ||
        error?.message ||
        "x402 verification failed",
      context: context.verify,
    };
  }

  if (context?.settle) {
    return {
      code: "settle_failed",
      reason:
        context.settle.errorReason ||
        error?.message ||
        "x402 settlement failed",
      context: context.settle,
    };
  }

  if (status === 402) {
    return {
      code: "payment_rejected",
      reason: error?.message || "x402 payment rejected",
      context,
    };
  }

  return {
    code: "paid_request_failed",
    reason: error?.message || `unexpected paid status ${status}`,
    context,
  };
}

function sumMicrousdc(values) {
  return values.reduce((total, value) => total + BigInt(value || "0"), 0n);
}

export function shouldRunScheduledSmoke(now, env = process.env) {
  if (!isEnabled(env.X402_SMOKE_ENABLED, false)) {
    return false;
  }

  const { minute, intervalHours, windowMinutes } = readScheduleConfig(env);
  const currentMinute = now.getUTCMinutes();
  const inWindow =
    currentMinute >= minute &&
    currentMinute < Math.min(minute + windowMinutes, 60);
  return inWindow && now.getUTCHours() % intervalHours === 0;
}

export function scheduledSmokeOverdueMs(env = process.env) {
  const { intervalHours, windowMinutes, overdueGraceMinutes } =
    readScheduleConfig(env);
  return (intervalHours * 60 + windowMinutes + overdueGraceMinutes) * 60_000;
}

async function readUsdcBalance(publicClient, parseAbi, token, wallet) {
  const erc20Abi = parseAbi([
    "function balanceOf(address owner) view returns (uint256)",
  ]);
  return publicClient.readContract({
    address: token,
    abi: erc20Abi,
    functionName: "balanceOf",
    args: [wallet],
  });
}

async function waitForSettledBalance(
  publicClient,
  parseAbi,
  usdcAddress,
  wallet,
  timeoutMs,
  balanceBefore,
) {
  let balanceAfter = await readUsdcBalance(
    publicClient,
    parseAbi,
    usdcAddress,
    wallet,
  );

  if (balanceAfter < balanceBefore) {
    return balanceAfter;
  }

  const deadline = Date.now() + Math.min(timeoutMs, 10_000);
  while (Date.now() < deadline) {
    await sleep(1_000);
    balanceAfter = await readUsdcBalance(
      publicClient,
      parseAbi,
      usdcAddress,
      wallet,
    );
    if (balanceAfter < balanceBefore) {
      break;
    }
  }

  return balanceAfter;
}

async function runSmokeTarget({
  account,
  balanceBefore,
  httpClient,
  parseAbi,
  publicClient,
  targetUrl,
  timeoutMs,
  usdcAddress,
}) {
  const name = targetName(targetUrl);
  const first = await fetch(targetUrl, {
    headers: { accept: "application/json" },
    signal: AbortSignal.timeout(timeoutMs),
  });
  const firstText = await first.text();
  const firstBody = parseJson(firstText);

  if (first.status !== 402) {
    throw new X402SmokeError(
      "quote_unexpected_status",
      `${name} quote expected 402, got ${first.status}`,
      {
        targetName: name,
        targetUrl,
        status: first.status,
        responseText: firstText.slice(0, 200),
      },
    );
  }

  if (!firstBody) {
    throw new X402SmokeError(
      "quote_invalid_json",
      `${name} quote response was not valid JSON`,
      {
        targetName: name,
        targetUrl,
      },
    );
  }

  let paymentRequired;
  try {
    paymentRequired = httpClient.getPaymentRequiredResponse(
      (headerName) => first.headers.get(headerName),
      firstBody,
    );
  } catch (error) {
    throw new X402SmokeError(
      "quote_invalid_payload",
      `${name} quote payload was invalid: ${error.message}`,
      {
        targetName: name,
        targetUrl,
        responseBody: firstBody,
      },
    );
  }

  let paymentPayload;
  try {
    paymentPayload = await httpClient.createPaymentPayload(paymentRequired);
  } catch (error) {
    throw new X402SmokeError(
      "payment_payload_failed",
      `${name} payment payload failed: ${error.message}`,
      {
        targetName: name,
        targetUrl,
      },
    );
  }

  const acceptedMicrousdc = paymentRequired.accepts?.[0]?.amount || "0";
  const second = await fetch(targetUrl, {
    headers: {
      accept: "application/json",
      ...httpClient.encodePaymentSignatureHeader(paymentPayload),
    },
    signal: AbortSignal.timeout(timeoutMs),
  });
  const secondText = await second.text();
  const secondBody = parseJson(secondText);

  if (second.status !== 200) {
    const failure = classifyPaidFailure(second.status, secondBody);
    throw new X402SmokeError(
      failure.code,
      `${name} x402 smoke failed: ${failure.reason}`,
      {
        targetName: name,
        targetUrl,
        status: second.status,
        responseBody: secondBody,
        responseText: secondText.slice(0, 200),
        failureContext: failure.context || null,
        acceptedMicrousdc,
      },
    );
  }

  if (!secondBody) {
    throw new X402SmokeError(
      "paid_invalid_json",
      `${name} paid response was not valid JSON`,
      {
        targetName: name,
        targetUrl,
      },
    );
  }

  let settlement;
  try {
    settlement = httpClient.getPaymentSettleResponse((headerName) =>
      second.headers.get(headerName),
    );
  } catch (error) {
    throw new X402SmokeError(
      "settlement_header_invalid",
      `${name} settlement header was invalid: ${error.message}`,
      {
        targetName: name,
        targetUrl,
      },
    );
  }

  if (!settlement) {
    throw new X402SmokeError(
      "settlement_header_missing",
      `${name} paid response did not include PAYMENT-RESPONSE`,
      {
        targetName: name,
        targetUrl,
      },
    );
  }

  if (!settlement.success) {
    throw new X402SmokeError(
      "settlement_header_failed",
      `${name} settlement header indicated failure`,
      {
        targetName: name,
        targetUrl,
        settlement,
      },
    );
  }

  if (
    settlement.payer &&
    settlement.payer.toLowerCase() !== account.address.toLowerCase()
  ) {
    throw new X402SmokeError(
      "unexpected_payer",
      `${name} settlement payer mismatch: ${settlement.payer}`,
      {
        targetName: name,
        targetUrl,
        settlement,
      },
    );
  }

  if (settlement.transaction) {
    await publicClient.waitForTransactionReceipt({
      hash: settlement.transaction,
      timeout: timeoutMs,
    });
  }

  const balanceAfter = await waitForSettledBalance(
    publicClient,
    parseAbi,
    usdcAddress,
    account.address,
    timeoutMs,
    balanceBefore,
  );
  const chargedMicrousdc =
    balanceBefore > balanceAfter
      ? (balanceBefore - balanceAfter).toString()
      : "0";

  return {
    targetName: name,
    targetUrl,
    firstStatus: first.status,
    secondStatus: second.status,
    paymentResponseHeaderPresent: second.headers.has("payment-response"),
    acceptedMicrousdc,
    balanceBeforeMicrousdc: balanceBefore.toString(),
    balanceAfterMicrousdc: balanceAfter.toString(),
    chargedMicrousdc,
    settlement,
  };
}

export async function runX402Smoke(env = process.env) {
  if (!isEnabled(env.X402_SMOKE_ENABLED, false)) {
    return { ok: true, skipped: true, reason: "x402 smoke disabled" };
  }

  const privateKey = String(env.X402_SMOKE_PAYER_PRIVATE_KEY || "").trim();
  if (!privateKey) {
    throw new Error("X402_SMOKE_PAYER_PRIVATE_KEY is required");
  }

  const targetUrls = buildTargetUrls(env);
  if (targetUrls.length === 0) {
    throw new Error("x402 smoke has no configured targets");
  }

  const timeoutMs = Number(env.X402_SMOKE_TIMEOUT_MS || "30000");
  const minUsdc = parseUsdc(env.X402_SMOKE_MIN_USDC || "1");
  const lowBalanceThreshold = parseUsdc(
    env.X402_SMOKE_LOW_BALANCE_USDC || DEFAULT_LOW_BALANCE_USDC,
  );
  const rpcUrl = String(
    env.X402_SMOKE_RPC_URL || env.BASE_RPC_URL || DEFAULT_RPC_URL,
  ).trim();
  const usdcAddress = String(
    env.X402_SMOKE_USDC_ADDRESS || DEFAULT_USDC_ADDRESS,
  ).trim();

  const [
    { x402Client },
    { x402HTTPClient },
    { ExactEvmScheme },
    {
      createPublicClient,
      createWalletClient,
      formatUnits,
      http,
      parseAbi,
      publicActions,
    },
    { privateKeyToAccount },
    { base },
  ] = await Promise.all([
    import("@x402/core/client"),
    import("@x402/core/http"),
    import("@x402/evm"),
    import("viem"),
    import("viem/accounts"),
    import("viem/chains"),
  ]);

  const account = privateKeyToAccount(privateKey);
  const publicClient = createPublicClient({
    chain: base,
    transport: http(rpcUrl),
  });
  const walletClient = createWalletClient({
    account,
    chain: base,
    transport: http(rpcUrl),
  }).extend(publicActions);
  const evmSigner = {
    address: account.address,
    signTypedData: (args) => walletClient.signTypedData(args),
    readContract: (args) => publicClient.readContract(args),
    getTransactionCount: (args) => publicClient.getTransactionCount(args),
    estimateFeesPerGas: () => publicClient.estimateFeesPerGas(),
    signTransaction: (args) => walletClient.signTransaction(args),
  };
  const httpClient = new x402HTTPClient(
    new x402Client().register("eip155:*", new ExactEvmScheme(evmSigner)),
  );

  const balanceBefore = await readUsdcBalance(
    publicClient,
    parseAbi,
    usdcAddress,
    account.address,
  );
  if (balanceBefore < minUsdc) {
    throw new X402SmokeError(
      "wallet_underfunded",
      `x402 smoke wallet underfunded: ${formatUnits(balanceBefore, USDC_DECIMALS)} USDC < ${formatUnits(minUsdc, USDC_DECIMALS)} USDC`,
      {
        payer: account.address,
        balanceBeforeMicrousdc: balanceBefore.toString(),
        minimumMicrousdc: minUsdc.toString(),
      },
    );
  }

  const routes = [];
  let balanceCursor = balanceBefore;
  for (const targetUrl of targetUrls) {
    try {
      const route = await runSmokeTarget({
        account,
        balanceBefore: balanceCursor,
        httpClient,
        parseAbi,
        publicClient,
        targetUrl,
        timeoutMs,
        usdcAddress,
      });
      routes.push(route);
      balanceCursor = BigInt(route.balanceAfterMicrousdc);
    } catch (error) {
      const failure =
        error instanceof X402SmokeError
          ? error
          : new X402SmokeError("smoke_failed", error.message, {
              targetName: targetName(targetUrl),
              targetUrl,
            });
      failure.payer = account.address;
      failure.routeResults = routes;
      failure.balanceBeforeMicrousdc = balanceBefore.toString();
      failure.balanceAfterMicrousdc = balanceCursor.toString();
      failure.lowBalanceThresholdMicrousdc = lowBalanceThreshold.toString();
      throw failure;
    }
  }

  const chargedMicrousdc =
    balanceBefore > balanceCursor
      ? (balanceBefore - balanceCursor).toString()
      : "0";

  return {
    ok: true,
    payer: account.address,
    routeCount: routes.length,
    routes,
    balanceBeforeMicrousdc: balanceBefore.toString(),
    balanceBeforeUsdc: formatUnits(balanceBefore, USDC_DECIMALS),
    balanceAfterMicrousdc: balanceCursor.toString(),
    balanceAfterUsdc: formatUnits(balanceCursor, USDC_DECIMALS),
    acceptedMicrousdc: sumMicrousdc(
      routes.map((route) => route.acceptedMicrousdc),
    ).toString(),
    chargedMicrousdc,
    chargedUsdc: formatUnits(BigInt(chargedMicrousdc), USDC_DECIMALS),
    lowBalanceThresholdMicrousdc: lowBalanceThreshold.toString(),
    lowBalanceThresholdUsdc: formatUnits(lowBalanceThreshold, USDC_DECIMALS),
    lowBalanceTriggered: balanceCursor < lowBalanceThreshold,
  };
}

async function main() {
  try {
    const result = await runX402Smoke(process.env);
    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    const message = `[relay44 ALERT] x402 smoke failed: ${error.message}`;
    console.error(
      JSON.stringify(
        {
          ok: false,
          code: error.code || "smoke_failed",
          message,
          details: error,
        },
        null,
        2,
      ),
    );
    await sendAlert(message, process.env);
    process.exit(1);
  }
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  main();
}
