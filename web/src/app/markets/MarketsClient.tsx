'use client';

import { useMemo, useState } from 'react';
import { Clock } from 'lucide-react';
import { Header, BottomNav } from '@/components/layout';
import { MarketList } from '@/components/market';
import { DistributionMarketCard } from '@/components/distribution';
import { useMarkets, useDistributionMarkets } from '@/hooks';
import { cn } from '@/lib/utils';
import { CATEGORIES } from '@/lib/constants';
import type { Market, MarketFilters, PaginatedResponse } from '@/types';
import type { DistributionMarket } from '@/types/distribution';

type SortTab = 'new' | 'ending';
type SourceTab = 'all' | 'internal' | 'limitless' | 'polymarket' | 'distribution';

interface MarketsClientProps {
  initialCategory?: string;
  initialMarkets?: PaginatedResponse<Market> | null;
  initialSearchQuery?: string;
}

function normalizeCategory(input: string | undefined): string {
  const normalized = (input || '').trim().toLowerCase();
  if (!normalized) return 'All';
  const fromCategories = CATEGORIES.find(
    (entry) => entry.toLowerCase() === normalized
  );
  return fromCategories || 'All';
}

export default function MarketsClient({
  initialCategory,
  initialMarkets,
  initialSearchQuery,
}: MarketsClientProps) {
  const [category, setCategory] = useState(normalizeCategory(initialCategory));
  const [sortTab, setSortTab] = useState<SortTab>('new');
  const [sourceTab, setSourceTab] = useState<SourceTab>('all');
  const [includeLowLiquidity, setIncludeLowLiquidity] = useState(false);
  const searchQuery = initialSearchQuery?.trim() || '';
  const normalizedSearchQuery = searchQuery.toLowerCase();

  const isDistOnly = sourceTab === 'distribution';

  const filters: MarketFilters = {
    source: isDistOnly ? 'all' : sourceTab as Exclude<SourceTab, 'distribution'>,
    category: category === 'All' ? undefined : category.toLowerCase(),
    sort: sortTab === 'new' ? 'newest' : 'ending',
    includeLowLiquidity,
    limit: 50,
  };

  const distFilters = useMemo(() => ({
    category: category === 'All' ? undefined : category.toLowerCase(),
    limit: 50,
  }), [category]);

  const defaultInitialData = useMemo(() => {
    if (!initialMarkets) return undefined;
    if (category !== 'All' || sortTab !== 'new' || sourceTab !== 'all') {
      return undefined;
    }
    return initialMarkets;
  }, [initialMarkets, category, sortTab, sourceTab]);

  const { data, isLoading, error } = useMarkets(filters, {
    initialData: defaultInitialData,
    enabled: !isDistOnly,
  });
  const { data: distData, isLoading: distLoading, error: distError } = useDistributionMarkets(distFilters);

  const markets = data?.data || [];
  const distMarkets = distData?.data || [];

  const visibleMarkets = useMemo(() => {
    if (isDistOnly) return [];
    if (!normalizedSearchQuery) return markets;

    return markets.filter((market) => {
      const haystack = [
        market.question,
        market.category,
        market.provider,
        market.source,
      ]
        .join(' ')
        .toLowerCase();

      return haystack.includes(normalizedSearchQuery);
    });
  }, [markets, normalizedSearchQuery, isDistOnly]);

  const visibleDistMarkets = useMemo(() => {
    if (!isDistOnly && sourceTab !== 'all') return [];
    const list = distMarkets.filter((m) => m.status === 'active');
    if (!normalizedSearchQuery) return list;
    return list.filter((m) => {
      const haystack = [m.question, m.category, m.outcomeUnit].join(' ').toLowerCase();
      return haystack.includes(normalizedSearchQuery);
    });
  }, [distMarkets, normalizedSearchQuery, isDistOnly, sourceTab]);

  const combinedLoading = isDistOnly ? distLoading : isLoading;
  const errorMessage = (error || distError) instanceof Error ? (error || distError)!.message : null;
  const totalCount = visibleMarkets.length + visibleDistMarkets.length;
  const emptyMessage = searchQuery
    ? `No markets matched "${searchQuery}"`
    : 'No markets found in this category';

  return (
    <div className="min-h-screen pt-header">
      <Header />
      <div className="top-header sticky z-40 bg-bg-primary border-b border-border">
        <div className="max-w-[1400px] mx-auto px-4 sm:px-8">
          <div className="flex items-center gap-4 py-3 overflow-x-auto scrollbar-hide">
            <div className="flex items-center gap-1 flex-shrink-0">
              <button
                onClick={() => setSortTab('new')}
                className={cn(
                  'flex items-center gap-1.5 px-3 py-1.5 text-[0.7rem] font-medium transition-colors cursor-pointer',
                  sortTab === 'new'
                    ? 'bg-accent text-text-inverse'
                    : 'text-text-secondary hover:bg-bg-hover'
                )}
              >
                <Clock className="w-3.5 h-3.5" />
                New
              </button>
              <button
                onClick={() => setSortTab('ending')}
                className={cn(
                  'flex items-center gap-1.5 px-3 py-1.5 text-[0.7rem] font-medium transition-colors cursor-pointer',
                  sortTab === 'ending'
                    ? 'bg-accent text-text-inverse'
                    : 'text-text-secondary hover:bg-bg-hover'
                )}
              >
                <Clock className="w-3.5 h-3.5" />
                Ending Soon
              </button>
            </div>

            <div className="w-px h-5 bg-border flex-shrink-0" />

            <div className="flex items-center gap-1 flex-shrink-0">
              {(['all', 'internal', 'limitless', 'polymarket', 'distribution'] as SourceTab[]).map((source) => (
                <button
                  key={source}
                  onClick={() => setSourceTab(source)}
                  className={cn(
                    'px-3 py-1.5 text-[0.7rem] font-medium whitespace-nowrap transition-colors cursor-pointer border',
                    sourceTab === source
                      ? 'border-accent text-accent'
                      : 'border-border text-text-secondary hover:border-border-hover'
                  )}
                >
                  {source.toUpperCase()}
                </button>
              ))}
            </div>

            <div className="w-px h-5 bg-border flex-shrink-0" />

            <button
              onClick={() => setIncludeLowLiquidity((current) => !current)}
              className={cn(
                'px-3 py-1.5 text-[0.7rem] font-medium whitespace-nowrap transition-colors cursor-pointer border',
                includeLowLiquidity
                  ? 'border-accent text-accent'
                  : 'border-border text-text-secondary hover:border-border-hover'
              )}
            >
              {includeLowLiquidity ? 'All Liquidity' : 'Live Liquidity'}
            </button>

            <div className="w-px h-5 bg-border flex-shrink-0" />

            <div className="flex items-center gap-1.5">
              {CATEGORIES.map((cat) => (
                <button
                  key={cat}
                  onClick={() => setCategory(cat)}
                  className={cn(
                    'px-3 py-1.5 text-[0.7rem] font-medium whitespace-nowrap transition-colors cursor-pointer',
                    category === cat
                      ? 'bg-bg-tertiary text-text-primary'
                      : 'text-text-secondary hover:bg-bg-hover hover:text-text-primary'
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
        <div className="mb-6 flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
          <div className="space-y-1">
            <h1 className="text-2xl font-semibold text-text-primary">
              {category === 'All' ? 'All Markets' : category}
            </h1>
            {searchQuery ? (
              <p className="text-sm text-text-secondary">
                Search results for "{searchQuery}"
              </p>
            ) : null}
          </div>
          <span className="text-sm text-text-muted">
            {totalCount} markets · {includeLowLiquidity ? 'including low-liquidity' : 'liquidity-filtered'}
          </span>
        </div>

        {errorMessage && (
          <div className="mb-4 p-3 border border-ask/20 bg-ask/10 text-ask text-sm">
            {errorMessage}
          </div>
        )}

        {/* Distribution market cards (shown first when in dist-only or 'all' mode) */}
        {visibleDistMarkets.length > 0 && (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 mb-4">
            {visibleDistMarkets.map((dm) => (
              <DistributionMarketCard key={`dist-${dm.id}`} market={dm} />
            ))}
          </div>
        )}

        {/* Binary market cards */}
        {!isDistOnly && (
          <MarketList
            markets={visibleMarkets}
            isLoading={combinedLoading && visibleDistMarkets.length === 0}
            columns={4}
            emptyMessage={totalCount === 0 ? emptyMessage : undefined}
          />
        )}

        {isDistOnly && !distLoading && visibleDistMarkets.length === 0 && (
          <div className="text-center py-12 text-text-muted">{emptyMessage}</div>
        )}
      </div>

      <BottomNav />
    </div>
  );
}
