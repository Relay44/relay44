'use client';

import { Card, Tabs, LoadingScreen } from '@/components/ui';
import { useOrderBook } from '@/hooks';
import { FeatureNotice } from '@/components/runtime/FeatureNotice';
import { PaymentGate } from '@/components/payments';
import { ApiError } from '@/lib/api';
import { formatPrice } from '@/lib/utils';
import type { Outcome, OrderBookLevel } from '@/types';
import { useState } from 'react';

export interface OrderBookProps {
  marketId: string;
}

export function OrderBookDisplay({ marketId }: OrderBookProps) {
  const [outcome, setOutcome] = useState<Outcome>('yes');
  const { data: orderBook, isLoading, error, refetch } = useOrderBook(marketId, outcome);

  if (error instanceof ApiError && error.status === 402) {
    return (
      <PaymentGate
        title="Premium order book"
        body="Full depth is payment-gated. Unlock premium order book access to see live depth for this market."
        resourcePath={`/evm/markets/${marketId}/orderbook`}
        onUnlocked={() => refetch()}
      />
    );
  }

  if (error) {
    return (
      <FeatureNotice
        title="Order book unavailable"
        body="The live order book could not be loaded for this market right now."
      />
    );
  }

  return (
    <Card>
      <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h3 className="font-semibold">Order Book</h3>
          {orderBook ? (
            <p className="mt-1 text-xs text-text-secondary">
              {orderBook.includesMirror && orderBook.includesBootstrap
                ? `Cross-venue depth: organic $${formatPrice(orderBook.organicDepth || 0)} | bootstrap $${formatPrice(orderBook.bootstrapDepth || 0)} | mirrored $${formatPrice(orderBook.mirrorDepth || 0)}`
                : orderBook.includesMirror
                  ? `Cross-venue depth: organic $${formatPrice(orderBook.organicDepth || 0)} | mirrored $${formatPrice(orderBook.mirrorDepth || 0)}`
                  : orderBook.includesBootstrap
                    ? `Unified depth includes bootstrap quotes. Organic $${formatPrice(orderBook.organicDepth || 0)} | bootstrap $${formatPrice(orderBook.bootstrapDepth || 0)}`
                    : `Organic depth $${formatPrice(orderBook.organicDepth || 0)}`}
            </p>
          ) : null}
        </div>
        <Tabs
          tabs={[
            { value: 'yes', label: 'Yes' },
            { value: 'no', label: 'No' },
          ]}
          value={outcome}
          onChange={(v) => setOutcome(v as Outcome)}
        />
      </div>

      {isLoading ? (
        <LoadingScreen />
      ) : orderBook ? (
        <OrderBookTable bids={orderBook.bids} asks={orderBook.asks} />
      ) : (
        <div className="text-center py-8 text-text-secondary">
          No orders yet
        </div>
      )}
    </Card>
  );
}

interface OrderBookTableProps {
  bids: OrderBookLevel[];
  asks: OrderBookLevel[];
}

function OrderBookTable({ bids, asks }: OrderBookTableProps) {
  const maxQuantity = Math.max(
    ...bids.map((b) => b.quantity),
    ...asks.map((a) => a.quantity),
    1
  );

  return (
    <div className="space-y-4">
      <div>
        <div className="mb-2 grid grid-cols-[minmax(0,1fr)_auto_auto] gap-2 text-[11px] text-text-secondary sm:text-xs">
          <span>Price</span>
          <span className="text-right">Qty</span>
          <span className="text-right">Total</span>
        </div>

        <div className="space-y-1">
          {asks.slice(0, 5).reverse().map((level, i) => (
            <OrderBookRow
              key={`ask-${i}`}
              level={level}
              side="ask"
              maxQuantity={maxQuantity}
            />
          ))}
        </div>
      </div>

      <div className="border-t border-border py-2 text-center">
        <span className="text-text-secondary text-sm">Spread</span>
      </div>

      <div className="space-y-1">
        {bids.slice(0, 5).map((level, i) => (
          <OrderBookRow
            key={`bid-${i}`}
            level={level}
            side="bid"
            maxQuantity={maxQuantity}
          />
        ))}
      </div>
    </div>
  );
}

interface OrderBookRowProps {
  level: OrderBookLevel;
  side: 'bid' | 'ask';
  maxQuantity: number;
}

function OrderBookRow({ level, side, maxQuantity }: OrderBookRowProps) {
  const barWidth = (level.quantity / maxQuantity) * 100;
  const bgColor = side === 'bid' ? 'bg-bid-muted' : 'bg-ask-muted';
  const textColor = side === 'bid' ? 'text-bid' : 'text-ask';

  return (
    <div className="relative grid grid-cols-[minmax(0,1fr)_auto_auto] gap-2 py-1 text-xs sm:text-sm">
      <div
        className={`absolute inset-y-0 ${side === 'bid' ? 'right-0' : 'left-0'} ${bgColor}`}
        style={{ width: `${barWidth}%` }}
      />
      <span className={`relative truncate ${textColor}`}>
        ${formatPrice(level.price)}
      </span>
      <span className="relative text-right">{level.quantity}</span>
      <span className="relative text-right">
        ${formatPrice(level.price * level.quantity)}
      </span>
    </div>
  );
}
