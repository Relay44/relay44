#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { parseArgs } from "node:util";
import { getAddress, isAddress } from "viem";

const { values } = parseArgs({
  options: {
    mode: { type: "string", default: "production" },
    "allow-missing-secrets": { type: "boolean", default: false },
    "write-report": { type: "boolean", default: false },
  },
});

const repoRoot = process.cwd();
const manifestPath = path.join(repoRoot, "config", "deployments", "base-addresses.json");
const reportDir = path.join(repoRoot, "output", "launch");

const mode = values.mode;
const environment = mode === "production" ? "production" : "staging";
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const expectedContracts = manifest.environments?.[environment]?.contracts || {};
const polymarketIndexerEnabled = ["1", "true", "yes", "on"].includes(
  String(process.env.POLYMARKET_INDEXER_ENABLED || "").trim().toLowerCase(),
);
const creatorEconomicsMaterializerEnabled = ["1", "true", "yes", "on"].includes(
  String(process.env.CREATOR_ECONOMICS_MATERIALIZER_ENABLED || "")
    .trim()
    .toLowerCase(),
);

const checks = [
  {
    name: "DATABASE_URL",
    kind: "config",
    validate: (value) => value.startsWith("postgres://") || value.startsWith("postgresql://"),
    description: "postgres connection string",
  },
  {
    name: "REDIS_URL",
    kind: "config",
    validate: (value) => value.startsWith("redis://") || value.startsWith("rediss://"),
    description: "redis connection string",
  },
  {
    name: "JWT_SECRET",
    kind: "secret",
    validate: (value) => value.length >= 32,
    description: "jwt signing secret",
  },
  {
    name: "SIWE_DOMAIN",
    kind: "config",
    validate: (value) => value.length > 0,
    description: "siwe domain",
  },
  {
    name: "ADMIN_WALLETS",
    kind: "config",
    validate: (value) =>
      value
        .split(",")
        .map((entry) => entry.trim())
        .filter(Boolean)
        .every((entry) => isAddress(entry)),
    description: "admin wallet allowlist",
  },
  {
    name: "BASE_RPC_URL",
    kind: "config",
    validate: (value) => value.startsWith("http"),
    description: "primary base rpc",
    optionalIf: () => Boolean(process.env.BASE_RPC_FALLBACK_URLS?.trim()),
  },
  {
    name: "BASE_RPC_FALLBACK_URLS",
    kind: "config",
    validate: (value) => value.split(",").map((entry) => entry.trim()).filter(Boolean).length > 0,
    description: "fallback base rpc list",
    optionalIf: () => Boolean(process.env.BASE_RPC_URL?.trim()),
  },
  {
    name: "MARKET_CORE_ADDRESS",
    kind: "config",
    validate: (value) => isAddress(value) && getAddress(value) === getAddress(expectedContracts.marketCore),
    description: "market core address matches manifest",
  },
  {
    name: "ORDER_BOOK_ADDRESS",
    kind: "config",
    validate: (value) => isAddress(value) && getAddress(value) === getAddress(expectedContracts.orderBook),
    description: "order book address matches manifest",
  },
  {
    name: "COLLATERAL_VAULT_ADDRESS",
    kind: "config",
    validate: (value) => isAddress(value) && getAddress(value) === getAddress(expectedContracts.collateralVault),
    description: "collateral vault address matches manifest",
  },
  {
    name: "AGENT_RUNTIME_ADDRESS",
    kind: "config",
    validate: (value) => isAddress(value) && getAddress(value) === getAddress(expectedContracts.agentRuntime),
    description: "agent runtime address matches manifest",
  },
  {
    name: "BOOTSTRAP_OPERATOR_ADDRESS",
    kind: "config",
    validate: (value) => isAddress(value),
    description: "bootstrap operator address",
  },
  {
    name: "EVM_WRITES_ENABLED",
    kind: "config",
    validate: (value) => ["1", "true", "yes", "on"].includes(value.toLowerCase()),
    description: "base writes enabled",
  },
  {
    name: "EXTERNAL_TRADING_ENABLED",
    kind: "config",
    validate: (value) => ["1", "true", "yes", "on"].includes(value.toLowerCase()),
    description: "external trading enabled",
  },
  {
    name: "EXTERNAL_AGENTS_ENABLED",
    kind: "config",
    validate: (value) => ["1", "true", "yes", "on"].includes(value.toLowerCase()),
    description: "external agents enabled",
  },
  {
    name: "EXTERNAL_CREDENTIALS_MASTER_KEY",
    kind: "secret",
    validate: (value) => value.length >= 32,
    description: "credential encryption key",
  },
  {
    name: "EXTERNAL_CREDENTIALS_KEY_ID",
    kind: "config",
    validate: (value) => value.length > 0,
    description: "credential key identifier",
  },
  {
    name: "POLYMARKET_BUILDER_API_KEY",
    kind: "secret",
    validate: (value) => value.length > 0,
    description: "polymarket builder api key",
  },
  {
    name: "POLYMARKET_BUILDER_API_SECRET",
    kind: "secret",
    validate: (value) => value.length > 0,
    description: "polymarket builder api secret",
  },
  {
    name: "POLYMARKET_BUILDER_API_PASSPHRASE",
    kind: "secret",
    validate: (value) => value.length > 0,
    description: "polymarket builder api passphrase",
  },
  {
    name: "POLYMARKET_INDEXER_API_URL",
    kind: "config",
    validate: (value) => value.startsWith("http"),
    description: "polymarket indexer api url",
    optionalIf: () => true,
  },
  {
    name: "POLYMARKET_INDEXER_SIWE_DOMAIN",
    kind: "config",
    validate: (value) => value.length > 0,
    description: "polymarket indexer siwe domain",
    optionalIf: () => true,
  },
  {
    name: "POLYMARKET_INDEXER_ADMIN_PRIVATE_KEY",
    kind: "secret",
    validate: (value) => value.startsWith("0x") && value.length >= 66,
    description: "polymarket indexer operator key",
    optionalIf: () => !polymarketIndexerEnabled,
  },
  {
    name: "POLYMARKET_INDEXER_CHAIN_ID",
    kind: "config",
    validate: (value) => Number.isInteger(Number(value)) && Number(value) > 0,
    description: "polymarket indexer chain id",
    optionalIf: () => true,
  },
  {
    name: "POLYMARKET_INDEXER_LIMIT",
    kind: "config",
    validate: (value) => Number.isInteger(Number(value)) && Number(value) > 0,
    description: "polymarket indexer tick limit",
    optionalIf: () => true,
  },
  {
    name: "POLYMARKET_INDEXER_FORWARDER_URL",
    kind: "config",
    validate: (value) => value.startsWith("http"),
    description: "polymarket indexer forwarder url",
    optionalIf: () => true,
  },
  {
    name: "POLYMARKET_INDEXER_OVERDUE_MS",
    kind: "config",
    validate: (value) => Number.isInteger(Number(value)) && Number(value) > 0,
    description: "polymarket indexer overdue threshold",
    optionalIf: () => true,
  },
  {
    name: "POLYMARKET_INDEXER_MAX_BACKFILL_LAG_BLOCKS",
    kind: "config",
    validate: (value) => Number.isInteger(Number(value)) && Number(value) > 0,
    description: "polymarket indexer max backfill lag",
    optionalIf: () => true,
  },
  {
    name: "POLYMARKET_INDEXER_MAX_RECONCILIATION_FAILURES",
    kind: "config",
    validate: (value) => Number.isInteger(Number(value)) && Number(value) > 0,
    description: "polymarket indexer max reconciliation failures",
    optionalIf: () => true,
  },
  {
    name: "CREATOR_ECONOMICS_MATERIALIZER_API_URL",
    kind: "config",
    validate: (value) => value.startsWith("http"),
    description: "creator economics materializer api url",
    optionalIf: () => true,
  },
  {
    name: "CREATOR_ECONOMICS_MATERIALIZER_ADMIN_KEY",
    kind: "secret",
    validate: (value) => value.length >= 16,
    description: "creator economics materializer admin key",
    optionalIf: () =>
      !creatorEconomicsMaterializerEnabled ||
      Boolean(process.env.ADMIN_CONTROL_KEY?.trim()),
  },
  {
    name: "CREATOR_ECONOMICS_MATERIALIZER_WINDOW_DAYS",
    kind: "config",
    validate: (value) => Number.isInteger(Number(value)) && Number(value) > 0,
    description: "creator economics materializer window days",
    optionalIf: () => true,
  },
  {
    name: "CREATOR_ECONOMICS_MATERIALIZER_LIMIT",
    kind: "config",
    validate: (value) => Number.isInteger(Number(value)) && Number(value) > 0,
    description: "creator economics materializer limit",
    optionalIf: () => true,
  },
  {
    name: "CREATOR_ECONOMICS_MATERIALIZER_OVERDUE_MS",
    kind: "config",
    validate: (value) => Number.isInteger(Number(value)) && Number(value) > 0,
    description: "creator economics materializer overdue threshold",
    optionalIf: () => true,
  },
  {
    name: "CREATOR_ECONOMICS_MATERIALIZER_MAX_LAG_DAYS",
    kind: "config",
    validate: (value) => Number.isInteger(Number(value)) && Number(value) >= 0,
    description: "creator economics materializer max lag days",
    optionalIf: () => true,
  },
];

function writeReport(results, failures) {
  const reportPath = path.join(reportDir, `launch-config-${mode}.md`);
  const lines = [
    "# Launch Config Validation",
    "",
    `Mode: ${mode}`,
    `Timestamp: ${new Date().toISOString()}`,
    `Allow missing secrets: ${values["allow-missing-secrets"]}`,
    "",
    "| Variable | Result | Detail |",
    "| --- | --- | --- |",
    ...results.map((result) => `| ${result.name} | ${result.ok ? "pass" : result.level} | ${result.detail} |`),
    "",
    `Failures: ${failures}`,
    "",
  ];
  fs.mkdirSync(reportDir, { recursive: true });
  fs.writeFileSync(reportPath, `${lines.join("\n")}\n`);
  return reportPath;
}

function main() {
  const results = [];
  let failures = 0;

  for (const check of checks) {
    const value = process.env[check.name]?.trim() || "";
    const missing = value.length === 0;
    const optional = check.optionalIf?.() ?? false;

    if (missing) {
      if (optional) {
        results.push({ name: check.name, ok: true, level: "pass", detail: "covered by alternate config" });
        continue;
      }

      const warning = check.kind === "secret" && values["allow-missing-secrets"];
      if (!warning) failures += 1;
      results.push({
        name: check.name,
        ok: warning,
        level: warning ? "warning" : "fail",
        detail: "missing",
      });
      continue;
    }

    const ok = check.validate(value);
    if (!ok) failures += 1;
    results.push({
      name: check.name,
      ok,
      level: ok ? "pass" : "fail",
      detail: ok ? check.description : `invalid ${check.description}`,
    });
  }

  const reportPath = values["write-report"] ? writeReport(results, failures) : null;
  for (const result of results) {
    const prefix = result.level === "warning" ? "WARN" : result.ok ? "PASS" : "FAIL";
    console.log(`${prefix} ${result.name} ${result.detail}`);
  }
  if (reportPath) {
    console.log(`report: ${reportPath}`);
  }

  if (failures > 0) {
    process.exit(1);
  }
}

main();
