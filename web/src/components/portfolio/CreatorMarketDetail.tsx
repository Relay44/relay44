'use client';

import Link from 'next/link';
import { useMemo } from 'react';

import { Badge, Card } from '@/components/ui';
import type {
  CreatorChartRange,
  CreatorEconomicsMarketDetail,
  CreatorEconomicsMarketSummary,
} from '@/types';
import {
  formatCurrency,
  formatDateTime,
} from '@/lib/utils';
import { CreatorEconomicsChart } from '@/components/portfolio/CreatorEconomicsChart';

function formatBps(bps: number): string {
  return `${bps >= 0 ? '+' : ''}${(bps / 100).toFixed(1)}%`;
}

function formatRatio(value: number): string {
  return `${(value * 100).toFixed(0)}%`;
}

function statusVariant(status?: string) {
  if (!status) {
    return 'secondary' as const;
  }

  switch (status) {
    case 'active':
    case 'bootstrapping':
      return 'success' as const;
    case 'graduated':
    case 'resolved':
      return 'accent' as const;
    case 'paused':
    case 'underfunded':
      return 'warning' as const;
    case 'error':
      return 'danger' as const;
    default:
      return 'secondary' as const;
  }
}

interface CreatorMarketDetailProps {
  detail?: CreatorEconomicsMarketDetail;
  selectedSummary?: CreatorEconomicsMarketSummary;
  isLoading?: boolean;
  errorMessage?: string | null;
  range: CreatorChartRange;
  onRangeChange: (range: CreatorChartRange) => void;
}

export function CreatorMarketDetail({
  detail,
  selectedSummary,
  isLoading,
  errorMessage,
  range,
  onRangeChange,
}: CreatorMarketDetailProps) {
  const content = useMemo(() => detail ?? selectedSummary, [detail, selectedSummary]);

  if (!content && !isLoading) {
    return (
      <Card className="flex min-h-[28rem] items-center justify-center">
        <div className="max-w-sm text-center">
          <h2 className="text-lg font-semibold text-text-primary">
            Select a creator market
          </h2>
          <p className="mt-2 text-sm text-text-secondary">
            Drill into one market to inspect seed usage, subsidy burn, ROI, and
            recent liquidity performance.
          </p>
        </div>
      </Card>
    );
  }

  if (isLoading) {
    return (
      <Card className="min-h-[28rem] animate-pulse bg-bg-primary">
        <div className="h-full space-y-4">
          <div className="h-6 w-2/3 bg-bg-secondary" />
          <div className="h-4 w-1/3 bg-bg-secondary" />
          <div className="grid gap-3 sm:grid-cols-2">
            {Array.from({ length: 6 }).map((_, index) => (
              <div key={index} className="h-20 bg-bg-secondary" />
            ))}
          </div>
          <div className="h-64 bg-bg-secondary" />
        </div>
      </Card>
    );
  }

  if (!content) {
    return null;
  }

  const detailStats = [
    { label: 'Seed', value: formatCurrency(content.seedUsdc) },
    {
      label: 'Reserved budget',
      value: formatCurrency(content.reservedBudgetUsdc),
    },
    {
      label: 'Free budget',
      value: formatCurrency(content.availableBudgetUsdc),
    },
    {
      label: 'YES inventory',
      value: formatCurrency(content.inventoryYesUsdc),
    },
    {
      label: 'NO inventory',
      value: formatCurrency(content.inventoryNoUsdc),
    },
    {
      label: 'Net inventory',
      value: formatCurrency(content.inventoryNetUsdc),
    },
    {
      label: 'Capital value',
      value: formatCurrency(content.currentCapitalValueUsdc),
    },
    {
      label: 'Bootstrap fills',
      value: formatCurrency(content.cumulativeBootstrapFillsUsdc),
    },
    {
      label: 'Organic replacement',
      value: formatRatio(content.organicReplacementRatio),
    },
    {
      label: 'Subsidy burn',
      value: formatCurrency(content.subsidyBurnUsdc),
    },
    {
      label: 'Net P&L',
      value: formatCurrency(content.netLiquidityPnlUsdc),
      accent: content.netLiquidityPnlUsdc >= 0,
    },
    {
      label: 'ROI',
      value: formatBps(content.roiBps),
      accent: content.roiBps >= 0,
    },
    {
      label: 'Mirror freshness',
      value:
        content.mirrorFreshnessSeconds == null
          ? 'N/A'
          : `${content.mirrorFreshnessSeconds}s`,
    },
    {
      label: 'Mirror backlog',
      value: content.mirrorPendingHedges.toString(),
    },
    {
      label: 'Mirror errors',
      value: content.mirrorErrorCount.toString(),
    },
    {
      label: 'Resolution P&L',
      value: formatCurrency(content.realizedResolutionPnlUsdc),
      accent: content.realizedResolutionPnlUsdc >= 0,
    },
  ];

  return (
    <div className="space-y-4">
      <Card>
        <div className="flex flex-col gap-4">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
            <div>
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant={statusVariant(content.bootstrapStatus ?? content.status)}>
                  {(content.bootstrapStatus ?? content.status).replace(/_/g, ' ')}
                </Badge>
                {content.graduationReason ? (
                  <Badge variant="secondary">
                    {content.graduationReason.replace(/_/g, ' ')}
                  </Badge>
                ) : null}
              </div>
              <h2 className="mt-3 text-xl font-semibold text-text-primary">
                {content.marketQuestion}
              </h2>
              <div className="mt-2 flex flex-wrap items-center gap-2 text-sm text-text-secondary">
                <span>Market #{content.marketId}</span>
                <span>&middot;</span>
                <span>{content.liquidityMode.replace(/_/g, ' ')}</span>
                {content.graduatedAt ? (
                  <>
                    <span>&middot;</span>
                    <span>Graduated {formatDateTime(content.graduatedAt)}</span>
                  </>
                ) : null}
              </div>
            </div>
            <Link
              href={`/markets/${encodeURIComponent(content.marketId)}`}
              className="inline-flex h-10 items-center border border-border px-4 text-sm text-text-secondary transition-colors hover:border-border-hover hover:text-text-primary"
            >
              Open market page
            </Link>
          </div>

          <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
            {detailStats.map((stat) => (
              <div key={stat.label} className="border border-border bg-bg-secondary px-4 py-3">
                <div className="text-sm text-text-secondary">{stat.label}</div>
                <div
                  className={`mt-2 text-lg font-semibold ${
                    stat.accent ? 'text-accent' : 'text-text-primary'
                  }`}
                >
                  {stat.value}
                </div>
              </div>
            ))}
          </div>

          <div className="text-xs text-text-secondary">
            Last reconciled{' '}
            {content.lastReconciledAt
              ? formatDateTime(content.lastReconciledAt)
              : 'not available'}
          </div>
        </div>
      </Card>

      {detail ? (
        <CreatorEconomicsChart
          detail={detail}
          range={range}
          onRangeChange={onRangeChange}
        />
      ) : (
        <Card className="flex h-64 items-center justify-center">
          <div className="max-w-sm text-center text-sm text-text-secondary">
            {errorMessage ||
              'The overview row loaded, but the drilldown endpoint has not returned chart data for this market yet.'}
          </div>
        </Card>
      )}
    </div>
  );
}
