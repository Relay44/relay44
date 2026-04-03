#!/usr/bin/env node

/**
 * Seeds wallet_follow_v2 agents in private_alpha cohort for forward paper trading.
 * Uses top-scored wallets from the research endpoint.
 *
 * Env vars:
 *   PAPER_COHORT_ADMIN_PRIVATE_KEY  - admin wallet private key
 *   PAPER_COHORT_API_URL            - API base (default: http://localhost:8080/v1)
 *   ALPHA_AGENT_COUNT               - number of agents to seed (default: 5)
 *   ALPHA_MARKET_CATEGORY           - market category to target (default: crypto)
 */

import {
  apiBase,
  apiGet,
  apiPatch,
  apiPost,
  buildHeaders,
  clampPrice,
  fetchWithRetry,
  listAgents,
  probabilityForOutcome,
  siweDomain,
  apiOrigin,
  chainId,
} from "./paper-cohort-lib.mjs";
import { privateKeyToAccount } from "viem/accounts";

async function loginAlphaAdmin() {
  const privateKey =
    process.env.ALPHA_COHORT_ADMIN_PRIVATE_KEY ||
    process.env.PAPER_COHORT_ADMIN_PRIVATE_KEY;
  if (!privateKey) {
    throw new Error(
      "ALPHA_COHORT_ADMIN_PRIVATE_KEY or PAPER_COHORT_ADMIN_PRIVATE_KEY required",
    );
  }
  const account = privateKeyToAccount(privateKey);
  const noncePayload = await fetchWithRetry(`${apiBase}/auth/siwe/nonce`);
  const nonce = noncePayload?.nonce;
  if (!nonce) throw new Error("missing SIWE nonce");

  const issuedAt = new Date().toISOString();
  const message = `${siweDomain} wants you to sign in with your Ethereum account:\n${account.address}\n\nSign in to relay44 alpha cohort\n\nURI: ${apiOrigin}\nVersion: 1\nChain ID: ${chainId}\nNonce: ${nonce}\nIssued At: ${issuedAt}`;
  const signature = await account.signMessage({ message });
  const tokens = await fetchWithRetry(`${apiBase}/auth/siwe/login`, {
    method: "POST",
    headers: buildHeaders(),
    body: JSON.stringify({ wallet: account.address, message, signature }),
  });

  if (!tokens?.access_token) throw new Error("missing access token");
  return { account, accessToken: tokens.access_token };
}

const AGENT_COUNT = Number(process.env.ALPHA_AGENT_COUNT || 5);
const MARKET_CATEGORY = (process.env.ALPHA_MARKET_CATEGORY || "").toLowerCase();

async function fetchTopWallets(token) {
  const qs = MARKET_CATEGORY
    ? `marketCategory=${MARKET_CATEGORY}&limit=20`
    : "limit=20";
  const payload = await apiGet(`/external/research/wallets?${qs}`, token);
  const wallets = payload?.items || payload?.wallets || payload?.data || [];
  return wallets
    .filter((w) => (w.composite_score ?? w.compositeScore ?? 0) >= 0.55)
    .slice(0, AGENT_COUNT);
}

async function fetchPolymarkets(token) {
  const payload = await apiGet(
    "/evm/markets?source=polymarket&tradable=agent&limit=25&offset=0&includeLowLiquidity=false",
    token,
  );
  return (payload?.markets || [])
    .map((m) => ({
      ...m,
      isExternal: m.isExternal ?? m.is_external ?? false,
      executionAgents: m.executionAgents ?? m.execution_agents ?? false,
    }))
    .filter((m) => m.isExternal && m.executionAgents);
}

function buildAlphaAgent(wallet, market, index) {
  const yes = probabilityForOutcome(market, "yes");
  const outcome = yes >= 0.5 ? "yes" : "no";
  const score = wallet.composite_score ?? wallet.compositeScore ?? 0.6;

  return {
    name: `alpha-wfv2-${String(index + 1).padStart(2, "0")}`,
    provider: market.provider,
    marketId: market.id,
    outcome,
    side: "buy",
    price: clampPrice(probabilityForOutcome(market, outcome) + 0.01),
    quantity: 5,
    cadenceSeconds: 120,
    strategy: "wallet-follow-v2",
    strategyParams: {
      targetWallet: wallet.wallet,
      followRatio: 0.6,
      walletScoreMin: 0.55,
      walletScore: score,
      maxDetectionToOrderMs: 1250,
      maxSlippageTicks: 1.0,
      maxConcurrentMarkets: 3,
      cooldownSeconds: 300,
      crowdingGate: 0.75,
    },
    active: true,
    maxNotionalPerExecution: 25,
    maxDailySpendUsdc: 100,
    maxSlippageBps: 50,
  };
}

async function main() {
  console.log(
    `Seeding ${AGENT_COUNT} wallet_follow_v2 agents (category: ${MARKET_CATEGORY})`,
  );

  const { accessToken } = await loginAlphaAdmin();

  const [wallets, markets] = await Promise.all([
    fetchTopWallets(accessToken),
    fetchPolymarkets(accessToken),
  ]);

  if (!wallets.length) {
    throw new Error(
      `no wallets with composite_score >= 0.55 for category ${MARKET_CATEGORY}`,
    );
  }

  if (!markets.length) {
    throw new Error("no executable polymarket markets available");
  }

  console.log(
    `Found ${wallets.length} eligible wallets, ${markets.length} markets`,
  );

  const existingAgents = await listAgents(accessToken);
  const existingByName = new Map(
    existingAgents.map((agent) => [agent.name, agent]),
  );

  let created = 0;
  let updated = 0;
  let unchanged = 0;

  for (let i = 0; i < Math.min(AGENT_COUNT, wallets.length); i++) {
    const wallet = wallets[i];
    const market = markets[i % markets.length];
    const spec = buildAlphaAgent(wallet, market, i);
    const existing = existingByName.get(spec.name);

    if (!existing) {
      const result = await apiPost("/external/agents", accessToken, spec);
      console.log(
        `  Created ${spec.name} -> wallet=${wallet.wallet.slice(0, 10)}... market=${market.id.slice(0, 20)}...`,
      );
      created++;
      continue;
    }

    const needsUpdate =
      existing.strategy !== "wallet-follow-v2" ||
      !existing.active ||
      (existing.executionMode || existing.execution_mode) !== "paper";

    if (needsUpdate) {
      await apiPatch(`/external/agents/${existing.id}`, accessToken, {
        strategy: spec.strategy,
        strategyParams: spec.strategyParams,
        active: true,
      });
      console.log(`  Updated ${spec.name}`);
      updated++;
    } else {
      unchanged++;
    }
  }

  const summary = {
    ok: true,
    strategy: "wallet-follow-v2",
    cohort: "private_alpha",
    executionMode: "paper",
    category: MARKET_CATEGORY,
    walletsFound: wallets.length,
    marketsUsed: markets.length,
    created,
    updated,
    unchanged,
  };

  console.log(JSON.stringify(summary, null, 2));
}

main().catch((error) => {
  console.error(
    JSON.stringify(
      {
        ok: false,
        error: error.message,
        status: error.status,
      },
      null,
      2,
    ),
  );
  process.exit(1);
});
