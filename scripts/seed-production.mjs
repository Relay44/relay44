#!/usr/bin/env node

/**
 * Orchestrates full production data seeding in the correct dependency order:
 *
 *   1. Bootstrap backfill (markets + liquidity)
 *   2. Paper cohort seed (creates paper agents)
 *   3. Alpha cohort seed (creates wallet-follow agents)
 *   4. Leaderboard compute (builds snapshots from any existing trades)
 *
 * Env vars:
 *   API_URL                         - API base (default: http://localhost:8080/v1)
 *   ADMIN_PRIVATE_KEY               - admin wallet private key (falls back to PAPER_COHORT_ADMIN_PRIVATE_KEY)
 *   SIWE_DOMAIN                     - SIWE domain (default: localhost:3000)
 *   SKIP_BOOTSTRAP                  - skip market backfill step
 *   SKIP_PAPER_COHORT               - skip paper cohort seeding
 *   SKIP_ALPHA_COHORT               - skip alpha cohort seeding
 *   SKIP_LEADERBOARD                - skip leaderboard compute
 */

import { privateKeyToAccount } from "viem/accounts";

function normalizeApiBase(raw) {
  const trimmed = String(raw || "").trim().replace(/\/$/, "");
  if (!trimmed) return "http://localhost:8080/v1";
  const withScheme =
    trimmed.startsWith("http://") || trimmed.startsWith("https://")
      ? trimmed
      : `http://${trimmed}`;
  return withScheme.endsWith("/v1") ? withScheme : `${withScheme}/v1`;
}

const apiBase = normalizeApiBase(
  process.env.API_URL ||
    process.env.PAPER_COHORT_API_URL ||
    process.env.BASE_AGENT_OPERATOR_API_URL,
);
const apiOrigin = apiBase.replace(/\/v1$/, "");
const siweDomain = (
  process.env.SIWE_DOMAIN ||
  process.env.PAPER_COHORT_SIWE_DOMAIN ||
  "localhost:3000"
).trim();
const chainId = Number(process.env.BASE_CHAIN_ID || 8453);

const FETCH_TIMEOUT_MS = 60_000;
const RETRYABLE_STATUSES = new Set([429, 502, 503, 504]);

function buildHeaders(token) {
  const headers = { "content-type": "application/json" };
  if (token) headers.authorization = `Bearer ${token}`;
  const country = (process.env.PAPER_RUNNER_COUNTRY_CODE || "").trim();
  if (country) headers["x-country-code"] = country;
  const internalKey = (process.env.INTERNAL_SERVICE_KEY || "").trim();
  if (internalKey) headers["x-internal-service-key"] = internalKey;
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
    const err = new Error(message);
    err.status = response.status;
    err.payload = payload;
    throw err;
  }
  return payload;
}

async function fetchWithRetry(url, init = {}, maxRetries = 3) {
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      return await fetchJson(url, init);
    } catch (err) {
      const isTimeout =
        err.name === "TimeoutError" || err.name === "AbortError";
      const retryable = isTimeout || RETRYABLE_STATUSES.has(err.status);
      if (!retryable || attempt === maxRetries) throw err;
      await new Promise((r) => setTimeout(r, 1000 * Math.pow(2, attempt)));
    }
  }
}

async function apiGet(pathname, token) {
  return fetchWithRetry(`${apiBase}${pathname}`, {
    method: "GET",
    headers: buildHeaders(token),
  });
}

async function apiPost(pathname, token, body = {}) {
  return fetchWithRetry(`${apiBase}${pathname}`, {
    method: "POST",
    headers: buildHeaders(token),
    body: JSON.stringify(body),
  });
}

async function loginAdmin() {
  const privateKey =
    process.env.ADMIN_PRIVATE_KEY ||
    process.env.PAPER_COHORT_ADMIN_PRIVATE_KEY ||
    process.env.BASE_AGENT_OPERATOR_PRIVATE_KEY ||
    process.env.ALPHA_COHORT_ADMIN_PRIVATE_KEY;
  if (!privateKey) {
    throw new Error(
      "ADMIN_PRIVATE_KEY (or PAPER_COHORT_ADMIN_PRIVATE_KEY) required",
    );
  }
  const account = privateKeyToAccount(privateKey);
  const noncePayload = await fetchWithRetry(`${apiBase}/auth/siwe/nonce`);
  const nonce = noncePayload?.nonce;
  if (!nonce) throw new Error("missing SIWE nonce");

  const issuedAt = new Date().toISOString();
  const message = `${siweDomain} wants you to sign in with your Ethereum account:\n${account.address}\n\nSign in to relay44 production seed\n\nURI: ${apiOrigin}\nVersion: 1\nChain ID: ${chainId}\nNonce: ${nonce}\nIssued At: ${issuedAt}`;
  const signature = await account.signMessage({ message });
  const tokens = await fetchWithRetry(`${apiBase}/auth/siwe/login`, {
    method: "POST",
    headers: buildHeaders(),
    body: JSON.stringify({ wallet: account.address, message, signature }),
  });
  if (!tokens?.access_token) throw new Error("missing access token");
  return { account, accessToken: tokens.access_token };
}

function skip(key) {
  const val = (process.env[key] || "").trim().toLowerCase();
  return ["1", "true", "yes"].includes(val);
}

function log(step, msg) {
  console.log(`[${step}] ${msg}`);
}

async function stepBootstrap(token) {
  if (skip("SKIP_BOOTSTRAP")) {
    log("bootstrap", "skipped");
    return { skipped: true };
  }
  log("bootstrap", "running market backfill...");
  const result = await apiPost("/evm/bootstrap/admin/backfill", token);
  log("bootstrap", `done — ${JSON.stringify(result)}`);
  return result;
}

async function stepPaperCohort(token) {
  if (skip("SKIP_PAPER_COHORT")) {
    log("paper-cohort", "skipped");
    return { skipped: true };
  }
  log("paper-cohort", "checking existing agents...");
  const existing = await apiGet(
    "/external/agents?limit=200&offset=0",
    token,
  );
  const agents = existing?.agents || [];
  const paperCount = agents.filter(
    (a) => a.name?.startsWith("paper-") && a.active,
  ).length;

  if (paperCount >= 10) {
    log("paper-cohort", `${paperCount} active paper agents already exist`);
    return { existingPaperAgents: paperCount, created: 0 };
  }

  log("paper-cohort", "fetching executable markets...");
  const [limitless, polymarket] = await Promise.all([
    apiGet(
      "/evm/markets?source=limitless&tradable=agent&limit=25&offset=0&includeLowLiquidity=true",
      token,
    ).catch(() => ({ markets: [] })),
    apiGet(
      "/evm/markets?source=polymarket&tradable=agent&limit=25&offset=0&includeLowLiquidity=true",
      token,
    ).catch(() => ({ markets: [] })),
  ]);

  const markets = [...(limitless?.markets || []), ...(polymarket?.markets || [])]
    .filter(
      (m) =>
        (m.isExternal ?? m.is_external) &&
        (m.executionAgents ?? m.execution_agents),
    );

  if (!markets.length) {
    log("paper-cohort", "no executable markets found — skipping agent creation");
    return { markets: 0, created: 0 };
  }

  const strategies = ["momentum", "mean-revert", "market-maker"];
  const targetCount = 15;
  let created = 0;

  for (let i = paperCount; i < targetCount && markets.length > 0; i++) {
    const strategy = strategies[i % strategies.length];
    const market = markets[i % markets.length];
    const yes = findProbability(market, "yes");
    const outcome = yes >= 0.5 ? "yes" : "no";

    const spec = {
      name: `paper-${strategy}-${String(i + 1).padStart(2, "0")}`,
      provider: market.provider,
      marketId: market.id,
      outcome,
      side: "buy",
      price: clamp(findProbability(market, outcome) + 0.01),
      quantity: strategy === "market-maker" ? 2 : strategy === "momentum" ? 4 : 3,
      cadenceSeconds: strategy === "market-maker" ? 180 : strategy === "momentum" ? 300 : 420,
      strategy,
      active: true,
    };

    try {
      await apiPost("/external/agents", token, spec);
      created++;
    } catch (err) {
      if (err.status === 409) continue; // already exists
      throw err;
    }
  }

  log("paper-cohort", `created ${created} paper agents across ${markets.length} markets`);
  return { markets: markets.length, created };
}

async function stepAlphaCohort(token) {
  if (skip("SKIP_ALPHA_COHORT")) {
    log("alpha-cohort", "skipped");
    return { skipped: true };
  }
  log("alpha-cohort", "fetching research wallets...");
  const payload = await apiGet("/external/research/wallets?limit=20", token).catch(() => null);
  const wallets = payload?.items || payload?.wallets || payload?.data || [];
  const eligible = wallets.filter(
    (w) =>
      (w.composite_score ?? w.compositeScore ?? 0) >= 0.55 &&
      (w.trade_count ?? w.tradeCount ?? 0) >= 20,
  );

  if (!eligible.length) {
    log("alpha-cohort", "no eligible research wallets found — skipping");
    return { eligible: 0, created: 0 };
  }

  const markets = await apiGet(
    "/evm/markets?source=polymarket&tradable=agent&limit=25&offset=0&includeLowLiquidity=false",
    token,
  ).catch(() => ({ markets: [] }));
  const execMarkets = (markets?.markets || []).filter(
    (m) =>
      (m.isExternal ?? m.is_external) &&
      (m.executionAgents ?? m.execution_agents),
  );

  if (!execMarkets.length) {
    log("alpha-cohort", "no executable markets — skipping");
    return { eligible: eligible.length, markets: 0, created: 0 };
  }

  let created = 0;
  const count = Math.min(5, eligible.length);
  for (let i = 0; i < count; i++) {
    const wallet = eligible[i];
    const market = execMarkets[i % execMarkets.length];
    const yes = findProbability(market, "yes");
    const outcome = yes >= 0.5 ? "yes" : "no";

    const spec = {
      name: `alpha-wfv2-${String(i + 1).padStart(2, "0")}`,
      provider: market.provider,
      marketId: market.id,
      outcome,
      side: "buy",
      price: clamp(findProbability(market, outcome) + 0.01),
      quantity: 5,
      cadenceSeconds: 120,
      strategy: "wallet-follow-v2",
      strategyParams: {
        targetWallet: wallet.wallet,
        followRatio: 0.6,
        walletScoreMin: 0.55,
        walletScore: wallet.composite_score ?? wallet.compositeScore ?? 0.6,
      },
      active: true,
    };

    try {
      await apiPost("/external/agents", token, spec);
      created++;
    } catch (err) {
      if (err.status === 409) continue;
      throw err;
    }
  }

  log("alpha-cohort", `created ${created} wallet-follow agents`);
  return { eligible: eligible.length, markets: execMarkets.length, created };
}

async function stepLeaderboard(token) {
  if (skip("SKIP_LEADERBOARD")) {
    log("leaderboard", "skipped");
    return { skipped: true };
  }
  log("leaderboard", "computing snapshots...");
  const result = await apiPost("/leaderboard/compute", token);
  log("leaderboard", `done — ${JSON.stringify(result)}`);
  return result;
}

async function stepVerify(token) {
  log("verify", "checking endpoint responses...");
  const checks = {};

  const lb = await apiGet("/leaderboard?period=weekly&metric=pnl&limit=5", token).catch(() => null);
  checks.leaderboard = (lb?.entries?.length || 0) > 0 ? "real data" : "empty (mock fallback will trigger)";

  const agents = await apiGet("/external/agents/public?limit=5", token).catch(() => null);
  checks.agents = (agents?.agents?.length || 0) > 0 ? `${agents.agents.length} agents` : "empty";

  const perf = await apiGet("/external/agents/public/performance", token).catch(() => null);
  checks.agentPerformance = perf?.totals ? `${perf.totals.agents} agents tracked` : "empty";

  log("verify", JSON.stringify(checks, null, 2));
  return checks;
}

function findProbability(market, outcome) {
  const match = (market.outcomes || []).find(
    (e) => String(e.label || "").trim().toLowerCase() === outcome,
  );
  return typeof match?.probability === "number" ? clamp(match.probability) : outcome === "no" ? 0.45 : 0.55;
}

function clamp(value) {
  const n = Number(value);
  if (!Number.isFinite(n)) return 0.5;
  return Math.max(0.02, Math.min(0.98, Number(n.toFixed(4))));
}

async function main() {
  const startMs = Date.now();
  console.log(`\nrelay44 production seed — ${apiBase}\n`);

  const { account, accessToken } = await loginAdmin();
  log("auth", `logged in as ${account.address}`);

  const results = {};
  results.bootstrap = await stepBootstrap(accessToken);
  results.paperCohort = await stepPaperCohort(accessToken);
  results.alphaCohort = await stepAlphaCohort(accessToken);
  results.leaderboard = await stepLeaderboard(accessToken);
  results.verify = await stepVerify(accessToken);

  const durationMs = Date.now() - startMs;
  console.log(
    `\n${JSON.stringify({ ok: true, durationMs, ...results }, null, 2)}`,
  );
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
