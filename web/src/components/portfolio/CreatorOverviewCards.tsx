'use client';

import { Card } from '@/components/ui';
import type { CreatorEconomicsOverview } from '@/types';
import { formatCurrency } from '@/lib/utils';

function formatPercent(value: number): string {
  return `${(value * 100).toFixed(0)}%`;
}

interface CreatorOverviewCardsProps {
  overview: CreatorEconomicsOverview;
}

export function CreatorOverviewCards({
  overview,
}: CreatorOverviewCardsProps) {
  const cards = [
    {
      label: 'Active seeded markets',
      value: overview.activeSeededMarkets.toString(),
      detail: `${formatCurrency(overview.totalSeedDeployedUsdc)} deployed`,
    },
    {
      label: 'Capital value',
      value: formatCurrency(overview.currentCapitalValueUsdc),
      detail: `${formatCurrency(overview.totalSeedDeployedUsdc)} seeded`,
    },
    {
      label: 'Net liquidity P&L',
      value: formatCurrency(overview.netLiquidityPnlUsdc),
      detail: `${formatCurrency(overview.realizedResolutionPnlUsdc)} realized on resolution`,
      accent: overview.netLiquidityPnlUsdc >= 0,
    },
    {
      label: 'Subsidy burn',
      value: formatCurrency(overview.subsidyBurnUsdc),
      detail: `${overview.staleErrorMirrorCount} stale/error mirrors`,
    },
    {
      label: 'Graduation success',
      value: formatPercent(overview.graduationSuccessRate),
      detail: `${overview.creator} creator scope`,
    },
    {
      label: 'Resolution P&L',
      value: formatCurrency(overview.realizedResolutionPnlUsdc),
      detail: 'Resolved creator markets only',
      accent: overview.realizedResolutionPnlUsdc >= 0,
    },
  ];

  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
      {cards.map((card) => (
        <Card key={card.label}>
          <div className="text-sm text-text-secondary">{card.label}</div>
          <div
            className={`mt-2 text-2xl font-semibold ${
              card.accent ? 'text-accent' : 'text-text-primary'
            }`}
          >
            {card.value}
          </div>
          <div className="mt-2 text-sm text-text-secondary">{card.detail}</div>
        </Card>
      ))}
    </div>
  );
}
