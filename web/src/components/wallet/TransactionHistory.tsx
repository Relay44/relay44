'use client';

import { useState, useEffect, useCallback } from 'react';
import { useSessionState } from '@/hooks';
import { api } from '@/lib/api';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import type { Transaction, TransactionType } from '@/types';
import { cn } from '@/lib/utils';

function formatUsdc(amount: number): string {
  return (amount / 1_000_000).toFixed(2);
}

function formatDate(date: string): string {
  return new Date(date).toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

const TX_TYPE_CONFIG: Record<
  TransactionType,
  { label: string; color: 'default' | 'success' | 'warning' | 'danger' }
> = {
  deposit: { label: 'Deposit', color: 'success' },
  withdraw: { label: 'Withdraw', color: 'warning' },
  buy: { label: 'Buy', color: 'success' },
  sell: { label: 'Sell', color: 'warning' },
  claim: { label: 'Claim', color: 'success' },
  mint: { label: 'Mint', color: 'default' },
  redeem: { label: 'Redeem', color: 'default' },
};

export function TransactionHistory() {
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [loading, setLoading] = useState(true);
  const [hasMore, setHasMore] = useState(false);
  const [offset, setOffset] = useState(0);
  const { hasSession, sessionRestored } = useSessionState();
  const limit = 10;

  const fetchTransactions = useCallback(async (currentOffset: number, reset = false) => {
    try {
      setLoading(true);
      const offsetValue = reset ? 0 : currentOffset;
      const response = await api.getTransactions({ limit, offset: offsetValue });

      if (reset) {
        setTransactions(response.data);
        setOffset(limit);
      } else {
        setTransactions((prev) => [...prev, ...response.data]);
        setOffset(offsetValue + limit);
      }

      setHasMore(response.hasMore);
    } catch (err) {
      console.error('Failed to fetch transactions:', err);
    } finally {
      setLoading(false);
    }
  }, [limit]);

  useEffect(() => {
    if (!sessionRestored) {
      return;
    }

    if (!hasSession) {
      setTransactions([]);
      setHasMore(false);
      setOffset(0);
      setLoading(false);
      return;
    }

    void fetchTransactions(0, true);
  }, [fetchTransactions, hasSession, sessionRestored]);

  if ((!sessionRestored || loading) && transactions.length === 0) {
    return (
      <div className="flex items-center justify-center h-48">
        <div className="animate-pulse text-text-secondary">Loading...</div>
      </div>
    );
  }

  if (!hasSession) {
    return (
      <div className="flex flex-col items-center justify-center h-48 text-text-secondary">
        <p>Sign in to view transaction history</p>
        <p className="text-sm mt-1">Wallet activity appears after authentication</p>
      </div>
    );
  }

  if (transactions.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-48 text-text-secondary">
        <p>No transactions yet</p>
        <p className="text-sm mt-1">Deposits and withdrawals will appear here</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        {transactions.map((tx) => {
          const config = TX_TYPE_CONFIG[tx.txType];
          const isIncoming = tx.txType === 'deposit' || tx.txType === 'claim';

          return (
            <div
              key={tx.id}
              className="flex items-center justify-between p-3  bg-bg-secondary hover:bg-bg-tertiary transition-colors"
            >
              <div className="flex items-center gap-3">
                <div
                  className={cn(
                    'w-8 h-8  flex items-center justify-center text-sm',
                    isIncoming ? 'bg-bid/10 text-bid' : 'bg-ask/10 text-ask'
                  )}
                >
                  {isIncoming ? '+' : '-'}
                </div>
                <div>
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-text-primary">
                      {config.label}
                    </span>
                    <Badge
                      variant={
                        tx.status === 'confirmed'
                          ? 'success'
                          : tx.status === 'pending'
                          ? 'warning'
                          : 'danger'
                      }
                    >
                      {tx.status}
                    </Badge>
                  </div>
                  <p className="text-sm text-text-secondary">
                    {formatDate(tx.createdAt)}
                  </p>
                </div>
              </div>
              <div className="text-right">
                <p
                  className={cn(
                    'font-medium',
                    isIncoming ? 'text-bid' : 'text-ask'
                  )}
                >
                  {isIncoming ? '+' : '-'}${formatUsdc(tx.amount)}
                </p>
                {tx.fee > 0 && (
                  <p className="text-xs text-text-secondary">
                    Fee: ${formatUsdc(tx.fee)}
                  </p>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {hasMore && (
        <Button
          variant="secondary"
          className="w-full"
          onClick={() => fetchTransactions(offset)}
          loading={loading}
        >
          Load More
        </Button>
      )}
    </div>
  );
}

