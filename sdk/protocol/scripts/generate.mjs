#!/usr/bin/env node
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const root = resolve(__dirname, '../../..');
const outputPath = resolve(root, 'sdk/protocol/src/generated.ts');
const deploymentPath = resolve(root, 'config/deployments/base-addresses.json');

const contracts = [
  { key: 'marketCore', label: 'MarketCore', artifact: 'MarketCore.sol/MarketCore.json', abiConst: 'marketCoreAbi', compatConst: 'MARKET_CORE_ABI' },
  { key: 'orderBook', label: 'OrderBook', artifact: 'OrderBook.sol/OrderBook.json', abiConst: 'orderBookAbi', compatConst: 'ORDER_BOOK_ABI' },
  { key: 'collateralVault', label: 'CollateralVault', artifact: 'CollateralVault.sol/CollateralVault.json', abiConst: 'collateralVaultAbi' },
  { key: 'agentRuntime', label: 'AgentRuntime', artifact: 'AgentRuntime.sol/AgentRuntime.json', abiConst: 'agentRuntimeAbi' },
  { key: 'collateralToken', label: 'ERC20', artifact: 'ERC20.sol/ERC20.json', abiConst: 'erc20Abi', compatConst: 'ERC20_ABI' },
  { key: 'relayToken', label: 'RelayToken', artifact: 'RelayToken.sol/RelayToken.json', abiConst: 'relayTokenAbi' },
  { key: 'relayStaking', label: 'RelayStaking', artifact: 'RelayStaking.sol/RelayStaking.json', abiConst: 'relayStakingAbi', compatConst: 'RELAY_STAKING_ABI' },
  { key: 'rewardDistributor', label: 'RewardDistributor', artifact: 'RewardDistributor.sol/RewardDistributor.json', abiConst: 'rewardDistributorAbi' },
  { key: 'agentIdentityRegistry', label: 'AgentIdentityRegistry', artifact: 'AgentIdentityRegistry.sol/AgentIdentityRegistry.json', abiConst: 'agentIdentityRegistryAbi' },
  { key: 'agentReputationRegistry', label: 'AgentReputationRegistry', artifact: 'AgentReputationRegistry.sol/AgentReputationRegistry.json', abiConst: 'agentReputationRegistryAbi' },
  { key: 'erc8004IdentityRegistry', label: 'ERC8004IdentityRegistry', artifact: 'ERC8004IdentityRegistry.sol/ERC8004IdentityRegistry.json', abiConst: 'erc8004IdentityRegistryAbi' },
  { key: 'erc8004ReputationRegistry', label: 'ERC8004ReputationRegistry', artifact: 'ERC8004ReputationRegistry.sol/ERC8004ReputationRegistry.json', abiConst: 'erc8004ReputationRegistryAbi' },
  { key: 'erc8004ValidationRegistry', label: 'ERC8004ValidationRegistry', artifact: 'ERC8004ValidationRegistry.sol/ERC8004ValidationRegistry.json', abiConst: 'erc8004ValidationRegistryAbi' },
];

const eventConstants = [
  { name: 'MARKET_CREATED_EVENT_ABI', contract: 'marketCore', event: 'MarketCreated' },
  { name: 'ORDER_PLACED_EVENT_ABI', contract: 'orderBook', event: 'OrderPlaced' },
];

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'));
}

function artifactAbi(contract) {
  const path = resolve(root, 'evm/out', contract.artifact);
  if (!existsSync(path)) {
    throw new Error(`Missing ${contract.artifact}. Run \`forge build --root evm\` before generating protocol artifacts.`);
  }
  const artifact = readJson(path);
  if (!Array.isArray(artifact.abi)) {
    throw new Error(`${contract.artifact} does not contain an ABI array.`);
  }
  return artifact.abi;
}

function assertAddress(address, context) {
  if (address == null) return;
  if (typeof address !== 'string' || !/^0x[a-fA-F0-9]{40}$/.test(address)) {
    throw new Error(`${context} is not a valid EVM address: ${address}`);
  }
}

function validateDeploymentManifest(manifest) {
  for (const [environment, entry] of Object.entries(manifest.environments ?? {})) {
    if (!entry || typeof entry !== 'object') {
      throw new Error(`Deployment environment ${environment} is invalid.`);
    }
    if (!Number.isInteger(entry.chainId)) {
      throw new Error(`Deployment environment ${environment} is missing chainId.`);
    }
    for (const [contract, address] of Object.entries(entry.contracts ?? {})) {
      assertAddress(address, `${environment}.${contract}`);
    }
  }
}

function formatConst(name, value) {
  return `export const ${name} = ${JSON.stringify(value, null, 2)} as const;\n\n`;
}

function buildGeneratedSource() {
  const manifest = readJson(deploymentPath);
  validateDeploymentManifest(manifest);

  const abiByContract = Object.fromEntries(contracts.map((contract) => [contract.key, artifactAbi(contract)]));
  const contractKeys = contracts.map((contract) => contract.key);
  const abiConstLines = contracts
    .map((contract) => formatConst(contract.abiConst, abiByContract[contract.key]))
    .join('');
  const compatConstLines = contracts
    .filter((contract) => contract.compatConst)
    .map((contract) => `export const ${contract.compatConst} = ${contract.abiConst};\n`)
    .join('');
  const eventConstLines = eventConstants
    .map((eventConstant) => {
      const events = abiByContract[eventConstant.contract].filter(
        (entry) => entry.type === 'event' && entry.name === eventConstant.event,
      );
      if (events.length !== 1) {
        throw new Error(`Expected exactly one ${eventConstant.event} event in ${eventConstant.contract}.`);
      }
      return formatConst(eventConstant.name, events);
    })
    .join('');
  const abiMapEntries = contracts
    .map((contract) => `  ${contract.key}: ${contract.abiConst},`)
    .join('\n');
  const labelMapEntries = contracts
    .map((contract) => `  ${contract.key}: '${contract.label}',`)
    .join('\n');

  return `/* eslint-disable */\n// Generated by sdk/protocol/scripts/generate.mjs. Do not edit by hand.\n\n` +
    formatConst('deploymentManifest', manifest) +
    formatConst('contractNames', contractKeys) +
    abiConstLines +
    compatConstLines +
    eventConstLines +
    `export const productionAddresses = deploymentManifest.environments.production.contracts;\n` +
    `export const stagingAddresses = deploymentManifest.environments.staging.contracts;\n\n` +
    `export const contractLabels = {\n${labelMapEntries}\n} as const;\n\n` +
    `export const contractAbis = {\n${abiMapEntries}\n} as const;\n\n` +
    `export type Address = \`0x\${string}\`;\n` +
    `export type NetworkName = keyof typeof deploymentManifest.environments;\n` +
    `export type ContractName = (typeof contractNames)[number];\n` +
    `export type ContractAbi = readonly Record<string, unknown>[];\n\n` +
    `export function getContractAddress(network: NetworkName, contract: ContractName): Address {\n` +
    `  const contracts = deploymentManifest.environments[network].contracts as Partial<Record<ContractName, Address | null>>;\n` +
    `  const address = contracts[contract];\n` +
    `  if (!address) {\n` +
    `    throw new Error(\`Relay44 contract \${contract} is not deployed on \${network}\`);\n` +
    `  }\n` +
    `  return address as Address;\n` +
    `}\n\n` +
    `export function getContractAbi(contract: ContractName): ContractAbi {\n` +
    `  return contractAbis[contract] as ContractAbi;\n` +
    `}\n`;
}

const nextSource = buildGeneratedSource();
const checkOnly = process.argv.includes('--check');
const currentSource = existsSync(outputPath) ? readFileSync(outputPath, 'utf8') : '';

if (checkOnly) {
  if (currentSource !== nextSource) {
    process.stderr.write('sdk/protocol/src/generated.ts is out of date. Run `npm run sdk:protocol:generate` after `forge build --root evm`.\n');
    process.exit(1);
  }
  process.stdout.write('protocol artifacts are current\n');
} else {
  writeFileSync(outputPath, nextSource);
  process.stdout.write(`wrote ${outputPath}\n`);
}
