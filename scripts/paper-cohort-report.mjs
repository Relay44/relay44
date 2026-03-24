#!/usr/bin/env node

import { apiGet, loginAdmin, writeOutputFile } from "./paper-cohort-lib.mjs";

function toFixed(value) {
  const numeric = Number(value || 0);
  return Number.isFinite(numeric) ? numeric.toFixed(2) : "0.00";
}

async function main() {
  const { accessToken } = await loginAdmin();
  const payload = await apiGet(
    "/external/agents/performance?scope=all",
    accessToken,
  );

  const markdown = [
    "# Paper Cohort Report",
    "",
    `Generated: ${payload.updatedAt}`,
    "",
    "## Totals",
    "",
    `- Agents: ${payload.totals.agents}`,
    `- Active agents: ${payload.totals.activeAgents}`,
    `- Open positions: ${payload.totals.openPositions}`,
    `- Closed positions: ${payload.totals.closedPositions}`,
    `- Fills: ${payload.totals.fills}`,
    `- Volume (USDC): ${toFixed(payload.totals.volumeUsdc)}`,
    `- Fees (USDC): ${toFixed(payload.totals.feesUsdc)}`,
    `- Realized PnL (USDC): ${toFixed(payload.totals.realizedPnlUsdc)}`,
    `- Unrealized PnL (USDC): ${toFixed(payload.totals.unrealizedPnlUsdc)}`,
    `- Net PnL (USDC): ${toFixed(payload.totals.netPnlUsdc)}`,
    "",
    "## Strategy Metrics",
    "",
    "| Strategy | Agents | Active | Open | Closed | Fills | Volume | Fees | Realized | Unrealized | Net | Win Rate |",
    "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ...payload.strategies.map(
      (entry) =>
        `| ${entry.strategy} | ${entry.agents} | ${entry.activeAgents} | ${entry.openPositions} | ${entry.closedPositions} | ${entry.fills} | ${toFixed(entry.volumeUsdc)} | ${toFixed(entry.feesUsdc)} | ${toFixed(entry.realizedPnlUsdc)} | ${toFixed(entry.unrealizedPnlUsdc)} | ${toFixed(entry.netPnlUsdc)} | ${(Number(entry.winRate || 0) * 100).toFixed(1)}% |`,
    ),
    "",
    "## PnL Trajectory",
    "",
    "| Bucket | Volume | Realized | Unrealized | Net |",
    "| --- | ---: | ---: | ---: | ---: |",
    ...payload.timeline.map(
      (entry) =>
        `| ${entry.bucket} | ${toFixed(entry.volumeUsdc)} | ${toFixed(entry.realizedPnlUsdc)} | ${toFixed(entry.unrealizedPnlUsdc)} | ${toFixed(entry.netPnlUsdc)} |`,
    ),
    "",
  ].join("\n");

  const strategyCsv = [
    "strategy,agents,active_agents,open_positions,closed_positions,fills,volume_usdc,fees_usdc,realized_pnl_usdc,unrealized_pnl_usdc,net_pnl_usdc,win_rate",
    ...payload.strategies.map((entry) =>
      [
        entry.strategy,
        entry.agents,
        entry.activeAgents,
        entry.openPositions,
        entry.closedPositions,
        entry.fills,
        toFixed(entry.volumeUsdc),
        toFixed(entry.feesUsdc),
        toFixed(entry.realizedPnlUsdc),
        toFixed(entry.unrealizedPnlUsdc),
        toFixed(entry.netPnlUsdc),
        Number(entry.winRate || 0).toFixed(4),
      ].join(","),
    ),
  ].join("\n");

  const timelineCsv = [
    "bucket,volume_usdc,realized_pnl_usdc,unrealized_pnl_usdc,net_pnl_usdc",
    ...payload.timeline.map((entry) =>
      [
        entry.bucket,
        toFixed(entry.volumeUsdc),
        toFixed(entry.realizedPnlUsdc),
        toFixed(entry.unrealizedPnlUsdc),
        toFixed(entry.netPnlUsdc),
      ].join(","),
    ),
  ].join("\n");

  const reportPath = await writeOutputFile("report.md", markdown);
  const strategyPath = await writeOutputFile(
    "strategy-metrics.csv",
    strategyCsv,
  );
  const timelinePath = await writeOutputFile("timeline.csv", timelineCsv);

  console.log(
    JSON.stringify(
      {
        ok: true,
        reportPath,
        strategyPath,
        timelinePath,
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
