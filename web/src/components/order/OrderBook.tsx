'use client';

import { useEffect, useState } from 'react';

import { Card, Tabs, LoadingScreen } from '@/components/ui';
import { useOrderBook } from '@/hooks';
import { FeatureNotice } from '@/components/runtime/FeatureNotice';
import { PaymentGate } from '@/components/payments';
import {
  ApiError,
  type BaseOrderBookResponse,
  normalizeBaseOrderBookResponse,
} from '@/lib/api';
import { formatPrice, formatTimeAgo } from '@/lib/utils';
import type { OrderBook, OrderBookLevel, Outcome } from '@/types';

export interface OrderBookProps {
  marketId: string;
}

function depthSummary(orderBook: OrderBook | undefined): string | null {
  if (!orderBook) {
    return null;
  }

  const provenance: string[] = [];
  if (orderBook.includesBootstrap) {
    provenance.push(
      `bootstrap ${formatPrice(orderBook.bootstrapDepth || 0)}`
    );
    if (
      orderBook.bootstrapInventoryYesUsdc != null ||
      orderBook.bootstrapInventoryNoUsdc != null
    ) {
      provenance.push(
        `inventory Y ${formatPrice(orderBook.bootstrapInventoryYesUsdc || 0)} / N ${formatPrice(orderBook.bootstrapInventoryNoUsdc || 0)}`
      );
    }
  }

  if (orderBook.includesMirror && orderBook.includesBootstrap) {
    const mirrorBits = [
      `mirror ${formatPrice(orderBook.mirrorDepth || 0)}`,
      typeof orderBook.mirrorActiveLinkCount === 'number' &&
        typeof orderBook.mirrorLinkCount === 'number'
        ? `links ${orderBook.mirrorActiveLinkCount}/${orderBook.mirrorLinkCount}`
        : null,
      orderBook.mirrorLastMirrorAt
        ? `fresh ${formatTimeAgo(orderBook.mirrorLastMirrorAt)}`
        : null,
      orderBook.mirrorLastHedgeAt
        ? `hedged ${formatTimeAgo(orderBook.mirrorLastHedgeAt)}`
        : null,
      typeof orderBook.mirrorPendingHedges === 'number'
        ? `${orderBook.mirrorPendingHedges} hedges pending`
        : null,
      typeof orderBook.mirrorLinksWithErrors === 'number' &&
        orderBook.mirrorLinksWithErrors > 0
        ? `${orderBook.mirrorLinksWithErrors} mirror errors`
        : null,
    ].filter(Boolean);
    return `Cross-venue depth: organic $${formatPrice(orderBook.organicDepth || 0)} | ${provenance.join(' | ')} | ${mirrorBits.join(' | ')}`;
  }

  if (orderBook.includesMirror) {
    const mirrorBits = [
      `mirror ${formatPrice(orderBook.mirrorDepth || 0)}`,
      typeof orderBook.mirrorActiveLinkCount === 'number' &&
        typeof orderBook.mirrorLinkCount === 'number'
        ? `links ${orderBook.mirrorActiveLinkCount}/${orderBook.mirrorLinkCount}`
        : null,
      orderBook.mirrorLastMirrorAt
        ? `fresh ${formatTimeAgo(orderBook.mirrorLastMirrorAt)}`
        : null,
      orderBook.mirrorLastHedgeAt
        ? `hedged ${formatTimeAgo(orderBook.mirrorLastHedgeAt)}`
        : null,
      typeof orderBook.mirrorPendingHedges === 'number'
        ? `${orderBook.mirrorPendingHedges} hedges pending`
        : null,
      typeof orderBook.mirrorLinksWithErrors === 'number' &&
        orderBook.mirrorLinksWithErrors > 0
        ? `${orderBook.mirrorLinksWithErrors} mirror errors`
        : null,
    ].filter(Boolean);
    return `Cross-venue depth: organic $${formatPrice(orderBook.organicDepth || 0)} | ${mirrorBits.join(' | ')}`;
  }

  if (orderBook.includesBootstrap) {
    return `Unified depth includes bootstrap quotes. Organic $${formatPrice(orderBook.organicDepth || 0)} | ${provenance.join(' | ')}`;
  }

  return `Organic depth $${formatPrice(orderBook.organicDepth || 0)}`;
}

export function OrderBookDisplay({ marketId }: OrderBookProps) {
  const [outcome, setOutcome] = useState<Outcome>('yes');
  const [paidSnapshots, setPaidSnapshots] = useState<
    Partial<Record<Outcome, OrderBook>>
  >({});
  const { data: orderBook, isLoading, error } = useOrderBook(marketId, outcome);
  const paidOrderBook = paidSnapshots[outcome];
  const summary = depthSummary(orderBook ?? paidOrderBook);

  useEffect(() => {
    setPaidSnapshots({});
  }, [marketId]);

  if (error instanceof ApiError && error.status === 402) {
    if (paidOrderBook) {
      return (
        <Card>
          <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <h3 className="font-semibold">Order Book</h3>
              <p className="mt-1 text-xs text-text-secondary">
                Paid snapshot. Refreshing depth requires another x402 payment.
              </p>
              {summary ? (
                <p className="mt-1 text-xs text-text-secondary">{summary}</p>
              ) : null}
            </div>
            <div className="flex items-center gap-3">
              <Tabs
                tabs={[
                  { value: 'yes', label: 'Yes' },
                  { value: 'no', label: 'No' },
                ]}
                value={outcome}
                onChange={(value) => setOutcome(value as Outcome)}
              />
              <button
                type="button"
                className="text-xs text-text-secondary underline-offset-4 hover:underline"
                onClick={() =>
                  setPaidSnapshots((current) => ({
                    ...current,
                    [outcome]: undefined,
                  }))
                }
              >
                Load fresh snapshot
              </button>
            </div>
          </div>

          <OrderBookTable bids={paidOrderBook.bids} asks={paidOrderBook.asks} />
        </Card>
      );
    }

    return (
      <PaymentGate
        title="Premium order book"
        body="Full depth is payment-gated. Pay once to load a live snapshot for this side of the market."
        resource="orderbook"
        resourcePath={`/evm/markets/${marketId}/orderbook?outcome=${outcome}&depth=20`}
        onPaidData={(data) => {
          if (!data || typeof data !== 'object') {
            return;
          }
          setPaidSnapshots((current) => ({
            ...current,
            [outcome]: normalizeBaseOrderBookResponse(
              data as BaseOrderBookResponse,
            ),
          }));
        }}
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
          {summary ? (
            <p className="mt-1 text-xs text-text-secondary">{summary}</p>
          ) : null}
        </div>
        <Tabs
          tabs={[
            { value: 'yes', label: 'Yes' },
            { value: 'no', label: 'No' },
          ]}
          value={outcome}
          onChange={(value) => setOutcome(value as Outcome)}
        />
      </div>

      {isLoading ? (
        <LoadingScreen />
      ) : orderBook ? (
        <OrderBookTable bids={orderBook.bids} asks={orderBook.asks} />
      ) : (
        <div className="py-8 text-center text-text-secondary">No orders yet</div>
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
    ...bids.map((bid) => bid.quantity),
    ...asks.map((ask) => ask.quantity),
    1,
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
          {asks
            .slice(0, 5)
            .reverse()
            .map((level, index) => (
              <OrderBookRow
                key={`ask-${index}`}
                level={level}
                side="ask"
                maxQuantity={maxQuantity}
              />
            ))}
        </div>
      </div>

      <div className="border-t border-border py-2 text-center">
        <span className="text-sm text-text-secondary">Spread</span>
      </div>

      <div className="space-y-1">
        {bids.slice(0, 5).map((level, index) => (
          <OrderBookRow
            key={`bid-${index}`}
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
