'use client';

import { useState } from 'react';
import { Button, useToast } from '@/components/ui';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { useResolveDistributionMarket } from '@/hooks/useDistribution';
import type { DistributionMarket } from '@/types/distribution';

interface DistributionResolvePanelProps {
  market: DistributionMarket;
  onResolved: () => void;
}

export function DistributionResolvePanel({ market, onResolved }: DistributionResolvePanelProps) {
  const [value, setValue] = useState('');
  const [showConfirm, setShowConfirm] = useState(false);
  const { addToast } = useToast();
  const resolveMutation = useResolveDistributionMarket();

  const numValue = parseFloat(value);
  const isValid = !isNaN(numValue) && numValue >= market.outcomeMin && numValue <= market.outcomeMax;

  const handleResolve = async () => {
    if (!isValid) return;
    setShowConfirm(false);
    try {
      await resolveMutation.mutateAsync({ marketId: market.id, value: numValue });
      addToast('Market resolved successfully', 'success');
      onResolved();
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Resolution failed';
      addToast(msg, 'error');
    }
  };

  if (market.status === 'resolved') {
    return (
      <div className="border border-border bg-bg-secondary p-4">
        <div className="text-xs text-text-secondary uppercase tracking-wide mb-2">
          Resolved Outcome
        </div>
        <div className="font-mono tabular-nums text-2xl text-accent">
          {market.resolvedValue?.toFixed(4)}
          {market.outcomeUnit && (
            <span className="text-sm text-text-secondary ml-2">{market.outcomeUnit}</span>
          )}
        </div>
        {market.resolvedAt && (
          <div className="text-xs text-text-muted mt-2">
            Resolved {new Date(market.resolvedAt).toLocaleString()}
          </div>
        )}
      </div>
    );
  }

  if (market.status === 'cancelled') return null;

  return (
    <>
      <div className="border border-border bg-bg-secondary p-4 space-y-4">
        <div>
          <div className="text-sm font-medium text-text-primary">Resolve this market</div>
          <div className="mt-1 text-xs text-text-secondary">
            Enter the actual outcome value to resolve and calculate payouts.
          </div>
        </div>

        <div className="space-y-2">
          <label className="text-xs text-text-secondary uppercase tracking-wide block">
            Outcome Value
          </label>
          <input
            type="number"
            step="any"
            min={market.outcomeMin}
            max={market.outcomeMax}
            value={value}
            onChange={(e) => setValue(e.target.value)}
            placeholder={`${market.outcomeMin} — ${market.outcomeMax}`}
            className="w-full h-10 px-3 py-2 bg-bg-primary border border-border text-text-primary font-mono tabular-nums text-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:border-accent"
          />
          <div className="flex justify-between text-xs text-text-muted font-mono tabular-nums">
            <span>Min: {market.outcomeMin}</span>
            <span>Max: {market.outcomeMax}</span>
          </div>
          {value && !isValid && (
            <div className="text-xs text-ask">
              Value must be between {market.outcomeMin} and {market.outcomeMax}
            </div>
          )}
        </div>

        <Button
          variant="primary"
          size="lg"
          className="w-full"
          onClick={() => setShowConfirm(true)}
          disabled={!isValid || resolveMutation.isPending}
          loading={resolveMutation.isPending}
        >
          {resolveMutation.isPending ? 'Resolving...' : 'Resolve Market'}
        </Button>
      </div>

      {/* Confirmation Dialog */}
      <Dialog open={showConfirm} onOpenChange={setShowConfirm}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Resolve Market</DialogTitle>
            <DialogDescription>
              This action is irreversible. All open positions will be settled at the specified value.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3 py-2">
            <div className="text-sm text-text-primary font-medium line-clamp-2">
              {market.question}
            </div>
            <div className="border border-border bg-bg-primary p-3">
              <div className="text-xs text-text-secondary uppercase tracking-wide mb-1">
                Resolution Value
              </div>
              <div className="font-mono tabular-nums text-xl text-text-primary">
                {numValue}
                {market.outcomeUnit && (
                  <span className="text-sm text-text-secondary ml-2">{market.outcomeUnit}</span>
                )}
              </div>
            </div>
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setShowConfirm(false)}>
              Cancel
            </Button>
            <Button variant="ask" onClick={handleResolve} loading={resolveMutation.isPending}>
              Resolve Market
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
