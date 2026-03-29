#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { execFile as execFileCallback } from "node:child_process";
import { parseArgs } from "node:util";
import { promisify } from "node:util";

const execFile = promisify(execFileCallback);

const { values } = parseArgs({
  options: {
    strict: { type: "boolean", default: false },
    "base-url": { type: "string" },
  },
});

const baseUrl = (values["base-url"] || process.env.BASE_URL || "https://relay44.com").replace(/\/$/, "");
const strict = values.strict;
const reportDir = path.join(process.cwd(), "output", "launch");
const reportPath = path.join(reportDir, "production-loop-report.md");

const endpoints = [
  { path: "/", expected: 200, label: "home" },
  { path: "/markets", expected: 200, label: "markets" },
  { path: "/markets/create", expected: 200, label: "create market" },
  { path: "/health", expected: 200, label: "web health" },
  { path: "/health/detailed", expected: 200, label: "detailed health" },
  { path: "/v1/web4/capabilities", expected: 200, label: "capabilities" },
  { path: "/api/proxy/health", expected: 200, label: "proxy health" },
  { path: "/leaderboard", expected: 404, label: "leaderboard hidden" },
  {
    path: "/profile/0x71C7656EC7ab88b098defB751B7401B5f6d8976F",
    expected: 404,
    label: "profile hidden",
  },
];

async function fetchResult(pathname) {
  let lastError = null;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      const response = await fetch(`${baseUrl}${pathname}`, {
        redirect: "manual",
        headers: { accept: "application/json,text/html;q=0.9,*/*;q=0.8" },
      });
      const text = await response.text();
      let json = null;
      if (text) {
        try {
          json = JSON.parse(text);
        } catch {}
      }
      return {
        status: response.status,
        ok: response.ok,
        contentType: response.headers.get("content-type") || "",
        body: json,
        text: text.slice(0, 400),
      };
    } catch (error) {
      lastError = error;
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
  }
  const url = `${baseUrl}${pathname}`;
  const { stdout } = await execFile("curl", [
    "-sS",
    "-H",
    "accept: application/json,text/html;q=0.9,*/*;q=0.8",
    "-w",
    "\n__STATUS__:%{http_code}\n__CONTENT_TYPE__:%{content_type}",
    url,
  ]);
  const statusMatch = stdout.match(/\n__STATUS__:(\d{3})/);
  const contentTypeMatch = stdout.match(/\n__CONTENT_TYPE__:(.*)$/m);
  const bodyText = stdout
    .replace(/\n__STATUS__:\d{3}\n__CONTENT_TYPE__:.*$/s, "")
    .trim();
  let json = null;
  if (bodyText) {
    try {
      json = JSON.parse(bodyText);
    } catch {}
  }
  return {
    status: statusMatch ? Number(statusMatch[1]) : 0,
    ok: statusMatch ? Number(statusMatch[1]) >= 200 && Number(statusMatch[1]) < 300 : false,
    contentType: contentTypeMatch?.[1]?.trim() || "",
    body: json,
    text: bodyText.slice(0, 400),
    fallbackError: lastError instanceof Error ? lastError.message : String(lastError),
  };
}

function pickLaunchState(payload) {
  if (!payload || typeof payload !== "object") return null;
  if ("launch" in payload) return payload.launch;
  if ("capabilities" in payload && payload.capabilities && typeof payload.capabilities === "object") {
    return payload.capabilities.launch ?? null;
  }
  return null;
}

function pickRuntime(payload) {
  if (!payload || typeof payload !== "object") return null;
  if ("runtime" in payload) return payload.runtime;
  if ("capabilities" in payload && payload.capabilities && typeof payload.capabilities === "object") {
    return payload.capabilities.runtime ?? null;
  }
  return null;
}

async function main() {
  const results = [];
  let failures = 0;

  for (const endpoint of endpoints) {
    try {
      const result = await fetchResult(endpoint.path);
      const passed = result.status === endpoint.expected;
      if (!passed) failures += 1;
      results.push({ ...endpoint, ...result, passed });
    } catch (error) {
      failures += 1;
      results.push({
        ...endpoint,
        status: 0,
        passed: false,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  }

  const capabilities = results.find((entry) => entry.path === "/v1/web4/capabilities")?.body;
  const healthDetailed = results.find((entry) => entry.path === "/health/detailed")?.body;
  const capabilityLaunch = pickLaunchState(capabilities);
  const healthLaunch = pickLaunchState(healthDetailed);
  const capabilityRuntime = pickRuntime(capabilities);
  const healthRuntime = pickRuntime(healthDetailed);

  const canCompareReadiness =
    (capabilityLaunch !== null && healthLaunch !== null) ||
    (capabilityRuntime !== null && healthRuntime !== null);
  const agreement =
    !canCompareReadiness ||
    (JSON.stringify(capabilityLaunch ?? null) === JSON.stringify(healthLaunch ?? null) &&
      JSON.stringify(capabilityRuntime ?? null) === JSON.stringify(healthRuntime ?? null));

  if (canCompareReadiness && !agreement) failures += 1;

  const lines = [
    "# Production Loop Report",
    "",
    `Base URL: ${baseUrl}`,
    `Timestamp: ${new Date().toISOString()}`,
    `Strict: ${strict}`,
    "",
    "## Endpoint Checks",
    "",
    "| Path | Expected | Actual | Result |",
    "| --- | --- | --- | --- |",
    ...results.map(
      (result) =>
        `| \`${result.path}\` | ${result.expected} | ${result.status || "error"} | ${result.passed ? "pass" : "fail"} |`,
    ),
    "",
    "## Readiness Agreement",
    "",
    `Health and capability payloads match: ${
      canCompareReadiness ? (agreement ? "yes" : "no") : "not comparable"
    }`,
    "",
    "### Capabilities",
    "```json",
    JSON.stringify(capabilities ?? null, null, 2),
    "```",
    "",
    "### Detailed Health",
    "```json",
    JSON.stringify(healthDetailed ?? null, null, 2),
    "```",
    "",
  ];

  fs.mkdirSync(reportDir, { recursive: true });
  fs.writeFileSync(reportPath, `${lines.join("\n")}\n`);

  for (const result of results) {
    const outcome = result.passed ? "PASS" : "FAIL";
    console.log(`${outcome} ${result.path} -> ${result.status || result.error}`);
  }
  console.log(
    `${canCompareReadiness ? (agreement ? "PASS" : "FAIL") : "PASS"} health/capabilities agreement`,
  );
  console.log(`report: ${reportPath}`);

  if (strict && failures > 0) {
    process.exit(1);
  }
  if (failures > 0) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack : String(error));
  process.exit(1);
});
