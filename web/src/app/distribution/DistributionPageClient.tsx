'use client';

import { useMemo, useState } from 'react';
import Link from 'next/link';
import { BarChart3, Plus, TrendingUp, Wallet } from 'lucide-react';
import { Header, BottomNav } from '@/components/layout';
import { Button } from '@/components/ui';
import { DistributionMarketCard } from '@/components/distribution';
import { useDistributionMarkets } from '@/hooks/useDistribution';
import { cn } from '@/lib/utils';
import { CATEGORIES } from '@/lib/constants';

type StatusTab = 'all' | 'active' | 'resolved';

function formatCompact(value: number): string {
  if (value >= 1_000_000) return `$${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `$${Math.round(value / 1_000)}k`;
  return `$${value.toLocaleString()}`;
}

export default function DistributionPageClient() {
  const [statusTab, setStatusTab] = useState<StatusTab>('active');
  const [category, setCategory] = useState('All');

  const filters = useMemo(
    () => ({
      status: statusTab === 'all' ? undefined : statusTab,
      category: category === 'All' ? undefined : category.toLowerCase(),
      limit: 50,
    }),
    [statusTab, category],
  );

  const { data, isLoading, error } = useDistributionMarkets(filters);
  const markets = data?.data ?? [];

  const activeCount = markets.filter((m) => m.status === 'active').length;
  const totalVolume = markets.reduce((sum, m) => sum + (m.totalVolume || 0), 0);
  const totalCollateral = markets.reduce((sum, m) => sum + (m.totalCollateral || 0), 0);

  const errorMessage = error instanceof Error ? error.message : null;

  return (
    <div className="min-h-screen pt-header">
      <Header />

      {/* Sticky filter bar */}
      <div className="top-header sticky z-40 bg-bg-primary border-b border-border">
        <div className="max-w-[1400px] mx-auto px-4 sm:px-8">
          <div className="flex items-center gap-4 py-3 overflow-x-auto scrollbar-hide">
            {/* Status tabs */}
            <div className="flex items-center gap-1 flex-shrink-0">
              {(['all', 'active', 'resolved'] as StatusTab[]).map((tab) => (
                <button
                  key={tab}
                  onClick={() => setStatusTab(tab)}
                  className={cn(
                    'px-3 py-1.5 text-[0.7rem] font-medium whitespace-nowrap transition-colors cursor-pointer border',
                    statusTab === tab
                      ? 'border-accent text-accent'
                      : 'border-border text-text-secondary hover:border-border-hover',
                  )}
                >
                  {tab.toUpperCase()}
                </button>
              ))}
            </div>

            <div className="w-px h-5 bg-border flex-shrink-0" />

            {/* Category pills */}
            <div className="flex items-center gap-1.5">
              {CATEGORIES.map((cat) => (
                <button
                  key={cat}
                  onClick={() => setCategory(cat)}
                  className={cn(
                    'px-3 py-1.5 text-[0.7rem] font-medium whitespace-nowrap transition-colors cursor-pointer',
                    category === cat
                      ? 'bg-bg-tertiary text-text-primary'
                      : 'text-text-secondary hover:bg-bg-hover hover:text-text-primary',
                  )}
                >
                  {cat}
                </button>
              ))}
            </div>
          </div>
        </div>
      </div>

      <div className="max-w-[1400px] mx-auto px-4 sm:px-8 py-6">
        {/* Header + stats */}
        <div className="mb-6">
          <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between mb-4">
            <div className="space-y-1">
              <h1 className="text-2xl font-semibold text-text-primary">
                Distribution Markets
              </h1>
              <p className="text-sm text-text-secondary">
                Continuous outcome markets powered by Gaussian LMSR pricing
              </p>
            </div>
            <Link href="/distribution/create">
              <Button variant="primary" size="sm" className="flex items-center gap-1.5">
                <Plus className="w-3.5 h-3.5" />
                Create Market
              </Button>
            </Link>
          </div>

          {/* Stats row */}
          <div className="grid grid-cols-3 gap-3">
            <div className="border border-border p-3">
              <div className="flex items-center gap-2 text-xs text-text-muted uppercase tracking-wide mb-1">
                <BarChart3 className="w-3.5 h-3.5" />
                Active
              </div>
              <div className="font-mono tabular-nums text-lg text-text-primary">
                {activeCount}
              </div>
            </div>
            <div className="border border-border p-3">
              <div className="flex items-center gap-2 text-xs text-text-muted uppercase tracking-wide mb-1">
                <TrendingUp className="w-3.5 h-3.5" />
                Volume
              </div>
              <div className="font-mono tabular-nums text-lg text-text-primary">
                {formatCompact(totalVolume)}
              </div>
            </div>
            <div className="border border-border p-3">
              <div className="flex items-center gap-2 text-xs text-text-muted uppercase tracking-wide mb-1">
                <Wallet className="w-3.5 h-3.5" />
                Collateral
              </div>
              <div className="font-mono tabular-nums text-lg text-text-primary">
                {formatCompact(totalCollateral)}
              </div>
            </div>
          </div>
        </div>

        {/* Error */}
        {errorMessage && (
          <div className="mb-4 p-3 border border-ask/20 bg-ask/10 text-ask text-sm">
            {errorMessage}
          </div>
        )}

        {/* Loading */}
        {isLoading && (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
            {Array.from({ length: 8 }).map((_, i) => (
              <div key={i} className="border border-border p-4 animate-pulse">
                <div className="h-3 w-20 bg-bg-tertiary mb-3" />
                <div className="h-4 w-full bg-bg-tertiary mb-2" />
                <div className="h-4 w-2/3 bg-bg-tertiary mb-4" />
                <div className="h-12 w-full bg-bg-tertiary mb-3" />
                <div className="h-3 w-full bg-bg-tertiary" />
              </div>
            ))}
          </div>
        )}

        {/* Market grid */}
        {!isLoading && markets.length > 0 && (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
            {markets.map((market) => (
              <DistributionMarketCard key={market.id} market={market} />
            ))}
          </div>
        )}

        {/* Empty state */}
        {!isLoading && markets.length === 0 && !errorMessage && (
          <div className="text-center py-16 space-y-3">
            <p className="text-text-secondary text-sm">
              No distribution markets found
            </p>
            <p className="text-text-muted text-xs max-w-md mx-auto">
              Distribution markets let you trade continuous outcomes using probability curves.
              Check back soon for new markets.
            </p>
          </div>
        )}
      </div>

      <BottomNav />
    </div>
  );
}
