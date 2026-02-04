#!/usr/bin/env node

import { createPublicClient, createWalletClient, fallback, formatEther, http } from 'viem';
import { privateKeyToAccount } from 'viem/accounts';
import { base } from 'viem/chains';

const enabled = isEnabled(process.env.BASE_AGENT_OPERATOR_ENABLED, false);
const apiBase = normalizeApiBase(process.env.BASE_AGENT_OPERATOR_API_URL || 'http://localhost:8080/v1');
const chainId = Number(process.env.BASE_CHAIN_ID || 8453);
const limit = Number(process.env.BASE_AGENT_OPERATOR_LIMIT || 100);
const dryRun = isEnabled(process.env.BASE_AGENT_OPERATOR_DRY_RUN, false);
const minBalanceEth = Number(process.env.BASE_AGENT_OPERATOR_MIN_BALANCE_ETH || 0.0002);

function envOrThrow(key) {
  const value = String(process.env[key] || '').trim();
  if (!value) {
    throw new Error(`${key} is required`);
  }
  return value;
}

function isEnabled(raw, fallback) {
  if (raw == null || raw === '') {
    return fallback;
  }
  return ['1', 'true', 'yes', 'on'].includes(String(raw).trim().toLowerCase());
}

function normalizeApiBase(raw) {
  const trimmed = String(raw || '').trim().replace(/\/$/, '');
  if (!trimmed) {
    return 'http://localhost:8080/v1';
  }
  const withScheme =
    trimmed.startsWith('http://') || trimmed.startsWith('https://')
      ? trimmed
      : `http://${trimmed}`;
  return withScheme.endsWith('/v1') ? withScheme : `${withScheme}/v1`;
}

function createTransport() {
  const primary = envOrThrow('BASE_RPC_URL');
  const fallbacks = String(process.env.BASE_RPC_FALLBACK_URLS || '')
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean)
    .filter((entry) => entry !== primary);
  const urls = [primary, ...fallbacks];
  if (urls.length === 1) {
    return http(urls[0], { timeout: 15_000 });
  }
  return fallback(urls.map((url) => http(url, { timeout: 15_000 })), { rank: false, retryCount: 1 });
}

async function fetchJson(url, init = {}) {
  const response = await fetch(url, init);
  const text = await response.text();
  const payload = text ? JSON.parse(text) : {};
  if (!response.ok) {
    throw new Error(payload?.error?.message || payload?.message || `${response.status} ${response.statusText}`);
  }
  return payload;
}

async function listAgents() {
  const response = await fetchJson(`${apiBase}/evm/agents?limit=${Math.max(limit, 1)}&offset=0&active=true`);
  return Array.isArray(response?.agents) ? response.agents : [];
}

async function prepareExecute(agentId, from) {
  return fetchJson(`${apiBase}/evm/write/agents/execute`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ from, agentId }),
  });
}

async function executeAgent(runtime, agent) {
  const prepared = await prepareExecute(Number(agent.id), runtime.account.address);
  if (dryRun) {
    return { agentId: agent.id, txHash: 'dry-run', method: prepared.method };
  }

  const hash = await runtime.walletClient.sendTransaction({
    account: runtime.account,
    to: prepared.to,
    data: prepared.data,
    value: BigInt(prepared.value),
  });
  const receipt = await runtime.publicClient.waitForTransactionReceipt({ hash });
  return { agentId: agent.id, txHash: hash, status: receipt.status };
}

async function ensureOperatorBalance(runtime) {
  const balance = await runtime.publicClient.getBalance({ address: runtime.account.address });
  const balanceEth = formatEther(balance);
  const minBalanceWei = BigInt(Math.ceil(minBalanceEth * 1e18));

  if (balance < minBalanceWei) {
    return {
      ok: false,
      skipped: true,
      reason: 'operator wallet underfunded',
      operator: runtime.account.address,
      balanceEth,
      minBalanceEth,
    };
  }

  return {
    ok: true,
    operator: runtime.account.address,
    balanceEth,
    minBalanceEth,
  };
}

async function main() {
  if (!enabled) {
    console.log(JSON.stringify({ ok: true, skipped: true, reason: 'base agent operator disabled' }, null, 2));
    return;
  }

  const privateKey = envOrThrow('BASE_AGENT_OPERATOR_PRIVATE_KEY');
  const account = privateKeyToAccount(privateKey);
  const transport = createTransport();
  const chain = { ...base, id: chainId };
  const runtime = {
    account,
    publicClient: createPublicClient({ chain, transport }),
    walletClient: createWalletClient({ account, chain, transport }),
  };
  const balanceCheck = await ensureOperatorBalance(runtime);
  if (!balanceCheck.ok) {
    console.log(JSON.stringify(balanceCheck, null, 2));
    return;
  }
  const startedAt = new Date().toISOString();
  const agents = await listAgents();
  const eligible = agents.filter((agent) => agent.active && agent.can_execute).slice(0, limit);
  const skipped = {
    inactive: agents.filter((agent) => !agent.active).length,
    not_due: agents.filter((agent) => agent.active && !agent.can_execute).length,
  };

  const executions = [];
  const failures = [];

  for (const agent of eligible) {
    try {
      executions.push(await executeAgent(runtime, agent));
    } catch (error) {
      failures.push({ agentId: agent.id, message: error instanceof Error ? error.message : String(error) });
    }
  }

  console.log(
    JSON.stringify(
      {
        ok: failures.length === 0,
        startedAt,
        operator: account.address,
        balanceEth: balanceCheck.balanceEth,
        minBalanceEth,
        dryRun,
        agentsScanned: agents.length,
        eligible: eligible.length,
        executed: executions.length,
        skipped,
        failures,
        executions,
      },
      null,
      2,
    ),
  );

  if (failures.length > 0) {
    process.exit(1);
  }
}

main().catch((error) => {
  console.error(JSON.stringify({ ok: false, message: error.message }, null, 2));
  process.exit(1);
});
