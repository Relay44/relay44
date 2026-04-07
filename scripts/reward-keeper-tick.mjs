#!/usr/bin/env node

/**
 * Reward keeper: calls RewardDistributor.distribute() when the epoch is ready.
 * Designed to run as a Render cron job (hourly check, distributes weekly).
 */

import { createPublicClient, createWalletClient, http, parseAbi } from 'viem';
import { privateKeyToAccount } from 'viem/accounts';
import { base } from 'viem/chains';

const RPC_URL = process.env.BASE_RPC_URL || 'https://mainnet.base.org';
const PRIVATE_KEY = process.env.REWARD_KEEPER_PRIVATE_KEY || process.env.BASE_DEPLOYER_KEY;
const DISTRIBUTOR = process.env.REWARD_DISTRIBUTOR_ADDRESS || '0x3c4c0A74F9d108F966908a835a9b4b8D946bBce3';

if (!PRIVATE_KEY) {
  console.error('FATAL: REWARD_KEEPER_PRIVATE_KEY or BASE_DEPLOYER_KEY required');
  process.exit(1);
}

const abi = parseAbi([
  'function distribute() external',
  'function lastDistributionAt() view returns (uint256)',
  'function epochDuration() view returns (uint256)',
  'function currentEpoch() view returns (uint256)',
  'function relayToken() view returns (address)',
]);

const erc20Abi = parseAbi([
  'function balanceOf(address) view returns (uint256)',
]);

const transport = http(RPC_URL, { timeout: 15_000 });
const chain = { ...base, id: Number(process.env.BASE_CHAIN_ID || 8453) };
const publicClient = createPublicClient({ chain, transport });
const account = privateKeyToAccount(PRIVATE_KEY);
const walletClient = createWalletClient({ account, chain, transport });

async function tick() {
  const [lastDist, duration, epoch, tokenAddr] = await Promise.all([
    publicClient.readContract({ address: DISTRIBUTOR, abi, functionName: 'lastDistributionAt' }),
    publicClient.readContract({ address: DISTRIBUTOR, abi, functionName: 'epochDuration' }),
    publicClient.readContract({ address: DISTRIBUTOR, abi, functionName: 'currentEpoch' }),
    publicClient.readContract({ address: DISTRIBUTOR, abi, functionName: 'relayToken' }),
  ]);

  const now = BigInt(Math.floor(Date.now() / 1000));
  const nextEpochAt = lastDist + duration;
  const ready = now >= nextEpochAt;

  const balance = await publicClient.readContract({
    address: tokenAddr,
    abi: erc20Abi,
    functionName: 'balanceOf',
    args: [DISTRIBUTOR],
  });

  const result = {
    ok: true,
    currentEpoch: Number(epoch),
    lastDistribution: new Date(Number(lastDist) * 1000).toISOString(),
    nextEpochAt: new Date(Number(nextEpochAt) * 1000).toISOString(),
    epochReady: ready,
    distributorBalance: balance.toString(),
    hasBalance: balance > 0n,
  };

  if (!ready) {
    console.log(JSON.stringify({ ...result, action: 'skip', reason: 'epoch not ready' }, null, 2));
    return;
  }

  if (balance === 0n) {
    console.log(JSON.stringify({ ...result, action: 'skip', reason: 'no RELAY balance to distribute' }, null, 2));
    return;
  }

  const hash = await walletClient.writeContract({
    address: DISTRIBUTOR,
    abi,
    functionName: 'distribute',
  });

  const receipt = await publicClient.waitForTransactionReceipt({ hash });

  console.log(JSON.stringify({
    ...result,
    action: 'distributed',
    txHash: hash,
    status: receipt.status === 'success' ? 'confirmed' : 'failed',
    gasUsed: receipt.gasUsed.toString(),
  }, null, 2));
}

tick().catch((error) => {
  console.error(JSON.stringify({ ok: false, error: error.message }, null, 2));
  process.exit(1);
});
