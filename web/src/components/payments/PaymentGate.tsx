'use client';

import { useState } from 'react';
import { x402Client, x402HTTPClient } from '@x402/core/client';
import type { PaymentRequired } from '@x402/core/types';
import { ExactEvmScheme, toClientEvmSigner } from '@x402/evm';
import { usePublicClient, useWalletClient } from 'wagmi';

import { useBaseWallet } from '@/hooks/useBaseWallet';
import { Button, Card } from '@/components/ui';
import { useToast } from '@/components/ui/Toast';
import { api, resolveApiUrl } from '@/lib/api';

type FlowState = 'idle' | 'quoting' | 'quoted' | 'paying' | 'unlocked' | 'error';
type ProtectedResource = 'orderbook' | 'trades' | 'mcp_tool_call';

interface PaymentGateProps {
  title: string;
  body: string;
  resource: ProtectedResource;
  resourcePath: string;
  onPaidData?: (data: unknown) => void;
}

function formatQuoteAmount(quote: PaymentRequired): string {
  const amount = Number(quote.accepts?.[0]?.amount ?? NaN);
  if (!Number.isFinite(amount)) {
    return quote.accepts?.[0]?.amount ?? 'unknown';
  }
  return `$${(amount / 1_000_000).toFixed(amount < 10_000 ? 4 : 2)}`;
}

export function PaymentGate({
  title,
  body,
  resource,
  resourcePath,
  onPaidData,
}: PaymentGateProps) {
  const { isConnected } = useBaseWallet();
  const publicClient = usePublicClient();
  const { data: walletClient } = useWalletClient();
  const { addToast } = useToast();

  const [state, setState] = useState<FlowState>('idle');
  const [quote, setQuote] = useState<PaymentRequired | null>(null);
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
      const nextQuote = await api.getX402Quote(resource);
      if (!nextQuote.accepts?.[0]) {
        throw new Error('No payment route is available for this resource.');
      }
      setQuote(nextQuote);
      setState('quoted');
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Failed to fetch payment quote';
      setErrorMessage(message);
      setState('error');
    }
  };

  const executePayment = async () => {
    if (!walletClient?.account || !publicClient) {
      setErrorMessage('Wallet client unavailable');
      setState('error');
      return;
    }
    if (!quote) {
      setErrorMessage('Payment quote unavailable');
      setState('error');
      return;
    }

    setState('paying');
    setErrorMessage(null);

    try {
      const signer = toClientEvmSigner(
        {
          address: walletClient.account.address,
          signTypedData: (args) => walletClient.signTypedData(args),
        },
        publicClient,
      );
      const paymentClient = new x402HTTPClient(
        new x402Client().register('eip155:*', new ExactEvmScheme(signer)),
      );
      const payload = await paymentClient.createPaymentPayload(quote);
      const response = await fetch(resolveApiUrl(resourcePath), {
        cache: 'no-store',
        headers: paymentClient.encodePaymentSignatureHeader(payload),
      });

      if (!response.ok) {
        const raw = await response.text();
        throw new Error(raw.trim() || 'Payment failed');
      }

      const data = await response.json();
      onPaidData?.(data);
      setState('unlocked');
      addToast('Payment confirmed', 'success');
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Payment failed';
      setErrorMessage(message);
      setState('error');
    }
  };

  if (state === 'unlocked') {
    return (
      <Card className="border-accent/30 bg-accent/5 p-5">
        <div className="space-y-3">
          <div>
            <p className="text-xs font-medium uppercase tracking-[0.18em] text-accent">
              Premium Access
            </p>
            <h2 className="mt-2 text-lg font-semibold text-text-primary">
              Paid Snapshot Loaded
            </h2>
          </div>
          <p className="text-sm text-text-secondary">
            The premium response was fetched successfully.
          </p>
        </div>
      </Card>
    );
  }

  return (
    <Card className="border-accent/30 bg-accent/5 p-5">
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

        {state === 'quoted' && quote ? (
          <div className="rounded border border-border bg-bg-secondary p-3">
            <div className="flex items-center justify-between">
              <span className="text-sm text-text-secondary">Price</span>
              <span className="text-xl font-bold text-accent">
                {formatQuoteAmount(quote)} USDC
              </span>
            </div>
            <p className="mt-2 text-xs text-text-secondary">
              Payment uses an x402 signature payload. The facilitator settles it
              after verification.
            </p>
          </div>
        ) : null}

        {state === 'error' && errorMessage ? (
          <p className="text-sm text-red-400">{errorMessage}</p>
        ) : null}

        <div className="flex items-center gap-3">
          {state === 'idle' ? (
            <Button
              variant="primary"
              disabled={!isConnected}
              onClick={fetchQuote}
            >
              {isConnected ? 'Preview Price' : 'Connect Wallet to Continue'}
            </Button>
          ) : null}

          {state === 'quoting' ? (
            <Button variant="primary" loading disabled>
              Fetching Quote...
            </Button>
          ) : null}

          {state === 'quoted' ? (
            <>
              <Button
                variant="primary"
                disabled={!walletClient?.account || !publicClient}
                onClick={executePayment}
              >
                Pay and Load
              </Button>
              <Button variant="ghost" onClick={resetToIdle}>
                Cancel
              </Button>
            </>
          ) : null}

          {state === 'paying' ? (
            <Button variant="primary" loading disabled>
              Processing Payment...
            </Button>
          ) : null}

          {state === 'error' ? (
            <Button variant="primary" onClick={fetchQuote}>
              Retry
            </Button>
          ) : null}
        </div>
      </div>
    </Card>
  );
}
