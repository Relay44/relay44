'use client';

import { cn } from '@/lib/utils';
import { Badge } from '@/components/ui';
import { formatDate } from '@/lib/utils';

export interface DistributionStatsProps {
  stiffness?: number;
  muPerUnit?: number;
  sigmaPerUnit?: number;
  peakDensity?: number;
  headroomPct?: number;
  lambda?: number;
  status: string;
  tradingEnd?: string;
  createdAt: string;
  className?: string;
}

function formatStatValue(value: number | undefined, decimals = 4): string {
  if (value === undefined || value === null) return '--';
  return value.toFixed(decimals);
}

const STATUS_VARIANTS: Record<string, 'success' | 'warning' | 'danger' | 'secondary'> = {
  active: 'success',
  paused: 'warning',
  closed: 'danger',
  resolved: 'secondary',
  cancelled: 'danger',
};

export function DistributionStats({
  stiffness,
  muPerUnit,
  sigmaPerUnit,
  peakDensity,
  headroomPct,
  lambda,
  status,
  tradingEnd,
  createdAt,
  className,
}: DistributionStatsProps) {
  return (
    <div className={cn('grid grid-cols-1 sm:grid-cols-3 gap-4', className)}>
      {/* Column 1 — STIFFNESS (LOCAL) */}
      <div className="border border-border p-4 space-y-3">
        <h4 className="text-xs text-text-secondary uppercase tracking-wide font-medium">
          Stiffness (Local)
        </h4>
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-xs text-text-secondary">S</span>
            <span className="font-mono tabular-nums text-sm text-text-primary">
              {formatStatValue(stiffness, 2)}
            </span>
          </div>
          <div className="w-full h-0.5 bg-border">
            <div
              className="h-full bg-accent transition-all"
              style={{ width: `${Math.min((stiffness ?? 0) * 10, 100)}%` }}
            />
          </div>

          <div className="flex items-center justify-between">
            <span className="text-xs text-text-secondary">{'\u03BC'} per +1</span>
            <span className="font-mono tabular-nums text-sm text-text-primary">
              {formatStatValue(muPerUnit, 4)}
            </span>
          </div>
          <div className="w-full h-0.5 bg-border">
            <div
              className="h-full bg-bid transition-all"
              style={{ width: `${Math.min(Math.abs(muPerUnit ?? 0) * 100, 100)}%` }}
            />
          </div>

          <div className="flex items-center justify-between">
            <span className="text-xs text-text-secondary">{'\u03C3'} per +1</span>
            <span className="font-mono tabular-nums text-sm text-text-primary">
              {formatStatValue(sigmaPerUnit, 4)}
            </span>
          </div>
          <div className="w-full h-0.5 bg-border">
            <div
              className="h-full bg-ask transition-all"
              style={{ width: `${Math.min(Math.abs(sigmaPerUnit ?? 0) * 100, 100)}%` }}
            />
          </div>
        </div>
      </div>

      {/* Column 2 — CAP & SCALE (lambda) */}
      <div className="border border-border p-4 space-y-3">
        <h4 className="text-xs text-text-secondary uppercase tracking-wide font-medium">
          Cap & Scale ({'\u03BB'})
        </h4>
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-xs text-text-secondary">Peak {'\u03C1'}</span>
            <span className="font-mono tabular-nums text-sm text-text-primary">
              {formatStatValue(peakDensity, 6)}
            </span>
          </div>

          <div className="flex items-center justify-between">
            <span className="text-xs text-text-secondary">Headroom</span>
            <span className="font-mono tabular-nums text-sm text-text-primary">
              {headroomPct !== undefined ? `${headroomPct.toFixed(1)}%` : '--'}
            </span>
          </div>

          <div className="flex items-center justify-between">
            <span className="text-xs text-text-secondary">{'\u03BB'}</span>
            <span className="font-mono tabular-nums text-sm text-text-primary">
              {formatStatValue(lambda, 4)}
            </span>
          </div>
        </div>
      </div>

      {/* Column 3 — LIFECYCLE */}
      <div className="border border-border p-4 space-y-3">
        <h4 className="text-xs text-text-secondary uppercase tracking-wide font-medium">
          Lifecycle
        </h4>
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-xs text-text-secondary">Status</span>
            <Badge variant={STATUS_VARIANTS[status] ?? 'secondary'}>
              {status.charAt(0).toUpperCase() + status.slice(1)}
            </Badge>
          </div>

          <div className="flex items-center justify-between">
            <span className="text-xs text-text-secondary">Expires</span>
            <span className="font-mono tabular-nums text-sm text-text-primary">
              {tradingEnd ? formatDate(tradingEnd) : '--'}
            </span>
          </div>

          <div className="flex items-center justify-between">
            <span className="text-xs text-text-secondary">Created</span>
            <span className="font-mono tabular-nums text-sm text-text-primary">
              {formatDate(createdAt)}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
