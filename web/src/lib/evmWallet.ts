'use client';

import { waitForTransactionReceipt, type Config } from '@wagmi/core';
import type { WalletClient } from 'viem';

import type { PreparedWalletTransaction } from '@/types';

export async function sendPreparedTransactions(
  walletClient: WalletClient,
  config: Config,
  txs: PreparedWalletTransaction[],
  account: `0x${string}`,
): Promise<`0x${string}`> {
  let finalHash: `0x${string}` | null = null;

  for (const tx of txs) {
    const hash = await walletClient.sendTransaction({
      account,
      chain: walletClient.chain,
      to: tx.to as `0x${string}`,
      data: tx.data,
      value: BigInt(tx.value),
    });
    await waitForTransactionReceipt(config, { hash });
    finalHash = hash;
  }

  if (!finalHash) {
    throw new Error('No transactions were submitted');
  }

  return finalHash;
}
