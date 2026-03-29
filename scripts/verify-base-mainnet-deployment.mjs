#!/usr/bin/env node

import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { createPublicClient, fallback, http } from 'viem';
import { base } from 'viem/chains';

const MANIFEST_PATH = path.join(process.cwd(), 'config', 'deployments', 'base-addresses.json');
const DEFAULT_ADMIN_ROLE = '0x0000000000000000000000000000000000000000000000000000000000000000';

const accessControlAbi = [
  { type: 'function', name: 'hasRole', stateMutability: 'view', inputs: [{ type: 'bytes32', name: 'role' }, { type: 'address', name: 'account' }], outputs: [{ type: 'bool' }] },
];
const marketCoreRoleAbi = [
  ...accessControlAbi,
  { type: 'function', name: 'MARKET_CREATOR_ROLE', stateMutability: 'view', inputs: [], outputs: [{ type: 'bytes32' }] },
  { type: 'function', name: 'RESOLVER_ROLE', stateMutability: 'view', inputs: [], outputs: [{ type: 'bytes32' }] },
  { type: 'function', name: 'PAUSER_ROLE', stateMutability: 'view', inputs: [], outputs: [{ type: 'bytes32' }] },
];
const orderBookAbi = [
  ...accessControlAbi,
  { type: 'function', name: 'PAUSER_ROLE', stateMutability: 'view', inputs: [], outputs: [{ type: 'bytes32' }] },
  { type: 'function', name: 'AGENT_RUNTIME_ROLE', stateMutability: 'view', inputs: [], outputs: [{ type: 'bytes32' }] },
  { type: 'function', name: 'marketCore', stateMutability: 'view', inputs: [], outputs: [{ type: 'address' }] },
  { type: 'function', name: 'collateralVault', stateMutability: 'view', inputs: [], outputs: [{ type: 'address' }] },
];
const collateralVaultAbi = [
  ...accessControlAbi,
  { type: 'function', name: 'PAUSER_ROLE', stateMutability: 'view', inputs: [], outputs: [{ type: 'bytes32' }] },
  { type: 'function', name: 'OPERATOR_ROLE', stateMutability: 'view', inputs: [], outputs: [{ type: 'bytes32' }] },
  { type: 'function', name: 'collateral', stateMutability: 'view', inputs: [], outputs: [{ type: 'address' }] },
];
const agentRuntimeAbi = [
  ...accessControlAbi,
  { type: 'function', name: 'PAUSER_ROLE', stateMutability: 'view', inputs: [], outputs: [{ type: 'bytes32' }] },
  { type: 'function', name: 'orderBook', stateMutability: 'view', inputs: [], outputs: [{ type: 'address' }] },
  { type: 'function', name: 'identityRegistry', stateMutability: 'view', inputs: [], outputs: [{ type: 'address' }] },
  { type: 'function', name: 'agentCount', stateMutability: 'view', inputs: [], outputs: [{ type: 'uint256' }] },
];
const erc20Abi = [
  { type: 'function', name: 'decimals', stateMutability: 'view', inputs: [], outputs: [{ type: 'uint8' }] },
  { type: 'function', name: 'symbol', stateMutability: 'view', inputs: [], outputs: [{ type: 'string' }] },
];

function envAddresses(key) {
  return String(process.env[key] || '')
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean);
}

function createTransport() {
  const primary = String(process.env.BASE_RPC_URL || 'https://mainnet.base.org').trim();
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

async function main() {
  const manifest = JSON.parse(await readFile(MANIFEST_PATH, 'utf8'));
  const production = manifest?.environments?.production;
  if (!production) {
    throw new Error('production deployment manifest missing');
  }

  const contracts = production.contracts;
  const publicClient = createPublicClient({ chain: base, transport: createTransport() });
  const failures = [];

  async function expectCode(label, address) {
    const code = await publicClient.getCode({ address });
    if (!code || code === '0x') {
      failures.push(`${label} has no bytecode at ${address}`);
    }
    return code;
  }

  const [marketCoreCode, orderBookCode, collateralVaultCode, agentRuntimeCode, usdcCode] = await Promise.all([
    expectCode('marketCore', contracts.marketCore),
    expectCode('orderBook', contracts.orderBook),
    expectCode('collateralVault', contracts.collateralVault),
    expectCode('agentRuntime', contracts.agentRuntime),
    expectCode('collateralToken', contracts.collateralToken),
  ]);

  const [marketCreatorRole, resolverRole, marketCorePauserRole, orderBookPauserRole, orderBookRuntimeRoleId, vaultPauserRole, vaultOperatorRoleId, runtimePauserRole] = await Promise.all([
    publicClient.readContract({ address: contracts.marketCore, abi: marketCoreRoleAbi, functionName: 'MARKET_CREATOR_ROLE' }),
    publicClient.readContract({ address: contracts.marketCore, abi: marketCoreRoleAbi, functionName: 'RESOLVER_ROLE' }),
    publicClient.readContract({ address: contracts.marketCore, abi: marketCoreRoleAbi, functionName: 'PAUSER_ROLE' }),
    publicClient.readContract({ address: contracts.orderBook, abi: orderBookAbi, functionName: 'PAUSER_ROLE' }),
    publicClient.readContract({ address: contracts.orderBook, abi: orderBookAbi, functionName: 'AGENT_RUNTIME_ROLE' }),
    publicClient.readContract({ address: contracts.collateralVault, abi: collateralVaultAbi, functionName: 'PAUSER_ROLE' }),
    publicClient.readContract({ address: contracts.collateralVault, abi: collateralVaultAbi, functionName: 'OPERATOR_ROLE' }),
    publicClient.readContract({ address: contracts.agentRuntime, abi: agentRuntimeAbi, functionName: 'PAUSER_ROLE' }),
  ]);

  const [wiredMarketCore, wiredVault, wiredCollateral, wiredOrderBook, wiredIdentityRegistry, agentCount, tokenDecimals, tokenSymbol, orderBookRuntimeRole, vaultOperatorRole] = await Promise.all([
    publicClient.readContract({ address: contracts.orderBook, abi: orderBookAbi, functionName: 'marketCore' }),
    publicClient.readContract({ address: contracts.orderBook, abi: orderBookAbi, functionName: 'collateralVault' }),
    publicClient.readContract({ address: contracts.collateralVault, abi: collateralVaultAbi, functionName: 'collateral' }),
    publicClient.readContract({ address: contracts.agentRuntime, abi: agentRuntimeAbi, functionName: 'orderBook' }),
    publicClient.readContract({ address: contracts.agentRuntime, abi: agentRuntimeAbi, functionName: 'identityRegistry' }),
    publicClient.readContract({ address: contracts.agentRuntime, abi: agentRuntimeAbi, functionName: 'agentCount' }),
    publicClient.readContract({ address: contracts.collateralToken, abi: erc20Abi, functionName: 'decimals' }),
    publicClient.readContract({ address: contracts.collateralToken, abi: erc20Abi, functionName: 'symbol' }),
    publicClient.readContract({ address: contracts.orderBook, abi: accessControlAbi, functionName: 'hasRole', args: [orderBookRuntimeRoleId, contracts.agentRuntime] }),
    publicClient.readContract({ address: contracts.collateralVault, abi: accessControlAbi, functionName: 'hasRole', args: [vaultOperatorRoleId, contracts.orderBook] }),
  ]);

  if (wiredMarketCore.toLowerCase() !== contracts.marketCore.toLowerCase()) {
    failures.push('orderBook.marketCore does not match deployment manifest');
  }
  if (wiredVault.toLowerCase() !== contracts.collateralVault.toLowerCase()) {
    failures.push('orderBook.collateralVault does not match deployment manifest');
  }
  if (wiredCollateral.toLowerCase() !== contracts.collateralToken.toLowerCase()) {
    failures.push('collateralVault.collateral does not match deployment manifest');
  }
  if (wiredOrderBook.toLowerCase() !== contracts.orderBook.toLowerCase()) {
    failures.push('agentRuntime.orderBook does not match deployment manifest');
  }
  if (!orderBookRuntimeRole) {
    failures.push('orderBook is missing AGENT_RUNTIME_ROLE for agentRuntime');
  }
  if (!vaultOperatorRole) {
    failures.push('collateralVault is missing OPERATOR_ROLE for orderBook');
  }

  const expectedAdmins = envAddresses('BASE_EXPECTED_ADMIN_WALLETS');
  const expectedPausers = envAddresses('BASE_EXPECTED_PAUSER_WALLETS');
  const expectedCreators = envAddresses('BASE_EXPECTED_MARKET_CREATOR_WALLETS');
  const expectedResolvers = envAddresses('BASE_EXPECTED_RESOLVER_WALLETS');
  const adminChecks = [];

  for (const address of expectedAdmins) {
    adminChecks.push({
      role: 'default_admin',
      address,
      onMarketCore: await publicClient.readContract({ address: contracts.marketCore, abi: accessControlAbi, functionName: 'hasRole', args: [DEFAULT_ADMIN_ROLE, address] }),
      onOrderBook: await publicClient.readContract({ address: contracts.orderBook, abi: accessControlAbi, functionName: 'hasRole', args: [DEFAULT_ADMIN_ROLE, address] }),
      onCollateralVault: await publicClient.readContract({ address: contracts.collateralVault, abi: accessControlAbi, functionName: 'hasRole', args: [DEFAULT_ADMIN_ROLE, address] }),
      onAgentRuntime: await publicClient.readContract({ address: contracts.agentRuntime, abi: accessControlAbi, functionName: 'hasRole', args: [DEFAULT_ADMIN_ROLE, address] }),
    });
  }

  for (const address of expectedPausers) {
    const checks = await Promise.all([
      publicClient.readContract({ address: contracts.marketCore, abi: accessControlAbi, functionName: 'hasRole', args: [marketCorePauserRole, address] }),
      publicClient.readContract({ address: contracts.orderBook, abi: accessControlAbi, functionName: 'hasRole', args: [orderBookPauserRole, address] }),
      publicClient.readContract({ address: contracts.collateralVault, abi: accessControlAbi, functionName: 'hasRole', args: [vaultPauserRole, address] }),
      publicClient.readContract({ address: contracts.agentRuntime, abi: accessControlAbi, functionName: 'hasRole', args: [runtimePauserRole, address] }),
    ]);
    if (checks.some((entry) => !entry)) {
      failures.push(`pauser wallet missing PAUSER_ROLE on one or more contracts: ${address}`);
    }
  }

  for (const address of expectedCreators) {
    const allowed = await publicClient.readContract({ address: contracts.marketCore, abi: accessControlAbi, functionName: 'hasRole', args: [marketCreatorRole, address] });
    if (!allowed) {
      failures.push(`market creator wallet missing MARKET_CREATOR_ROLE: ${address}`);
    }
  }

  for (const address of expectedResolvers) {
    const allowed = await publicClient.readContract({ address: contracts.marketCore, abi: accessControlAbi, functionName: 'hasRole', args: [resolverRole, address] });
    if (!allowed) {
      failures.push(`resolver wallet missing RESOLVER_ROLE: ${address}`);
    }
  }

  const summary = {
    ok: failures.length === 0,
    network: production.name,
    chainId: production.chainId,
    contracts,
    bytecode: {
      marketCore: marketCoreCode !== '0x',
      orderBook: orderBookCode !== '0x',
      collateralVault: collateralVaultCode !== '0x',
      agentRuntime: agentRuntimeCode !== '0x',
      collateralToken: usdcCode !== '0x',
    },
    wiring: {
      orderBookMarketCore: wiredMarketCore,
      orderBookCollateralVault: wiredVault,
      collateralVaultToken: wiredCollateral,
      agentRuntimeOrderBook: wiredOrderBook,
      agentRuntimeIdentityRegistry: wiredIdentityRegistry,
      orderBookRuntimeRole,
      vaultOperatorRole,
    },
    runtime: {
      agentCount: agentCount.toString(),
    },
    collateralToken: {
      symbol: tokenSymbol,
      decimals: Number(tokenDecimals),
    },
    expectedAdmins: adminChecks,
    failures,
  };

  console.log(JSON.stringify(summary, null, 2));
  if (failures.length > 0) {
    process.exit(1);
  }
}

main().catch((error) => {
  console.error(JSON.stringify({ ok: false, message: error instanceof Error ? error.message : String(error) }, null, 2));
  process.exit(1);
});
