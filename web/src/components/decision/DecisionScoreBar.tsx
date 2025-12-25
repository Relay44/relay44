'use client';

import { cn } from '@/lib/utils';

interface DecisionScoreBarProps {
  label: string;
  scoreBps: number;
  rank?: number;
}

export function DecisionScoreBar({ label, scoreBps, rank }: DecisionScoreBarProps) {
  const clamped = Math.max(-10000, Math.min(10000, scoreBps));
  const magnitude = Math.min(100, Math.abs(clamped) / 100);
  const positive = clamped >= 0;

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-3 text-sm">
        <div className="min-w-0">
          <span className="font-medium text-text-primary">{label}</span>
          {typeof rank === 'number' ? (
            <span className="ml-2 text-xs uppercase tracking-[0.14em] text-text-muted">#{rank + 1}</span>
          ) : null}
        </div>
        <span className={cn('font-medium', positive ? 'text-bid' : 'text-ask')}>
          {positive ? '+' : ''}
          {(clamped / 100).toFixed(1)}%
        </span>
      </div>
      <div className="relative h-3 overflow-hidden border border-border bg-bg-secondary">
        <div className="absolute inset-y-0 left-1/2 w-px bg-border" />
        <div
          className={cn(
            'absolute inset-y-0',
            positive ? 'left-1/2 bg-bid/70' : 'right-1/2 bg-ask/70',
          )}
          style={{ width: `${magnitude / 2}%` }}
        />
      </div>
    </div>
  );
}
