'use client';

import Link from 'next/link';
import { useParams } from 'next/navigation';
import { ArrowLeft } from 'lucide-react';
import { useMarket } from '@/hooks/useMarkets';
import { useRuntimeMode } from '@/hooks';
import { useFarcaster } from '@/components/farcaster';
import { Badge } from '@/components/ui';
import { OrderBookDisplay } from '@/components/order';
import { MiniAppOrderForm } from '../../MiniAppOrderForm';
import { ShareCastButton, SwapToUsdc } from '@/components/farcaster';
import { MARKET_STATUS_LABELS } from '@/lib/constants';
import { formatCurrency, formatPercent } from '@/lib/utils';
import { SITE_URL } from '@/lib/seo';

export default function MiniAppMarketPage() {
  const params = useParams();
  const marketId = decodeURIComponent(params.id as string);
  const { readOnly } = useRuntimeMode();
  const fc = useFarcaster();
  const { data: market, isLoading, error } = useMarket(marketId);

  if (isLoading) {
    return (
      <div className="flex justify-center py-12">
        <div className="h-5 w-5 border-2 border-accent border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (error || !market) {
    return (
      <div className="text-center py-12">
        <h2 className="text-lg font-semibold mb-2">Market not found</h2>
        <Link href="/miniapp" className="text-accent hover:text-accent-hover text-sm">
          Back to Markets
        </Link>
      </div>
    );
  }

  const marketUrl = `${SITE_URL}/markets/${encodeURIComponent(market.id)}`;
  const yesPercent = Math.round(market.yesPrice * 100);
  const noPercent = Math.round(market.noPrice * 100);
  const shareText = `${market.question}\n\nYES ${yesPercent}% | NO ${noPercent}%`;

  const statusVariant =
    market.status === 'active' ? 'accent' : market.status === 'resolved' ? 'default' : 'muted';

  return (
    <div className="space-y-3 pb-4">
      {/* Top bar */}
      <div className="flex items-center justify-between gap-2">
        <Link
          href="/miniapp"
          className="inline-flex items-center gap-1 text-text-secondary hover:text-text-primary text-xs shrink-0"
        >
          <ArrowLeft className="h-3.5 w-3.5" />
          Back
        </Link>
        <div className="flex items-center gap-1.5">
          {fc.isMiniApp && <SwapToUsdc className="!px-2 !py-1 !text-[10px]" />}
          <ShareCastButton text={shareText} embedUrl={marketUrl} className="!px-2 !py-1 !text-[10px]" />
        </div>
      </div>

      {/* Tags */}
      <div className="flex flex-wrap items-center gap-1.5">
        <Badge variant="muted">{market.category}</Badge>
        <Badge variant={market.isExternal ? 'accent' : 'muted'}>{market.provider}</Badge>
        <Badge variant="muted">
          {market.chainId === 137 ? 'polygon' : market.chainId === 8453 ? 'base' : `chain-${market.chainId}`}
        </Badge>
        <Badge variant={statusVariant}>{MARKET_STATUS_LABELS[market.status]}</Badge>
      </div>

      {/* Question */}
      <h1 className="text-base font-bold leading-tight">{market.question}</h1>

      {/* Description */}
      {market.description && (
        <p className="text-xs leading-5 text-text-secondary line-clamp-3">
          {market.description}
        </p>
      )}

      {/* Price cards */}
      <div className="grid grid-cols-2 gap-2">
        <div className="rounded-lg border border-green-500/20 bg-green-500/5 p-3">
          <div className="text-[10px] text-text-muted mb-0.5">Yes</div>
          <div className="text-lg font-bold text-green-400">{formatPercent(market.yesPrice)}</div>
        </div>
        <div className="rounded-lg border border-red-500/20 bg-red-500/5 p-3">
          <div className="text-[10px] text-text-muted mb-0.5">No</div>
          <div className="text-lg font-bold text-red-400">{formatPercent(market.noPrice)}</div>
        </div>
      </div>

      {/* Volume row */}
      <div className="grid grid-cols-2 gap-2">
        <div className="rounded-lg border border-border/50 p-2.5">
          <div className="text-[10px] text-text-muted mb-0.5">24h Volume</div>
          <div className="text-sm font-semibold">{formatCurrency(market.volume24h)}</div>
        </div>
        <div className="rounded-lg border border-border/50 p-2.5">
          <div className="text-[10px] text-text-muted mb-0.5">Total Volume</div>
          <div className="text-sm font-semibold">{formatCurrency(market.totalVolume)}</div>
        </div>
      </div>

      {/* Order section */}
      {market.status === 'active' && !readOnly ? (
        <MiniAppOrderForm market={market} />
      ) : null}

      <OrderBookDisplay marketId={marketId} />
    </div>
  );
}
