'use client';

import Link from 'next/link';
import { useParams } from 'next/navigation';
import { ArrowLeft } from 'lucide-react';
import { useMarket } from '@/hooks/useMarkets';
import { useRuntimeMode } from '@/hooks';
import { useFarcaster } from '@/components/farcaster';
import { LoadingScreen } from '@/components/ui';
import { MarketHeader, MarketStats } from '@/components/market';
import { OrderBookDisplay } from '@/components/order';
import { MiniAppOrderForm } from '../../MiniAppOrderForm';
import { ShareCastButton, SwapToUsdc } from '@/components/farcaster';
import { SITE_URL } from '@/lib/seo';

export default function MiniAppMarketPage() {
  const params = useParams();
  const marketId = decodeURIComponent(params.id as string);
  const { readOnly } = useRuntimeMode();
  const fc = useFarcaster();
  const { data: market, isLoading, error } = useMarket(marketId);

  if (isLoading) return <LoadingScreen />;

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
  const shareText = `${market.question}\n\nYES ${Math.round(market.yesPrice * 100)}% | NO ${Math.round(market.noPrice * 100)}%`;

  return (
    <div className="space-y-4">
      {/* Mini App navigation */}
      <div className="flex items-center justify-between">
        <Link
          href="/miniapp"
          className="inline-flex items-center gap-1 text-text-secondary hover:text-text-primary text-sm"
        >
          <ArrowLeft className="h-4 w-4" />
          Back
        </Link>
        <div className="flex items-center gap-2">
          {fc.isMiniApp && <SwapToUsdc />}
          <ShareCastButton text={shareText} embedUrl={marketUrl} />
        </div>
      </div>

      <MarketHeader market={market} />
      <MarketStats market={market} />

      {/* Order section */}
      {market.status === 'active' && !readOnly ? (
        <MiniAppOrderForm market={market} />
      ) : null}

      <OrderBookDisplay marketId={marketId} />
    </div>
  );
}
