#!/usr/bin/env node

import {
  agentCount,
  apiPatch,
  apiPost,
  buildAgentSpec,
  listAgents,
  listExecutableMarkets,
  loginAdmin,
  strategySequence,
} from "./paper-cohort-lib.mjs";

async function main() {
  const { accessToken } = await loginAdmin();
  const markets = await listExecutableMarkets(accessToken);

  if (!markets.length) {
    throw new Error(
      "no executable external markets available for paper cohort",
    );
  }

  const providers = new Set(markets.map((market) => market.provider));
  if (!providers.has("limitless") || !providers.has("polymarket")) {
    throw new Error(
      "paper cohort requires executable markets from both limitless and polymarket",
    );
  }

  const existingAgents = await listAgents(accessToken);
  const existingByName = new Map(
    existingAgents.map((agent) => [agent.name, agent]),
  );
  const sequence = strategySequence(agentCount);

  let created = 0;
  let activated = 0;
  let unchanged = 0;

  for (let index = 0; index < sequence.length; index += 1) {
    const strategy = sequence[index];
    const market = markets[index % markets.length];
    const spec = buildAgentSpec(strategy, market, index);
    const existing = existingByName.get(spec.name);

    if (!existing) {
      await apiPost("/external/agents", accessToken, spec);
      created += 1;
      continue;
    }

    const sameStrategy = existing.strategy === spec.strategy;
    const sameActive = existing.active === true;

    if (sameStrategy && sameActive) {
      unchanged += 1;
      continue;
    }

    await apiPatch(`/external/agents/${existing.id}`, accessToken, {
      outcome: spec.outcome,
      side: spec.side,
      price: spec.price,
      quantity: spec.quantity,
      cadenceSeconds: spec.cadenceSeconds,
      strategy: spec.strategy,
      active: true,
    });
    activated += 1;
  }

  console.log(
    JSON.stringify(
      {
        ok: true,
        targetAgents: agentCount,
        created,
        activated,
        unchanged,
        marketsUsed: markets.length,
        providers: [...providers].sort(),
      },
      null,
      2,
    ),
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
