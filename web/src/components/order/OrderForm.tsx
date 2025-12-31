'use client';

import { useState, useMemo } from 'react';
import { Button, Input, Card, Tabs, Spinner, useToast } from '@/components/ui';
import { usePlaceOrder } from '@/hooks';
import { formatPrice, cn } from '@/lib/utils';
import type { Market, Outcome, OrderSide } from '@/types';

export interface OrderFormProps {
  market: Market;
  onSuccess?: () => void;
}

export function OrderForm({ market, onSuccess }: OrderFormProps) {
  const [outcome, setOutcome] = useState<Outcome>('yes');
  const [side, setSide] = useState<OrderSide>('buy');
  const [amount, setAmount] = useState('');
  const [price, setPrice] = useState('');
  const [errors, setErrors] = useState<Record<string, string>>({});

  const placeOrder = usePlaceOrder();
  const { addToast } = useToast();

  const currentPrice = outcome === 'yes' ? market.yesPrice : market.noPrice;

  const effectivePrice = price ? parseFloat(price) : currentPrice;
  const amountValue = parseFloat(amount) || 0;

  const { shares, potentialReturn } = useMemo(() => {
    if (!amountValue || !effectivePrice) {
      return { shares: 0, potentialReturn: 0 };
    }

    if (side === 'buy') {
      const s = amountValue / effectivePrice;
      const ret = s * (1 - effectivePrice);
      return { shares: s, potentialReturn: ret };
    } else {
      return { shares: amountValue, potentialReturn: amountValue * effectivePrice };
    }
  }, [amountValue, effectivePrice, side]);

  const validate = (): boolean => {
    const newErrors: Record<string, string> = {};

    if (!amountValue || amountValue <= 0) {
      newErrors.amount = 'Amount must be greater than 0';
    }

    if (price) {
      const priceVal = parseFloat(price);
      if (priceVal < 0.01 || priceVal > 0.99) {
        newErrors.price = 'Price must be between 0.01 and 0.99';
      }
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!validate()) return;

    try {
      await placeOrder.mutateAsync({
        marketId: market.id,
        side,
        outcome,
        orderType: price ? 'limit' : 'market',
        price: price ? parseFloat(price) : undefined,
        quantity: amountValue,
      });
      setAmount('');
      setPrice('');
      setErrors({});
      addToast('Order placed successfully', 'success');
      onSuccess?.();
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Order failed';
      addToast(message, 'error');
    }
  };

  const isYes = outcome === 'yes';
  const isPending = placeOrder.isPending;

  return (
    <Card className="!p-4 sm:!p-6 relative">
      {/* Transaction pending overlay */}
      {isPending && (
        <div className="absolute inset-0 bg-bg-base/80   z-10 flex flex-col items-center justify-center gap-3">
          <Spinner size="lg" className={isYes ? 'text-bid' : 'text-ask'} />
          <div className="text-center">
            <p className="font-medium text-text-primary">Confirming transaction...</p>
            <p className="text-sm text-text-muted mt-1">Waiting for blockchain confirmation</p>
          </div>
        </div>
      )}

      <h3 className="font-display font-semibold text-lg mb-4">Trade</h3>

      {/* Outcome selector - Yes/No */}
      <div className="grid grid-cols-2 gap-2 mb-4">
        <button
          type="button"
          onClick={() => setOutcome('yes')}
          disabled={isPending}
          className={cn(
            "py-3  font-semibold text-center transition-all duration-fast",
            "border-2 cursor-pointer",
            "disabled:cursor-not-allowed disabled:opacity-50",
            isYes
              ? "bg-bid-muted border-bid text-bid"
              : "bg-bg-secondary border-border text-text-secondary hover:border-border-hover"
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
            "py-3  font-semibold text-center transition-all duration-fast",
            "border-2 cursor-pointer",
            "disabled:cursor-not-allowed disabled:opacity-50",
            !isYes
              ? "bg-ask-muted border-ask text-ask"
              : "bg-bg-secondary border-border text-text-secondary hover:border-border-hover"
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
          <Input
            type="number"
            label={side === 'buy' ? 'Amount (USDC)' : 'Shares to sell'}
            placeholder="0.00"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            min="0"
            step="0.01"
            error={errors.amount}
            disabled={isPending}
          />

