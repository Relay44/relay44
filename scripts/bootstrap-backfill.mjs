#!/usr/bin/env node

import { privateKeyToAccount } from "viem/accounts";

const apiBase = normalizeApiBase(
  process.env.BASE_AGENT_OPERATOR_API_URL || "http://localhost:8080/v1",
);
const apiOrigin = apiBase.replace(/\/v1$/, "");
const siweDomain = (
  process.env.BASE_AGENT_OPERATOR_SIWE_DOMAIN ||
  process.env.SIWE_DOMAIN ||
  "localhost:3000"
).trim();
const chainId = Number(process.env.BASE_CHAIN_ID || 8453);

function envOrThrow(key) {
  const value = String(process.env[key] || "").trim();
  if (!value) {
    throw new Error(`${key} is required`);
  }
  return value;
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

function buildHeaders(token) {
  const headers = {
    "content-type": "application/json",
  };

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
  const payload = text ? JSON.parse(text) : null;

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
  const message = `${siweDomain} wants you to sign in with your Ethereum account:\n${account.address}\n\nSign in to relay44 bootstrap admin\n\nURI: ${apiOrigin}\nVersion: 1\nChain ID: ${chainId}\nNonce: ${nonce}\nIssued At: ${issuedAt}`;
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

  return tokens.access_token;
}

function parseArgs(argv) {
  const options = {
    marketIds: [],
    strategy: null,
    preset: null,
    dryRun: false,
  };

  for (const arg of argv) {
    if (arg === "--dry-run") {
      options.dryRun = true;
      continue;
    }
    if (arg.startsWith("--market-id=")) {
      options.marketIds.push(Number(arg.split("=")[1]));
      continue;
    }
    if (arg.startsWith("--strategy=")) {
      options.strategy = arg.split("=")[1] || null;
      continue;
    }
    if (arg.startsWith("--preset=")) {
      options.preset = arg.split("=")[1] || null;
    }
  }

  return options;
}

async function main() {
  const privateKey = envOrThrow("BASE_AGENT_OPERATOR_PRIVATE_KEY");
  const account = privateKeyToAccount(privateKey);
  const accessToken = await loginAdmin(account);
  const options = parseArgs(process.argv.slice(2));

  const response = await fetchJson(`${apiBase}/evm/bootstrap/admin/backfill`, {
    method: "POST",
    headers: buildHeaders(accessToken),
    body: JSON.stringify({
      marketIds: options.marketIds.length > 0 ? options.marketIds : undefined,
      strategy: options.strategy || undefined,
      preset: options.preset || undefined,
      dryRun: options.dryRun,
    }),
  });

  console.log(
    JSON.stringify(
      {
        ok: true,
        operator: account.address,
        ...response,
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
