#!/usr/bin/env node

// Oracle Keeper — resolves oracle-backed markets past their closeTime.
// Follows the base-agent-operator.mjs pattern: SIWE login → tick plan → execute tx → report.

import {
  createPublicClient,
  createWalletClient,
  fallback,
  formatEther,
  http,
} from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { base } from "viem/chains";

const enabled = isEnabled(process.env.ORACLE_KEEPER_ENABLED, false);
const apiBase = normalizeApiBase(
  process.env.ORACLE_KEEPER_API_URL || "http://localhost:8080/v1",
);
const apiOrigin = apiBase.replace(/\/v1$/, "");
const siweDomain = (
  process.env.ORACLE_KEEPER_SIWE_DOMAIN ||
  process.env.SIWE_DOMAIN ||
  "localhost:3000"
).trim();
const chainId = Number(process.env.BASE_CHAIN_ID || 8453);
const limit = Math.max(Number(process.env.ORACLE_KEEPER_LIMIT || 50), 1);
const dryRun = isEnabled(process.env.ORACLE_KEEPER_DRY_RUN, false);
const minBalanceEth = Number(
  process.env.ORACLE_KEEPER_MIN_BALANCE_ETH || 0.001,
);

function envOrThrow(key) {
  const value = String(process.env[key] || "").trim();
  if (!value) {
    throw new Error(`${key} is required`);
  }
  return value;
}

function isEnabled(raw, fallbackValue) {
  if (raw == null || raw === "") {
    return fallbackValue;
  }
  return ["1", "true", "yes", "on"].includes(String(raw).trim().toLowerCase());
}

function normalizeApiBase(raw) {
  const trimmed = String(raw || "")
    .trim()
    .replace(/\/$/, "");
  if (!trimmed) {
    return "http://localhost:8080/v1";
  }
  const withScheme =
    trimmed.startsWith("http://") || trimmed.startsWith("https://")
      ? trimmed
      : `http://${trimmed}`;
  return withScheme.endsWith("/v1") ? withScheme : `${withScheme}/v1`;
}

function createTransport() {
  const primary = envOrThrow("BASE_RPC_URL");
  const fallbacks = String(process.env.BASE_RPC_FALLBACK_URLS || "")
    .split(",")
    .map((entry) => entry.trim())
    .filter(Boolean)
    .filter((entry) => entry !== primary);
  const urls = [primary, ...fallbacks];

  if (urls.length === 1) {
    return http(urls[0], { timeout: 15_000 });
  }

  return fallback(
    urls.map((url) => http(url, { timeout: 15_000 })),
    { rank: false, retryCount: 1 },
  );
}

function buildHeaders(token) {
  const headers = { "content-type": "application/json" };
  if (token) {
    headers.authorization = `Bearer ${token}`;
  }
  const internalKey = (process.env.INTERNAL_SERVICE_KEY || "").trim();
  if (internalKey) {
    headers["x-internal-service-key"] = internalKey;
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

async function loginAdmin(account) {
  const noncePayload = await fetchJson(`${apiBase}/auth/siwe/nonce`);
  const nonce = noncePayload?.nonce;

  if (!nonce) {
    throw new Error("missing SIWE nonce");
  }

  const issuedAt = new Date().toISOString();
  const message = `${siweDomain} wants you to sign in with your Ethereum account:\n${account.address}\n\nSign in to relay44 oracle keeper\n\nURI: ${apiOrigin}\nVersion: 1\nChain ID: ${chainId}\nNonce: ${nonce}\nIssued At: ${issuedAt}`;
  const signature = await account.signMessage({ message });
  const tokens = await fetchJson(`${apiBase}/auth/siwe/login`, {
    method: "POST",
    headers: buildHeaders(),
    body: JSON.stringify({ wallet: account.address, message, signature }),
  });

  if (!tokens?.access_token) {
    throw new Error("missing access token");
  }

  return tokens.access_token;
}

async function apiPost(pathname, token, body = {}) {
  return fetchJson(`${apiBase}${pathname}`, {
    method: "POST",
    headers: buildHeaders(token),
    body: JSON.stringify(body),
  });
}

const GAS_PRICE_ORACLE = "0x420000000000000000000000000000000000000F";
const L1_FEE_ABI = [
  {
    name: "getL1FeeUpperBound",
    type: "function",
    stateMutability: "view",
    inputs: [{ name: "_unsignedTxSize", type: "uint256" }],
    outputs: [{ name: "", type: "uint256" }],
  },
];
const TYPICAL_TX_SIZE = 200;

async function estimateL1Fee(publicClient) {
  try {
    return await publicClient.readContract({
      address: GAS_PRICE_ORACLE,
      abi: L1_FEE_ABI,
      functionName: "getL1FeeUpperBound",
      args: [BigInt(TYPICAL_TX_SIZE)],
    });
  } catch (err) {
    console.warn("L1 fee estimation failed:", err?.message);
    return 0n;
  }
}

async function ensureOperatorBalance(runtime) {
  const [balance, l1Fee] = await Promise.all([
    runtime.publicClient.getBalance({ address: runtime.account.address }),
    estimateL1Fee(runtime.publicClient),
  ]);
  const balanceEth = formatEther(balance);
  const minBalanceWei = BigInt(Math.ceil(minBalanceEth * 1e18));
  const requiredWei = minBalanceWei > l1Fee ? minBalanceWei : l1Fee * 2n;

  if (balance < requiredWei) {
    return {
      ok: false,
      skipped: true,
      reason: "oracle keeper wallet underfunded",
      operator: runtime.account.address,
      balanceEth,
      minBalanceEth,
      estimatedL1FeeEth: formatEther(l1Fee),
    };
  }

  return { ok: true, operator: runtime.account.address, balanceEth, minBalanceEth };
}

async function fetchTickPlan(token) {
  return apiPost("/evm/oracle/keeper/tick", token, { limit });
}

async function postReport(token, report) {
  let lastError = null;
  for (let attempt = 1; attempt <= 3; attempt += 1) {
    try {
      return await apiPost("/evm/oracle/keeper/report", token, report);
    } catch (error) {
      lastError = error;
      if (attempt < 3) {
        await new Promise((resolve) => setTimeout(resolve, attempt * 1_000));
      }
    }
  }
  throw lastError;
}

async function executeAction(runtime, action, accessToken) {
  const prepared = action.preparedWrite || {};
  if (Number(prepared.chainId || 0) !== chainId) {
    throw new Error(
      `chain mismatch for oracle action marketId=${action.marketId}`,
    );
  }

  let hash = null;

  try {
    hash = await runtime.walletClient.sendTransaction({
      account: runtime.account,
      chain: runtime.chain,
      to: prepared.to,
      data: prepared.data,
      value: BigInt(prepared.value || "0x0"),
    });
    const receipt = await runtime.publicClient.waitForTransactionReceipt({ hash });
    const success = receipt.status === "success";

    await postReport(accessToken, {
      marketId: action.marketId,
      kind: action.kind,
      txHash: hash,
      success,
      error: success ? null : `transaction reverted for ${action.kind}`,
    });

    return {
      marketId: action.marketId,
      kind: action.kind,
      txHash: hash,
      success,
      status: receipt.status,
    };
  } catch (error) {
    if (hash) {
      try {
        await postReport(accessToken, {
          marketId: action.marketId,
          kind: action.kind,
          txHash: hash,
          success: false,
          error: error instanceof Error ? error.message : String(error),
        });
      } catch (reportError) {
        throw new Error(
          `tx ${hash} failed for ${action.kind} and report failed: ${
            reportError instanceof Error ? reportError.message : String(reportError)
          }`,
        );
      }
    }
    throw error;
  }
}

async function main() {
  if (!enabled) {
    console.log(
      JSON.stringify(
        { ok: true, skipped: true, reason: "oracle keeper disabled" },
        null,
        2,
      ),
    );
    return;
  }

  const privateKey = envOrThrow("ORACLE_KEEPER_PRIVATE_KEY");
  const account = privateKeyToAccount(privateKey);
  const transport = createTransport();
  const chain = { ...base, id: chainId };
  const runtime = {
    account,
    chain,
    publicClient: createPublicClient({ chain, transport }),
    walletClient: createWalletClient({ account, chain, transport }),
  };

  const balanceCheck = await ensureOperatorBalance(runtime);
  if (!balanceCheck.ok) {
    console.log(JSON.stringify(balanceCheck, null, 2));
    return;
  }

  const accessToken = await loginAdmin(runtime.account);
  const startedAt = new Date().toISOString();
  const plan = await fetchTickPlan(accessToken);
  const actions = Array.isArray(plan?.actions) ? plan.actions : [];

  if (dryRun) {
    console.log(
      JSON.stringify(
        {
          ok: true,
          startedAt,
          operator: runtime.account.address,
          balanceEth: balanceCheck.balanceEth,
          minBalanceEth,
          scanned: plan?.scanned ?? 0,
          dryRun: true,
          actions: actions.map((action) => ({
            marketId: action.marketId,
            kind: action.kind,
            feedType: action.feedType || null,
            feedAddress: action.feedAddress || null,
          })),
        },
        null,
        2,
      ),
    );
    return;
  }

  const executions = [];
  const failures = [];

  for (const action of actions) {
    try {
      executions.push(await executeAction(runtime, action, accessToken));
    } catch (error) {
      failures.push({
        marketId: action.marketId,
        kind: action.kind,
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  console.log(
    JSON.stringify(
      {
        ok: failures.length === 0,
        startedAt,
        operator: runtime.account.address,
        balanceEth: balanceCheck.balanceEth,
        minBalanceEth,
        dryRun: false,
        scanned: plan?.scanned ?? 0,
        planned: actions.length,
        executed: executions.length,
        failures,
        executions,
      },
      null,
      2,
    ),
  );

  if (failures.length > 0) {
    process.exit(1);
  }
}

main().catch((error) => {
  console.error(
    JSON.stringify(
      {
        ok: false,
        message: error.message,
        status: error.status || null,
        details: error.payload || null,
      },
      null,
      2,
    ),
  );
  process.exit(1);
});
