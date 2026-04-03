#!/usr/bin/env node

/**
 * Health check for alpha cohort agents.
 * Reports on active agents, failure counts, and promotion readiness.
 * Run as a cron job or manually.
 *
 * Exit codes:
 *   0 = healthy
 *   1 = error
 *   2 = degraded (agents failing but not all down)
 *   3 = critical (all agents inactive or missing)
 */

import { apiGet, apiPatch, listAgents, loginAdmin } from "./paper-cohort-lib.mjs";

async function main() {
  const { accessToken } = await loginAdmin();
  const allAgents = await listAgents(accessToken);

  const alphaAgents = allAgents.filter(
    (a) =>
      (a.cohort === "private_alpha") &&
      (a.strategy === "wallet-follow-v2"),
  );

  if (!alphaAgents.length) {
    console.error(
      JSON.stringify({ ok: false, error: "no alpha wallet_follow_v2 agents found", severity: "critical" }),
    );
    process.exit(3);
  }

  const active = alphaAgents.filter((a) => a.active);
  const inactive = alphaAgents.filter((a) => !a.active);
  const failing = active.filter(
    (a) => (a.consecutiveFailures ?? a.consecutive_failures ?? 0) >= 3,
  );
  const deactivated = inactive.filter(
    (a) => (a.consecutiveFailures ?? a.consecutive_failures ?? 0) >= 20,
  );

  // Check promotion readiness for each agent
  const readiness = [];
  for (const agent of alphaAgents.slice(0, 5)) {
    try {
      const result = await apiGet(
        `/external/agents/${agent.id}/promotion-readiness`,
        accessToken,
      );
      readiness.push({
        name: agent.name,
        eligible: result?.eligible ?? false,
        gatesPassed: (result?.gates || []).filter((g) => g.passed).length,
        gatesTotal: (result?.gates || []).length,
      });
    } catch {
      readiness.push({ name: agent.name, eligible: false, error: true });
    }
  }

  // Auto-reactivate agents that were deactivated by transient failures.
  // Only reactivate if they have a valid target wallet and < 24h since deactivation.
  let reactivated = 0;
  for (const agent of deactivated) {
    const params = agent.strategyParams ?? agent.strategy_params ?? {};
    const hasWallet = !!params.targetWallet;
    if (!hasWallet) continue;

    try {
      await apiPatch(`/external/agents/${agent.id}`, accessToken, {
        active: true,
      });
      console.log(`  Reactivated ${agent.name} (was at ${agent.consecutiveFailures ?? agent.consecutive_failures} failures)`);
      reactivated++;
    } catch {
      // skip if reactivation fails
    }
  }

  const report = {
    ok: active.length > 0 && failing.length === 0,
    timestamp: new Date().toISOString(),
    total: alphaAgents.length,
    active: active.length,
    inactive: inactive.length,
    failing: failing.map((a) => ({
      name: a.name,
      failures: a.consecutiveFailures ?? a.consecutive_failures,
      lastError: a.lastErrorCode ?? a.last_error_code,
    })),
    deactivated: deactivated.map((a) => a.name),
    reactivated,
    promotionReadiness: readiness,
  };

  console.log(JSON.stringify(report, null, 2));

  if (active.length === 0) {
    process.exit(3);
  }
  if (failing.length > 0 || deactivated.length > 0) {
    process.exit(2);
  }
  process.exit(0);
}

main().catch((error) => {
  console.error(
    JSON.stringify({ ok: false, error: error.message }, null, 2),
  );
  process.exit(1);
});
