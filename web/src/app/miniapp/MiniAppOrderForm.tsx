'use client';

import { useState, useMemo, useEffect } from 'react';
import { Button, Input, Card, Tabs, Spinner, useToast } from '@/components/ui';
import { api, type ExternalCredential, type ExternalOrderIntent } from '@/lib/api';
import { usePlaceOrder } from '@/hooks';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { formatPrice, cn } from '@/lib/utils';
import type { Market, Outcome, OrderSide } from '@/types';

interface MiniAppOrderFormProps {
  market: Market;
  onSuccess?: () => void;
}

function providerFromMarket(market: Market): 'limitless' | 'polymarket' {
  return market.provider === 'polymarket' ? 'polymarket' : 'limitless';
}

export function MiniAppOrderForm({ market, onSuccess }: MiniAppOrderFormProps) {
  const [outcome, setOutcome] = useState<Outcome>('yes');
  const [side, setSide] = useState<OrderSide>('buy');
  const [amount, setAmount] = useState('');
  const [price, setPrice] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [credential, setCredential] = useState<ExternalCredential | null>(null);
  const [credentialLoading, setCredentialLoading] = useState(false);

  const { addToast } = useToast();
  const baseWallet = useBaseWallet();
  const placeOrder = usePlaceOrder();
  const provider = providerFromMarket(market);
  const isExternal = market.isExternal;
  const walletReady = baseWallet.isConnected && Boolean(baseWallet.address);
  const credentialReady = !isExternal || Boolean(credential);

  const currentPrice = outcome === 'yes' ? market.yesPrice : market.noPrice;
  const effectivePrice = price ? parseFloat(price) : currentPrice;
  const amountValue = parseFloat(amount) || 0;
  const isYes = outcome === 'yes';
  const isPending = isSubmitting || placeOrder.isPending;

  // Auto-load first credential for external markets
  useEffect(() => {
    if (!isExternal) return;

    let canceled = false;
    setCredentialLoading(true);

    api
      .getExternalCredentials(provider)
      .then((list) => {
        if (!canceled && list.length > 0) {
          setCredential(list[0]);
        }
      })
      .catch(() => {})
      .finally(() => {
        if (!canceled) setCredentialLoading(false);
      });

    return () => {
      canceled = true;
    };
  }, [isExternal, provider]);

  const { shares, potentialReturn } = useMemo(() => {
    if (!amountValue || !effectivePrice) {
      return { shares: 0, potentialReturn: 0 };
    }
    if (side === 'buy') {
      const s = amountValue / effectivePrice;
      const ret = s * (1 - effectivePrice);
      return { shares: s, potentialReturn: ret };
    }
    return { shares: amountValue, potentialReturn: amountValue * effectivePrice };
  }, [amountValue, effectivePrice, side]);

  const handleSubmitExternal = async () => {
    if (!walletReady) {
      addToast('Connect wallet to place order', 'error');
      return;
    }
    const numericPrice = effectivePrice;

    if (!numericPrice || numericPrice <= 0 || numericPrice >= 1) {
      addToast('Price must be between 0 and 1', 'error');
      return;
    }
    if (!amountValue || amountValue <= 0) {
      addToast('Amount must be greater than 0', 'error');
      return;
    }
    if (!credential) {
      addToast('No venue credential is ready for this market', 'error');
      return;
    }

    setIsSubmitting(true);
    try {
      const intent = await api.post<ExternalOrderIntent>('/external/orders/intent', {
        provider,
        marketId: market.id,
        outcome,
        side,
        price: numericPrice,
        quantity: amountValue,
        credentialId: credential?.id,
      });

      // API may return camelCase or snake_case
      const raw = intent as unknown as Record<string, unknown>;
      const typedData = (raw.typedData ?? raw.typed_data) as Record<string, unknown>;

      const ethereum = (
        window as unknown as {
          ethereum?: { request: (args: Record<string, unknown>) => Promise<unknown> };
        }
      ).ethereum;

      if (!ethereum || !baseWallet.address) {
        throw new Error('Connect wallet to place order');
      }

      const signature = await ethereum.request({
        method: 'eth_signTypedData_v4',
        params: [baseWallet.address, JSON.stringify(typedData)],
      });

      await api.post('/external/orders/submit', {
        intentId: intent.id,
        signedOrder: { typedData, signature: String(signature || '') },
        credentialId: credential?.id,
      });

      addToast('Order placed!', 'success');
      setAmount('');
      setPrice('');
      onSuccess?.();
    } catch (err: unknown) {
      let message = 'Order failed';
      if (err instanceof Error) {
        message = err.message;
      }
      // ApiError.message is "[object Object]" when API returns { error: { code, message } }
      if (message.includes('[object')) message = 'Order failed — please try again';
      addToast(message, 'error');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleSubmitInternal = async () => {
    if (!amountValue || amountValue <= 0) {
      addToast('Amount must be greater than 0', 'error');
      return;
    }

    try {
      await placeOrder.mutateAsync({
        marketId: market.id,
        side,
        outcome,
        orderType: price ? 'limit' : 'market',
        price: price ? parseFloat(price) : undefined,
        quantity: amountValue,
      });
      addToast('Order placed!', 'success');
      setAmount('');
      setPrice('');
      onSuccess?.();
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Order failed';
      addToast(message, 'error');
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (isExternal) {
      await handleSubmitExternal();
    } else {
      await handleSubmitInternal();
    }
  };

  return (
    <Card className="!p-4 sm:!p-6 relative">
      {isPending && (
        <div className="absolute inset-0 bg-bg-base/80 z-10 flex flex-col items-center justify-center gap-3">
          <Spinner size="lg" className={isYes ? 'text-bid' : 'text-ask'} />
          <div className="text-center">
            <p className="font-medium text-text-primary">Confirming transaction...</p>
            <p className="text-sm text-text-muted mt-1">Waiting for confirmation</p>
          </div>
        </div>
      )}

      <h3 className="font-display font-semibold text-lg mb-4">Trade</h3>

      {/* Outcome selector */}
      <div className="grid grid-cols-2 gap-2 mb-4">
        <button
          type="button"
          onClick={() => setOutcome('yes')}
          disabled={isPending}
          className={cn(
            'py-3 font-semibold text-center transition-all duration-fast',
            'border-2 cursor-pointer',
            'disabled:cursor-not-allowed disabled:opacity-50',
            isYes
              ? 'bg-bid-muted border-bid text-bid'
              : 'bg-bg-secondary border-border text-text-secondary hover:border-border-hover',
          )}
        >
          <div className="font-mono text-xl">{Math.round(market.yesPrice * 100)}¢</div>
          <div className="text-xs mt-0.5 opacity-80">Yes</div>
        </button>
        <button
          type="button"
          onClick={() => setOutcome('no')}
          disabled={isPending}
          className={cn(
            'py-3 font-semibold text-center transition-all duration-fast',
            'border-2 cursor-pointer',
            'disabled:cursor-not-allowed disabled:opacity-50',
            !isYes
              ? 'bg-ask-muted border-ask text-ask'
              : 'bg-bg-secondary border-border text-text-secondary hover:border-border-hover',
          )}
        >
          <div className="font-mono text-xl">{Math.round(market.noPrice * 100)}¢</div>
          <div className="text-xs mt-0.5 opacity-80">No</div>
        </button>
      </div>

      {/* Buy/Sell tabs */}
      <Tabs
        tabs={[
          { value: 'buy', label: 'Buy' },
          { value: 'sell', label: 'Sell' },
        ]}
        value={side}
        onChange={(v) => setSide(v as OrderSide)}
        disabled={isPending}
        className="mb-4"
      />

      <form onSubmit={handleSubmit}>
        <div className="space-y-4 mb-4">
          {!walletReady ? (
            <div className="border border-border bg-bg-secondary p-3 text-sm text-text-secondary">
              Connect your Base wallet from the miniapp shell before you place a trade.
            </div>
          ) : null}

