'use client';

import { useState } from 'react';
import { useWalletClient } from 'wagmi';
import { encodeFunctionData, parseUnits } from 'viem';

import { useBaseWallet } from '@/hooks/useBaseWallet';
import { Button, Card, CardContent } from '@/components/ui';
import { useToast } from '@/components/ui/Toast';
import { api } from '@/lib/api';
import { payWithBase, isBasePayAvailable } from '@/lib/basePay';

const USDC_ADDRESS = '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913' as const;
const USDC_DECIMALS = 6;

const ERC20_TRANSFER_ABI = [
  {
    name: 'transfer',
    type: 'function',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'to', type: 'address' },
      { name: 'amount', type: 'uint256' },
    ],
    outputs: [{ name: '', type: 'bool' }],
  },
] as const;

type FlowState =
  | 'idle'
  | 'quoting'
  | 'quoted'
  | 'paying'
  | 'verifying'
  | 'unlocked'
  | 'error';

interface PaymentQuote {
  amount: string;
  recipient: string;
  token: string;
  description: string;
}

interface PaymentGateProps {
  title: string;
  body: string;
  resourcePath: string;
  onUnlocked?: () => void;
}

export function PaymentGate({
  title,
  body,
  resourcePath,
  onUnlocked,
}: PaymentGateProps) {
  const { isConnected } = useBaseWallet();
  const { data: walletClient } = useWalletClient();
  const { addToast } = useToast();

  const [state, setState] = useState<FlowState>('idle');
  const [quote, setQuote] = useState<PaymentQuote | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const resetToIdle = () => {
    setState('idle');
    setQuote(null);
    setErrorMessage(null);
  };

  const fetchQuote = async () => {
    setState('quoting');
    setErrorMessage(null);

    try {
      const data = await (api as any).request(
        '/payments/x402/quote?resource=' + encodeURIComponent(resourcePath)
      );
      setQuote(data);
      setState('quoted');
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Failed to fetch payment quote';
      setErrorMessage(message);
      setState('error');
    }
  };

  const executePayment = async () => {
    if (!quote) return;

    setState('paying');
    setErrorMessage(null);

    let txHash: string;

    try {
      if (isBasePayAvailable()) {
        const result = await payWithBase(quote.amount, quote.recipient);
        txHash = result.id;
      } else {
        if (!walletClient) {
          throw new Error('Wallet not connected');
        }

        const amountInUnits = parseUnits(quote.amount, USDC_DECIMALS);

        const data = encodeFunctionData({
          abi: ERC20_TRANSFER_ABI,
          functionName: 'transfer',
          args: [quote.recipient as `0x${string}`, amountInUnits],
        });

        txHash = await walletClient.sendTransaction({
          to: USDC_ADDRESS,
          data,
          chain: walletClient.chain,
          account: walletClient.account,
        });
      }
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Payment transaction failed';
      setErrorMessage(message);
      setState('error');
      return;
    }

    setState('verifying');

    try {
      await (api as any).request('/payments/x402/verify', {
        method: 'POST',
        body: JSON.stringify({ resource: resourcePath, txHash }),
      });

      setState('unlocked');
      addToast('Access unlocked successfully', 'success');
      onUnlocked?.();
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Payment verification failed';
      setErrorMessage(message);
      setState('error');
    }
  };

  const formatAmount = (amount: string) => {
    const num = parseFloat(amount);
    if (isNaN(num)) return amount;
    return `$${num.toFixed(num < 0.01 ? 4 : 2)}`;
  };

  if (state === 'unlocked') {
    return (
      <Card className="border-accent/30 bg-accent/5">
        <div className="space-y-3">
          <div>
            <p className="text-xs font-medium uppercase tracking-[0.18em] text-accent">
              Premium Access
            </p>
            <h2 className="mt-2 text-lg font-semibold text-text-primary">
              Access Unlocked
            </h2>
          </div>
          <p className="text-sm text-text-secondary">
            Payment confirmed. Loading premium content...
          </p>
        </div>
      </Card>
    );
  }

  return (
    <Card className="border-accent/30 bg-accent/5">
      <div className="space-y-4">
        <div>
          <p className="text-xs font-medium uppercase tracking-[0.18em] text-accent">
            Premium Access
          </p>
          <h2 className="mt-2 text-lg font-semibold text-text-primary">
            {title}
          </h2>
        </div>

        <p className="text-sm text-text-secondary">{body}</p>

        {state === 'quoted' && quote && (
          <div className="space-y-2 rounded border border-border bg-bg-secondary p-3">
            <div className="flex items-center justify-between">
              <span className="text-sm text-text-secondary">Amount</span>
              <span className="text-xl font-bold text-accent">
                {formatAmount(quote.amount)} USDC
              </span>
            </div>
            {quote.description && (
              <p className="text-xs text-text-secondary">{quote.description}</p>
            )}
          </div>
        )}

        {state === 'error' && errorMessage && (
          <p className="text-sm text-red-400">{errorMessage}</p>
        )}

        <div className="flex items-center gap-3">
          {state === 'idle' && (
            <Button
              variant="primary"
              disabled={!isConnected}
              onClick={fetchQuote}
            >
              {isConnected ? 'Unlock Access' : 'Connect Wallet to Unlock'}
            </Button>
          )}

          {state === 'quoting' && (
            <Button variant="primary" loading disabled>
              Fetching Quote...
            </Button>
          )}

          {state === 'quoted' && (
            <>
              <Button variant="primary" onClick={executePayment}>
                Confirm Payment
              </Button>
              <Button variant="ghost" onClick={resetToIdle}>
                Cancel
              </Button>
            </>
          )}

          {state === 'paying' && (
            <Button variant="primary" loading disabled>
              Processing Payment...
            </Button>
          )}

          {state === 'verifying' && (
            <Button variant="primary" loading disabled>
              Verifying...
            </Button>
          )}

          {state === 'error' && (
            <Button variant="primary" onClick={fetchQuote}>
              Retry
            </Button>
          )}
        </div>
      </div>
    </Card>
  );
}
