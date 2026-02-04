'use client';

import { useState, useEffect } from 'react';
import { waitForTransactionReceipt } from '@wagmi/core';
import { useConfig, useWalletClient } from 'wagmi';
import { useRuntimeMode } from '@/hooks';
import { ReadOnlyNotice } from '@/components/runtime/ReadOnlyNotice';
import { api } from '@/lib/api';
import { BASE_CHAIN_ID } from '@/lib/constants';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import type { DepositAddress, PreparedWalletTransaction } from '@/types';
import { useBaseWallet } from '@/hooks/useBaseWallet';

interface DepositFormProps {
  onSuccess?: () => void;
}

export function DepositForm({ onSuccess }: DepositFormProps) {
  const [amount, setAmount] = useState('');
  const [depositAddress, setDepositAddress] = useState<DepositAddress | null>(null);
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

  useEffect(() => {
    if (readOnly) {
      return;
    }

    async function fetchDepositAddress() {
      try {
        const addr = await api.getDepositAddress();
        setDepositAddress(addr);
      } catch (err) {
        console.error('Failed to fetch deposit address:', err);
      }
    }
    void fetchDepositAddress();
  }, [readOnly]);

  if (readOnly) {
    return (
      <ReadOnlyNotice
        title="Deposits are disabled"
        body="This preview exposes wallet state without allowing new deposits."
      />
    );
  }

  const handleDeposit = async () => {
    if (!amount || parseFloat(amount) < 1) {
      setError('Minimum deposit is 1 USDC');
      return;
    }

    setLoading(true);
    setError(null);
    setSuccess(null);

    try {
      if (!wallet.isConnected || !wallet.address) {
        throw new Error('Connect your Base wallet before depositing');
      }
      if (!walletClient) throw new Error('Wallet client unavailable');
      await wallet.ensureBaseChain();

      const amountLamports = Math.floor(parseFloat(amount) * 1_000_000);
      const prepared = await api.deposit({
        amount: amountLamports,
        source: 'wallet',
        mode: 'prepare',
      });
      if (!prepared.intentId || !prepared.preparedTransactions?.length) {
        throw new Error('Deposit preparation failed: missing intent or transactions');
      }
      const txHash = await sendPreparedTransactions(
        prepared.preparedTransactions,
        wallet.address as `0x${string}`
      );
      const response = await api.deposit({
        amount: amountLamports,
        source: 'wallet',
        mode: 'confirm',
        intentId: prepared.intentId,
        txSignature: txHash,
      });

      if (response.status === 'confirmed') {
        setSuccess('Deposit confirmed onchain.');
      } else {
        setSuccess('Deposit submitted and pending confirmation.');
      }
      setAmount('');
      onSuccess?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Deposit failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="p-4 border border-border text-sm text-text-secondary">
        Deposit flow is now vault-first on Base:
        approve USDC, deposit to vault, then confirm.
      </div>

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
            className="pl-7"
          />
        </div>
        <div className="flex gap-2">
          {[10, 50, 100, 500].map((preset) => (
            <Button
              key={preset}
              variant="ghost"
              size="sm"
              onClick={() => setAmount(preset.toString())}
              className="flex-1"
            >
              ${preset}
            </Button>
          ))}
        </div>
      </div>

      {depositAddress && (
        <div className="space-y-2 p-4  bg-bg-secondary">
          <p className="text-sm font-medium text-text-secondary">
            Vault Contract
          </p>
          <div className="flex items-center gap-2">
            <code className="flex-1 text-sm text-text-primary bg-bg-tertiary px-3 py-2  font-mono break-all">
              {depositAddress.address}
            </code>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => navigator.clipboard.writeText(depositAddress.address)}
            >
              Copy
            </Button>
          </div>
          <p className="text-xs text-text-secondary mt-2">
            Transactions are signed from your connected wallet and settled on Base.
          </p>
        </div>
      )}

      {error && (
        <div className="p-3  bg-ask/10 border border-ask/20">
          <p className="text-sm text-ask">{error}</p>
        </div>
      )}

      {success && (
        <div className="p-3  bg-bid/10 border border-bid/20">
          <p className="text-sm text-bid">{success}</p>
        </div>
      )}

      <Button
        variant="primary"
        size="lg"
        className="w-full"
        onClick={() => {
          if (wallet.isConnected) {
            void handleDeposit();
            return;
          }
          setError(null);
          void wallet.connect().catch((err) => {
            setError(err instanceof Error ? err.message : 'Wallet connection failed');
          });
        }}
        loading={loading || walletBusy}
        disabled={wallet.isConnected && (!amount || parseFloat(amount) < 1)}
      >
        {wallet.isConnected ? 'Deposit to Vault' : 'Connect Base Wallet'}
      </Button>
    </div>
  );
}

