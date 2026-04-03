'use client';

import Link from 'next/link';
import { useEffect, useMemo, useState } from 'react';
import { usePathname, useRouter, useSearchParams } from 'next/navigation';

import { CreatorMarketDetail } from '@/components/portfolio/CreatorMarketDetail';
import { CreatorMarketsTable } from '@/components/portfolio/CreatorMarketsTable';
import { CreatorOverviewCards } from '@/components/portfolio/CreatorOverviewCards';
import { Card, StatCardSkeleton } from '@/components/ui';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import {
  useCreatorEconomicsMarket,
  useCreatorEconomicsMarkets,
  useCreatorEconomicsOverview,
  useSessionState,
} from '@/hooks';
import { ApiError } from '@/lib/api';

function isUnavailableError(error: unknown): boolean {
  return error instanceof ApiError && [404, 405, 501].includes(error.status);
}

export function CreatorDashboardPage() {
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const { isConnected } = useBaseWallet();
  const { hasSession, sessionRestored } = useSessionState();

  const enabled = isConnected && hasSession && sessionRestored;
  const [range, setRange] = useState<'7d' | '30d' | '90d'>('30d');

  const overviewQuery = useCreatorEconomicsOverview({ enabled });
  const marketsQuery = useCreatorEconomicsMarkets({ enabled });
  const markets = marketsQuery.data?.data ?? [];
  const requestedMarketId = searchParams.get('marketId');
  const selectedMarketId =
    markets.length === 0
      ? null
      : requestedMarketId &&
          markets.some((market) => market.marketId === requestedMarketId)
        ? requestedMarketId
        : markets[0].marketId;

  const selectedSummary = useMemo(
    () => markets.find((market) => market.marketId === selectedMarketId),
    [markets, selectedMarketId],
  );

  useEffect(() => {
    if (!selectedMarketId || requestedMarketId === selectedMarketId) {
      return;
    }

    const params = new URLSearchParams(searchParams.toString());
    params.set('marketId', selectedMarketId);
    router.replace(`${pathname}?${params.toString()}`, { scroll: false });
  }, [pathname, requestedMarketId, router, searchParams, selectedMarketId]);

  const detailQuery = useCreatorEconomicsMarket(selectedMarketId, range, {
    enabled: enabled && !!selectedMarketId,
  });

  const unavailable =
    isUnavailableError(overviewQuery.error) || isUnavailableError(marketsQuery.error);

  const loadError =
    overviewQuery.error instanceof ApiError
      ? overviewQuery.error
      : marketsQuery.error instanceof ApiError
        ? marketsQuery.error
        : null;

  const detailError =
    detailQuery.error instanceof ApiError ? detailQuery.error : null;

  const handleSelectMarket = (marketId: string) => {
    const params = new URLSearchParams(searchParams.toString());
    params.set('marketId', marketId);
    router.replace(`${pathname}?${params.toString()}`, { scroll: false });
  };

  if (!isConnected) {
    return (
      <section className="flex min-h-[60vh] items-center justify-center">
        <Card className="max-w-xl text-center">
          <h1 className="text-2xl font-semibold text-text-primary">
            Creator dashboard
          </h1>
          <p className="mt-3 text-text-secondary">
            Connect your Base wallet to inspect private creator economics,
            market-level burn, and liquidity ROI.
          </p>
          <div className="mt-5 flex flex-wrap justify-center gap-3">
            <Link
              href="/portfolio"
              className="inline-flex h-10 items-center border border-border px-4 text-sm text-text-secondary transition-colors hover:border-border-hover hover:text-text-primary"
            >
              Back to portfolio
            </Link>
            <Link
              href="/markets"
              className="inline-flex h-10 items-center border border-accent px-4 text-sm text-accent transition-colors hover:bg-accent/10"
            >
              Browse markets
            </Link>
          </div>
        </Card>
      </section>
    );
  }

  if (!hasSession && sessionRestored) {
    return (
      <section className="flex min-h-[60vh] items-center justify-center">
        <Card className="max-w-xl text-center">
          <h1 className="text-2xl font-semibold text-text-primary">
            Finish sign-in
          </h1>
          <p className="mt-3 text-text-secondary">
            Creator economics are private. Approve the wallet sign-in prompt,
            then reload this page.
          </p>
        </Card>
      </section>
    );
  }

  if (!sessionRestored) {
    return (
      <section className="flex min-h-[60vh] items-center justify-center">
        <Card className="max-w-xl text-center">
          <h1 className="text-2xl font-semibold text-text-primary">
            Creator dashboard
          </h1>
          <p className="mt-3 text-text-secondary">
            Restoring your session before loading private creator economics.
          </p>
        </Card>
      </section>
    );
  }

  if (unavailable) {
    return (
      <section className="space-y-4">
        <header>
          <h1 className="text-2xl font-bold text-text-primary">
            Creator dashboard
          </h1>
          <p className="mt-2 max-w-2xl text-text-secondary">
            Private economics for markets you launched. This deployment does
            not expose the creator dashboard API yet.
          </p>
        </header>
        <Card>
          <h2 className="text-lg font-semibold text-text-primary">
            Creator economics unavailable
          </h2>
          <p className="mt-2 text-sm text-text-secondary">
            The web client is ready for the private creator endpoints, but the
            backend route is not available here yet.
          </p>
        </Card>
      </section>
    );
  }

  return (
    <section className="space-y-6">
      <header>
        <h1 className="text-2xl font-bold text-text-primary">Creator dashboard</h1>
        <p className="mt-2 max-w-2xl text-text-secondary">
          Private economics for your bootstrapped markets. Public market pages
          stay lightweight; creator ROI and subsidy burn stay here.
        </p>
      </header>

      {overviewQuery.isLoading ? (
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 6 }).map((_, index) => (
            <StatCardSkeleton key={index} />
          ))}
        </div>
      ) : overviewQuery.data ? (
        <CreatorOverviewCards overview={overviewQuery.data} />
      ) : loadError ? (
        <Card>
          <h2 className="text-lg font-semibold text-text-primary">
            Failed to load creator overview
          </h2>
          <p className="mt-2 text-sm text-text-secondary">
            {loadError.message}
          </p>
        </Card>
      ) : null}

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
        <div className="space-y-4">
          {marketsQuery.isLoading ? (
            <Card>
              <div className="space-y-3">
                {Array.from({ length: 6 }).map((_, index) => (
                  <div key={index} className="h-16 animate-pulse bg-bg-secondary" />
                ))}
              </div>
            </Card>
          ) : markets.length > 0 ? (
            <CreatorMarketsTable
              markets={markets}
              selectedMarketId={selectedMarketId}
              onSelect={handleSelectMarket}
            />
          ) : (
            <Card>
              <h2 className="text-lg font-semibold text-text-primary">
                No creator markets yet
              </h2>
              <p className="mt-2 text-sm text-text-secondary">
                Once you launch creator-funded markets, their private economics
                roll up here.
              </p>
            </Card>
          )}
        </div>

        <CreatorMarketDetail
          detail={detailQuery.data}
          selectedSummary={selectedSummary}
          isLoading={detailQuery.isLoading}
          errorMessage={detailError?.message ?? null}
          range={range}
          onRangeChange={setRange}
        />
      </div>
    </section>
  );
}
