import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import { privateKeyToAccount } from "viem/accounts";

const rawApiBase = (
  process.env.PAPER_COHORT_API_URL || "http://localhost:8080/v1"
).trim();
export const apiBase = rawApiBase.endsWith("/v1")
  ? rawApiBase.replace(/\/$/, "")
  : `${rawApiBase.replace(/\/$/, "")}/v1`;
export const apiOrigin = apiBase.replace(/\/v1$/, "");
export const siweDomain = (
  process.env.PAPER_COHORT_SIWE_DOMAIN ||
  process.env.SIWE_DOMAIN ||
  "localhost:3000"
).trim();
export const chainId = Number(
  process.env.PAPER_COHORT_CHAIN_ID || process.env.BASE_CHAIN_ID || 8453,
);
export const agentCount = Number(process.env.PAPER_COHORT_AGENT_COUNT || 60);
export const runnerCountryCode = (process.env.PAPER_RUNNER_COUNTRY_CODE || "")
  .trim()
  .toUpperCase();

export function envOrThrow(key) {
  const value = process.env[key]?.trim();
  if (!value) {
    throw new Error(`${key} is required`);
  }
  return value;
}

export function buildHeaders(token) {
  const headers = {
    "content-type": "application/json",
  };

  if (token) {
    headers.authorization = `Bearer ${token}`;
  }

  if (runnerCountryCode) {
    headers["x-country-code"] = runnerCountryCode;
  }

  return headers;
}

const FETCH_TIMEOUT_MS = 60_000;
const RETRYABLE_STATUSES = new Set([429, 502, 503, 504]);

export async function fetchJson(url, init = {}) {
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
      `${response.status} ${response.statusText}`;
    const err = new Error(message);
    err.status = response.status;
    err.payload = payload;
    throw err;
  }

  return payload;
}

export async function fetchWithRetry(url, init = {}, { maxRetries = 3, baseDelay = 1000 } = {}) {
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      return await fetchJson(url, init);
    } catch (err) {
      const retryable = RETRYABLE_STATUSES.has(err.status);
      if (!retryable || attempt === maxRetries) throw err;
      const delay = baseDelay * Math.pow(2, attempt);
      await new Promise((r) => setTimeout(r, delay));
    }
  }
}

export async function loginAdmin() {
  const privateKey = envOrThrow("PAPER_COHORT_ADMIN_PRIVATE_KEY");
  const account = privateKeyToAccount(privateKey);
  const noncePayload = await fetchJson(`${apiBase}/auth/siwe/nonce`);
  const nonce = noncePayload?.nonce;

  if (!nonce) {
    throw new Error("missing SIWE nonce");
  }

  const issuedAt = new Date().toISOString();
  const message = `${siweDomain} wants you to sign in with your Ethereum account:\n${account.address}\n\nSign in to relay44 paper cohort\n\nURI: ${apiOrigin}\nVersion: 1\nChain ID: ${chainId}\nNonce: ${nonce}\nIssued At: ${issuedAt}`;
  const signature = await account.signMessage({ message });
  const tokens = await fetchJson(`${apiBase}/auth/siwe/login`, {
    method: "POST",
    headers: buildHeaders(),
    body: JSON.stringify({
      wallet: account.address,
      message,
      signature,
    }),
  });

  if (!tokens?.access_token) {
    throw new Error("missing access token");
  }

  return {
    account,
    accessToken: tokens.access_token,
  };
}

export async function apiGet(pathname, token) {
  return fetchWithRetry(`${apiBase}${pathname}`, {
    method: "GET",
    headers: buildHeaders(token),
  });
}

export async function apiPost(pathname, token, body = {}) {
  return fetchWithRetry(`${apiBase}${pathname}`, {
    method: "POST",
    headers: buildHeaders(token),
    body: JSON.stringify(body),
  });
}

export async function apiPatch(pathname, token, body = {}) {
  return fetchWithRetry(`${apiBase}${pathname}`, {
    method: "PATCH",
    headers: buildHeaders(token),
    body: JSON.stringify(body),
  });
}

export async function listAgents(token) {
  const payload = await apiGet("/external/agents?limit=200&offset=0", token);
  return payload?.agents || [];
}

export async function listExecutableMarkets(token) {
  const [limitless, polymarket] = await Promise.all([
    apiGet(
      "/evm/markets?source=limitless&tradable=agent&limit=25&offset=0&includeLowLiquidity=true",
      token,
    ),
    apiGet(
      "/evm/markets?source=polymarket&tradable=agent&limit=25&offset=0&includeLowLiquidity=true",
      token,
    ),
  ]);

  return [...(limitless?.markets || []), ...(polymarket?.markets || [])]
    .map((market) => ({
      ...market,
      isExternal: market.isExternal ?? market.is_external ?? false,
      executionAgents:
        market.executionAgents ?? market.execution_agents ?? false,
      executionUsers: market.executionUsers ?? market.execution_users ?? false,
    }))
    .filter((market) => market.isExternal && market.executionAgents);
}

export function probabilityForOutcome(market, outcome) {
  const match = (market.outcomes || []).find(
    (entry) =>
      String(entry.label || "")
        .trim()
        .toLowerCase() === outcome,
  );

  if (typeof match?.probability === "number") {
    return clampPrice(match.probability);
  }

  return outcome === "no" ? 0.45 : 0.55;
}

export function clampPrice(value) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    return 0.5;
  }
  return Math.max(0.02, Math.min(0.98, Number(numeric.toFixed(4))));
}

export function buildAgentSpec(strategy, market, index) {
  const yes = probabilityForOutcome(market, "yes");
  const no = probabilityForOutcome(market, "no");
  const favoredOutcome = yes >= no ? "yes" : "no";
  const contrarianOutcome = favoredOutcome === "yes" ? "no" : "yes";
  const marketMakerOutcome = index % 2 === 0 ? "yes" : "no";

  if (strategy === "momentum") {
    return {
      name: `paper-momentum-${String(index + 1).padStart(2, "0")}`,
      provider: market.provider,
      marketId: market.id,
      outcome: favoredOutcome,
      side: "buy",
      price: clampPrice(probabilityForOutcome(market, favoredOutcome) + 0.01),
      quantity: 4,
      cadenceSeconds: 300,
      strategy,
      active: true,
    };
  }

  if (strategy === "mean-revert") {
    return {
      name: `paper-mean-revert-${String(index + 1).padStart(2, "0")}`,
      provider: market.provider,
      marketId: market.id,
      outcome: contrarianOutcome,
      side: "buy",
      price: clampPrice(
        probabilityForOutcome(market, contrarianOutcome) + 0.015,
      ),
      quantity: 3,
      cadenceSeconds: 420,
      strategy,
      active: true,
    };
  }

  return {
    name: `paper-market-maker-${String(index + 1).padStart(2, "0")}`,
    provider: market.provider,
    marketId: market.id,
    outcome: marketMakerOutcome,
    side: index % 2 === 0 ? "buy" : "sell",
    price: clampPrice(probabilityForOutcome(market, marketMakerOutcome)),
    quantity: 2,
    cadenceSeconds: 180,
    strategy,
    active: true,
  };
}

export function strategySequence(total = agentCount) {
  const sequence = [];
  for (let i = 0; i < total; i += 1) {
    if (i < total / 3) {
      sequence.push("momentum");
    } else if (i < (total / 3) * 2) {
      sequence.push("mean-revert");
    } else {
      sequence.push("market-maker");
    }
  }
  return sequence;
}

export async function ensureOutputDir() {
  const outputDir = path.join(process.cwd(), "tmp", "paper-cohort");
  await mkdir(outputDir, { recursive: true });
  return outputDir;
}

export async function writeOutputFile(filename, content) {
  const outputDir = await ensureOutputDir();
  const destination = path.join(outputDir, filename);
  await writeFile(destination, content);
  return destination;
}
