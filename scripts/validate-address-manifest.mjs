#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { parseArgs } from "node:util";
import { getAddress, isAddress } from "viem";

const { values } = parseArgs({
  options: {
    environment: { type: "string", default: "production" },
    "write-report": { type: "boolean", default: false },
  },
});

const repoRoot = process.cwd();
const manifestPath = path.join(repoRoot, "config", "deployments", "base-addresses.json");
const reportDir = path.join(repoRoot, "output", "launch");

const envMap = {
  marketCore: ["MARKET_CORE_ADDRESS", "NEXT_PUBLIC_MARKET_CORE_ADDRESS"],
  orderBook: ["ORDER_BOOK_ADDRESS", "NEXT_PUBLIC_ORDER_BOOK_ADDRESS"],
  collateralVault: ["COLLATERAL_VAULT_ADDRESS", "NEXT_PUBLIC_COLLATERAL_VAULT_ADDRESS"],
  agentRuntime: ["AGENT_RUNTIME_ADDRESS", "NEXT_PUBLIC_AGENT_RUNTIME_ADDRESS"],
  collateralToken: ["COLLATERAL_TOKEN_ADDRESS", "NEXT_PUBLIC_COLLATERAL_TOKEN_ADDRESS"],
};

function normalizeAddress(value) {
  if (!value || !isAddress(value)) return null;
  return getAddress(value);
}

function writeReport(environment, checks, failures) {
  const reportPath = path.join(reportDir, `address-manifest-${environment}.md`);
  const lines = [
    "# Address Manifest Validation",
    "",
    `Environment: ${environment}`,
    `Timestamp: ${new Date().toISOString()}`,
    "",
    "| Check | Result | Detail |",
    "| --- | --- | --- |",
    ...checks.map((check) => `| ${check.name} | ${check.ok ? "pass" : "fail"} | ${check.detail} |`),
    "",
    `Failures: ${failures}`,
    "",
  ];
  fs.mkdirSync(reportDir, { recursive: true });
  fs.writeFileSync(reportPath, `${lines.join("\n")}\n`);
  return reportPath;
}

function main() {
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  const environment = values.environment;
  const entry = manifest.environments?.[environment];
  if (!entry) {
    throw new Error(`unknown environment: ${environment}`);
  }

  const checks = [];
  let failures = 0;

  checks.push({
    name: "chainId",
    ok: Number.isInteger(entry.chainId) && entry.chainId > 0,
    detail: String(entry.chainId),
  });

  for (const [name, value] of Object.entries(entry.contracts || {})) {
    const required = environment === "production" || name !== "agentRuntime";
    const normalized = normalizeAddress(value);
    const ok = required ? Boolean(normalized) : normalized !== null || value === null;
    if (!ok) failures += 1;
    checks.push({
      name: `manifest:${name}`,
      ok,
      detail: value === null ? "null" : String(value),
    });

    const envKeys = envMap[name] || [];
    for (const envKey of envKeys) {
      const envValue = process.env[envKey];
      if (!envValue) continue;
      const envAddress = normalizeAddress(envValue);
      const matches = normalized ? envAddress === normalized : envAddress === null;
      if (!matches) failures += 1;
      checks.push({
        name: `env:${envKey}`,
        ok: matches,
        detail: envAddress ? `${envAddress} vs ${normalized}` : "invalid address",
      });
    }
  }

  const reportPath = values["write-report"] ? writeReport(environment, checks, failures) : null;
  for (const check of checks) {
    console.log(`${check.ok ? "PASS" : "FAIL"} ${check.name} ${check.detail}`);
  }
  if (reportPath) {
    console.log(`report: ${reportPath}`);
  }

  if (failures > 0) {
    process.exit(1);
  }
}

main();
