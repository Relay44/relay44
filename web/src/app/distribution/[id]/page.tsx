'use client';

import { useParams } from 'next/navigation';
import { useState, useCallback, useMemo } from 'react';
import Link from 'next/link';
import { Link as LinkIcon } from 'lucide-react';
import { cn, formatCurrency, formatTimeAgo } from '@/lib/utils';
import { PageShell } from '@/components/layout';
import { Button, LoadingScreen, useToast } from '@/components/ui';
import {
  DistributionChart,
  DistributionTradePanel,
  DistributionStats,
  DistributionPositions,
  DistributionResolvePanel,
  DistributionCurveHistory,
  MobileDistributionSheet,
} from '@/components/distribution';
import { ShareCastButton } from '@/components/farcaster';
import {
  useDistributionMarket,
  useDistributionQuote,
  useDistributionCurve,
  useDistributionTrade,
  useDistributionPositions,
  useCloseDistPosition,
  useClaimDistPayout,
  useDistributionCurveHistory,
  useDistributionActivity,
} from '@/hooks/useDistribution';
import { useDistributionLiveData } from '@/hooks/useWebSocket';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { useAdminGate } from '@/hooks/useAdminGate';
import { SITE_URL } from '@/lib/seo';

export default function DistributionMarketPage() {
  const params = useParams();
  const marketId = decodeURIComponent(params.id as string);

  // Subscribe to real-time WebSocket updates for this market
  useDistributionLiveData(marketId);
  const { addToast } = useToast();
  const { address } = useBaseWallet();
  const isAdmin = useAdminGate(address);

  const { data: market, isLoading, error, refetch } = useDistributionMarket(marketId);
  const { data: positionsData } = useDistributionPositions();
  const { data: curveHistory } = useDistributionCurveHistory(marketId);
  const { data: recentActivity } = useDistributionActivity(marketId, 8);
  const tradeMutation = useDistributionTrade();
  const closeMutation = useCloseDistPosition();
  const claimMutation = useClaimDistPayout();

  // Trade state
  const [proposalMu, setProposalMu] = useState<number | null>(null);
  const [proposalSigma, setProposalSigma] = useState<number | null>(null);
  const [size, setSize] = useState(100);

  // Initialize from market data
  const mu = proposalMu ?? market?.marketMu ?? 0;
  const sigma = proposalSigma ?? market?.marketSigma ?? 1;

  // Quote (debounced via enabled flag)
  const { data: quote, isLoading: isLoadingQuote, error: quoteError } = useDistributionQuote(
    marketId,
    mu,
    sigma,
    size,
  );

  // Curve data for chart
  const { data: curveData } = useDistributionCurve(
    marketId,
    proposalMu ?? undefined,
    proposalSigma ?? undefined,
  );

  // Positions for this market
  const myPositions = useMemo(
    () => (positionsData ?? []).filter((p) => p.marketId === marketId),
    [positionsData, marketId],
  );

  const handleMuChange = useCallback((v: number) => setProposalMu(v), []);
  const handleSigmaChange = useCallback((v: number) => setProposalSigma(v), []);
  const handleSizeChange = useCallback((v: number) => setSize(v), []);

  const handleExecuteTrade = useCallback(async () => {
    if (!market) return;
    try {
      await tradeMutation.mutateAsync({ marketId, mu, sigma, size });
      addToast('Position opened successfully', 'success');
      refetch();
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Trade failed';
      addToast(msg, 'error');
    }
  }, [market, marketId, mu, sigma, size, tradeMutation, addToast, refetch]);

  const handleClose = useCallback(
    async (positionId: number) => {
      try {
        await closeMutation.mutateAsync(positionId);
        addToast('Position closed', 'success');
        refetch();
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Close failed';
        addToast(msg, 'error');
      }
    },
    [closeMutation, addToast, refetch],
  );

  const handleClaim = useCallback(
    async (positionId: number) => {
      try {
        await claimMutation.mutateAsync(positionId);
        addToast('Payout claimed', 'success');
        refetch();
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Claim failed';
        addToast(msg, 'error');
      }
    },
    [claimMutation, addToast, refetch],
  );

  const handleCopyLink = useCallback(() => {
    const url = `${SITE_URL}/distribution/${encodeURIComponent(marketId)}`;
    navigator.clipboard.writeText(url).then(
      () => addToast('Link copied', 'success'),
      () => addToast('Failed to copy link', 'error'),
    );
  }, [marketId, addToast]);

  const handleShareX = useCallback(() => {
    if (!market) return;
    const url = `${SITE_URL}/distribution/${encodeURIComponent(marketId)}`;
    const text = encodeURIComponent(`${market.question} — trade on the distribution`);
    window.open(
      `https://x.com/intent/tweet?text=${text}&url=${encodeURIComponent(url)}`,
      '_blank',
    );
  }, [market, marketId]);

  if (isLoading) return <LoadingScreen />;
  if (error || !market) {
    return (
      <PageShell>
        <div className="flex flex-col items-center justify-center py-20 gap-4">
          <p className="text-text-secondary text-sm">
            {error ? 'Failed to load market' : 'Market not found'}
          </p>
          <Link href="/distribution">
            <Button variant="secondary" size="sm">Back to Distribution Markets</Button>
          </Link>
        </div>
      </PageShell>
    );
  }

  const statusColor =
    market.status === 'active'
      ? 'text-bid'
      : market.status === 'resolved'
        ? 'text-accent'
        : 'text-text-secondary';

  const marketUrl = `${SITE_URL}/distribution/${encodeURIComponent(marketId)}`;

  const tradePanelProps = {
    market,
    quote: quote ?? null,
    isLoadingQuote,
    quoteError: quoteError instanceof Error ? quoteError : null,
    proposalMu: mu,
    proposalSigma: sigma,
    size,
    onMuChange: handleMuChange,
    onSigmaChange: handleSigmaChange,
    onSizeChange: handleSizeChange,
    onExecuteTrade: handleExecuteTrade,
    isTrading: tradeMutation.isPending,
  };

  return (
    <PageShell>
      <div className="max-w-[1400px] mx-auto px-4 sm:px-6 py-6 pb-24 lg:pb-6">
        {/* Header */}
        <div className="flex flex-col sm:flex-row items-start justify-between gap-3 mb-6">
          <div className="flex-1 min-w-0">
            <div className="flex flex-wrap items-center gap-2 sm:gap-3 mb-1">
              <h1 className="text-base sm:text-lg font-medium text-text-primary truncate">
                {market.question}
              </h1>
              <span
                className={cn(
                  'inline-flex items-center gap-1.5 text-[0.7rem] px-2 py-0.5 border',
                  statusColor,
                  'border-current/25 bg-current/10',
                )}
              >
                <span className="w-1.5 h-1.5 rounded-full bg-current" />
                {market.status.charAt(0).toUpperCase() + market.status.slice(1)}
              </span>
            </div>
            {market.category && (
              <p className="text-xs text-text-secondary">
                {market.category}
                {market.outcomeUnit ? ` (${market.outcomeUnit})` : ''}
              </p>
            )}
            {market.description && (
              <p className="text-xs text-text-muted mt-1 max-w-2xl">
                {market.description}
              </p>
            )}
          </div>
          <div className="flex items-center gap-2 flex-wrap">
            <Link href="/distribution">
              <Button variant="ghost" size="sm">Back</Button>
            </Link>
            <Button variant="ghost" size="sm" onClick={() => refetch()}>
              Refresh
            </Button>
            <button
              onClick={handleCopyLink}
              className="inline-flex items-center gap-1.5 border border-border px-3 py-1.5 text-xs font-medium text-text-secondary hover:bg-bg-hover transition-colors"
            >
              <LinkIcon className="h-3.5 w-3.5" />
              Copy Link
            </button>
            <ShareCastButton
              text={`${market.question} — trade on the distribution`}
              embedUrl={marketUrl}
            />
            <button
              onClick={handleShareX}
              className="inline-flex items-center gap-1.5 border border-border px-3 py-1.5 text-xs font-medium text-text-secondary hover:bg-bg-hover transition-colors"
            >
              <svg className="h-3.5 w-3.5" viewBox="0 0 24 24" fill="currentColor">
                <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z" />
              </svg>
              Share
            </button>
          </div>
        </div>

        {/* Main layout: chart + trade panel */}
        <div className="flex flex-col lg:flex-row gap-6">
          {/* Left: Chart + Stats */}
          <div className="flex-1 min-w-0">
            {/* Distribution Chart */}
            <div className="border border-border p-4 mb-4">
              <DistributionChart
                curveData={curveData ?? []}
                outcomeMin={market.outcomeMin}
                outcomeMax={market.outcomeMax}
                outcomeUnit={market.outcomeUnit}
                marketMu={market.marketMu}
                marketSigma={market.marketSigma}
                userPositions={myPositions.length > 0 ? myPositions : undefined}
              />
            </div>

            {/* Stats */}
            <DistributionStats
              stiffness={market.stiffness}
              muPerUnit={quote?.deltaMu ? Math.abs(1 / (quote.deltaMu || 1)) : undefined}
              sigmaPerUnit={quote?.deltaSigma ? Math.abs(1 / (quote.deltaSigma || 1)) : undefined}
              peakDensity={market.peakDensity}
              headroomPct={market.headroomPct}
              lambda={market.lambda}
              status={market.status}
              tradingEnd={market.tradingEnd}
              createdAt={market.createdAt}
            />

            {/* Admin: Resolve panel */}
            {isAdmin && market.status !== 'cancelled' && (
              <div className="mt-6">
                <DistributionResolvePanel market={market} onResolved={refetch} />
              </div>
            )}

            {/* Curve History */}
            {(curveHistory ?? []).length >= 2 && (
              <div className="mt-6">
                <DistributionCurveHistory
                  snapshots={curveHistory!}
                  outcomeUnit={market.outcomeUnit}
                />
              </div>
            )}

            {/* Recent Activity */}
            {(recentActivity ?? []).length > 0 && (
              <div className="mt-6">
                <h2 className="text-xs text-text-secondary uppercase tracking-wide mb-3">
                  Recent Activity
                </h2>
                <div className="border border-border divide-y divide-border">
                  {recentActivity!.map((entry, i) => (
                    <div
                      key={`${entry.createdAt}-${entry.mu}-${entry.sigma}-${i}`}
                      className="flex items-center justify-between px-4 py-2.5 text-xs"
                    >
                      <div className="flex items-center gap-3">
                        <span
                          className={cn(
                            'px-1.5 py-0.5 border text-[10px] uppercase tracking-wider',
                            entry.status === 'open'
                              ? 'text-bid border-bid/30 bg-bid/10'
                              : entry.status === 'closed'
                                ? 'text-ask border-ask/30 bg-ask/10'
                                : 'text-text-muted border-border bg-bg-secondary',
                          )}
                        >
                          {entry.status}
                        </span>
                        <span className="text-text-secondary">
                          {'\u03BC'}={entry.mu.toFixed(2)} {'\u03C3'}={entry.sigma.toFixed(2)}
                        </span>
                        <span className="font-mono tabular-nums text-text-primary">
                          {formatCurrency(entry.collateral)}
                        </span>
                      </div>
                      <span className="text-text-muted">
                        {formatTimeAgo(entry.createdAt)}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Positions */}
            {myPositions.length > 0 && (
              <div className="mt-6">
                <h2 className="text-xs text-text-secondary uppercase tracking-wide mb-3">
                  Your Positions
                </h2>
                <DistributionPositions
                  positions={myPositions}
                  marketResolved={market.status === 'resolved'}
                  onClose={handleClose}
                  onClaim={handleClaim}
                />
              </div>
            )}
          </div>

          {/* Right: Trade Panel — desktop only */}
          <div className="w-full lg:w-[380px] flex-shrink-0 hidden lg:block">
            <DistributionTradePanel {...tradePanelProps} />
          </div>
        </div>
      </div>

      {/* Mobile trade sheet */}
      <MobileDistributionSheet {...tradePanelProps} />
    </PageShell>
  );
}
