'use client';

import { useState } from 'react';
import { waitForTransactionReceipt } from '@wagmi/core';
import { useConfig, useWalletClient } from 'wagmi';
import { useRuntimeMode } from '@/hooks';
import { ReadOnlyNotice } from '@/components/runtime/ReadOnlyNotice';
import { api } from '@/lib/api';
import { BASE_CHAIN_ID } from '@/lib/constants';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import type { PreparedWalletTransaction } from '@/types';
import { useBaseWallet } from '@/hooks/useBaseWallet';

interface WithdrawFormProps {
  availableBalance: number;
  onSuccess?: () => void;
}

function formatUsdc(amount: number): string {
  return (amount / 1_000_000).toFixed(2);
}

export function WithdrawForm({ availableBalance, onSuccess }: WithdrawFormProps) {
  const [amount, setAmount] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const wallet = useBaseWallet();
  const config = useConfig();
  const { data: walletClient } = useWalletClient();
  const { readOnly } = useRuntimeMode();
  const walletBusy =
    (!wallet.isConnected && wallet.isConnecting) ||
    (wallet.chainId !== undefined &&
      wallet.chainId !== BASE_CHAIN_ID &&
      wallet.isSwitchingChain);

  const amountNumber = parseFloat(amount) || 0;
  const amountLamports = Math.floor(amountNumber * 1_000_000);
  const fee = 0;
  const netAmount = amountLamports;

  const sendPreparedTransactions = async (
    txs: PreparedWalletTransaction[],
    account: `0x${string}`
  ): Promise<`0x${string}`> => {
    let finalHash: `0x${string}` | null = null;
    for (const tx of txs) {
      const hash = await walletClient!.sendTransaction({
        account,
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
  };

  const handleWithdraw = async () => {
    if (amountLamports < 1_000_000) {
      setError('Minimum withdrawal is 1 USDC');
      return;
    }

    if (amountLamports > availableBalance) {
      setError('Insufficient balance');
      return;
    }

    setLoading(true);
    setError(null);
    setSuccess(null);

    try {
      if (!wallet.isConnected || !wallet.address) {
        throw new Error('Connect your Base wallet before withdrawing');
      }
      if (!walletClient) throw new Error('Wallet client unavailable');
      await wallet.ensureBaseChain();

      const prepared = await api.withdraw({
        amount: amountLamports,
        destination: wallet.address,
        mode: 'prepare',
      });
      if (!prepared.intentId || !prepared.preparedTransactions?.length) {
        throw new Error('Withdraw preparation failed: missing intent or transactions');
      }
      const txHash = await sendPreparedTransactions(
        prepared.preparedTransactions,
        wallet.address as `0x${string}`
      );
      const response = await api.withdraw({
        amount: amountLamports,
        destination: wallet.address,
        mode: 'confirm',
        intentId: prepared.intentId,
        txSignature: txHash,
      });

      if (response.status === 'confirmed') {
        setSuccess(
          `Withdrawal confirmed onchain. ${formatUsdc(response.netAmount)} USDC sent.`
        );
      } else {
        setSuccess(
          `Withdrawal submitted and pending confirmation.`
        );
      }
      setAmount('');
      onSuccess?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Withdrawal failed');
    } finally {
      setLoading(false);
    }
  };

  const handleMaxAmount = () => {
    const maxAmount = availableBalance / 1_000_000;
    setAmount(maxAmount.toFixed(2));
  };

  if (readOnly) {
    return (
      <ReadOnlyNotice
        title="Withdrawals are disabled"
        body="This preview keeps balances and history visible while withdrawal execution stays locked."
      />
    );
  }

  return (
    <div className="space-y-6">
      {/* Available Balance */}
      <div className="p-4  bg-bg-secondary">
        <p className="text-sm text-text-secondary">Available Balance</p>
        <p className="text-xl font-semibold text-text-primary">
          ${formatUsdc(availableBalance)} USDC
        </p>
      </div>

      {/* Amount Input */}
      <div className="space-y-2">
        <label className="text-sm font-medium text-text-secondary">
          Amount (USDC)
        </label>
        <div className="relative">
          <span className="absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary">
            $
          </span>
          <Input
            type="number"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            placeholder="0.00"
            min="1"
            step="0.01"
            className="pl-7 pr-16"
          />
          <button
            onClick={handleMaxAmount}
            className="absolute right-3 top-1/2 -translate-y-1/2 text-sm text-accent hover:text-accent-hover cursor-pointer"
          >
            MAX
          </button>
        </div>
      </div>

