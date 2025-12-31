'use client';

import { useState, useEffect } from 'react';
import { api, ApiError } from '@/lib/api';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import { FeatureNotice } from '@/components/runtime/FeatureNotice';
import type { Trade, Outcome } from '@/types';
import { cn } from '@/lib/utils';

interface TradeLogProps {
  marketId: string;
  outcome?: Outcome;
  limit?: number;
}

export function TradeLog({ marketId, outcome, limit = 20 }: TradeLogProps) {
  const [trades, setTrades] = useState<Trade[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<'premium' | 'unavailable' | null>(null);

  useEffect(() => {
    async function fetchTrades() {
      try {
        setLoading(true);
        setError(null);
        const response = await api.getTrades(marketId, { outcome, limit });
        setTrades(response.data);
      } catch (err) {
        if (err instanceof ApiError && err.status === 402) {
          setError('premium');
        } else {
          console.error('Failed to fetch trades:', err);
          setError('unavailable');
        }
      } finally {
        setLoading(false);
      }
    }

    fetchTrades();
  }, [marketId, outcome, limit]);

  if (loading) {
    return (
      <Card>
        <CardContent className="flex items-center justify-center h-32">
          <div className="animate-pulse text-text-secondary">Loading trades...</div>
        </CardContent>
      </Card>
    );
  }

  if (error === 'premium') {
    return (
      <FeatureNotice
        title="Premium trade feed"
        body="Recent fills for this market are payment-gated on the public API."
      />
    );
  }

  if (error === 'unavailable') {
    return (
      <FeatureNotice
        title="Trade log unavailable"
        body="Recent trades could not be loaded for this market right now."
      />
    );
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Recent Trades</CardTitle>
      </CardHeader>
      <CardContent>
        {trades.length === 0 ? (
          <div className="text-center py-8 text-text-secondary">
            No trades yet
          </div>
        ) : (
          <div className="space-y-1">
            {/* Header */}
            <div className="grid grid-cols-[72px_minmax(0,1fr)_auto_auto] gap-2 border-b border-border py-2 text-[11px] text-text-secondary sm:grid-cols-4 sm:text-xs">
              <span>Time</span>
              <span>Outcome</span>
              <span className="text-right">Price</span>
              <span className="text-right">Size</span>
            </div>

            {/* Trades */}
            {trades.map((trade, index) => {
              const isYes = trade.outcome === 'yes';

              return (
                <div
                  key={trade.id}
                  className={cn(
                    'grid grid-cols-[72px_minmax(0,1fr)_auto_auto] gap-2 py-2 text-xs sm:grid-cols-4 sm:text-sm',
                    index % 2 === 0 ? 'bg-bg-secondary/50' : ''
                  )}
                >
                  <span className="truncate text-text-secondary">
                    {formatTime(trade.createdAt)}
                  </span>
                  <span className={isYes ? 'text-bid' : 'text-ask'}>
                    {trade.outcome.toUpperCase()}
                  </span>
                  <span className="text-right text-text-primary">
                    {trade.price.toFixed(1)}%
                  </span>
                  <span className="text-right text-text-secondary">
                    {formatQuantity(trade.quantity)}
                  </span>
                </div>
              );
