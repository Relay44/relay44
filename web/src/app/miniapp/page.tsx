'use client';

import { useState } from 'react';
import Link from 'next/link';
import { TrendingUp } from 'lucide-react';
import { useFarcaster } from '@/components/farcaster';
import { useFarcasterAuth } from '@/hooks/useFarcasterAuth';
import { useMarkets } from '@/hooks/useMarkets';
import { SwapToUsdc } from '@/components/farcaster';
import type { Market } from '@/types';

function MiniMarketCard({ market }: { market: Market }) {
  const yesPercent = Math.round(market.yesPrice * 100);
  const noPercent = Math.round(market.noPrice * 100);

  return (
    <Link
      href={`/miniapp/market/${encodeURIComponent(market.id)}`}
      className="block border border-border/70 rounded-lg p-3 hover:border-accent transition-colors"
    >
      <p className="text-sm font-medium text-text-primary line-clamp-2 mb-2">
        {market.question}
      </p>
      <div className="flex items-center gap-2">
        <span className="text-xs font-mono text-green-400">
          YES {yesPercent}%
        </span>
        <div className="flex-1 h-1.5 bg-bg-secondary rounded-full overflow-hidden">
          <div
            className="h-full bg-green-400 rounded-full"
            style={{ width: `${yesPercent}%` }}
          />
        </div>
        <span className="text-xs font-mono text-red-400">
          NO {noPercent}%
        </span>
      </div>
      {market.volume24h > 0 && (
        <p className="text-[10px] text-text-tertiary mt-1.5">
          24h vol: ${market.volume24h.toLocaleString()}
        </p>
      )}
    </Link>
  );
}

export default function MiniAppHome() {
  const fc = useFarcaster();
  const fcAuth = useFarcasterAuth();
  const [isSignedIn, setIsSignedIn] = useState(false);

  const handleSignIn = async () => {
    const success = await fcAuth.login();
    if (success) setIsSignedIn(true);
  };

  const { data: markets, isLoading } = useMarkets({
    sort: 'volume',
    limit: 20,
  });

  return (
    <div className="space-y-3">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <TrendingUp className="h-4 w-4 text-accent" />
          <h1 className="text-base font-semibold">relay44</h1>
        </div>
        {fc.isMiniApp && <SwapToUsdc />}
      </div>

      {fc.user && (
        <p className="text-xs text-text-secondary">
          Hey, {fc.user.displayName || fc.user.username || `FID #${fc.user.fid}`}
          {!isSignedIn && (
            <button
              onClick={handleSignIn}
              disabled={fcAuth.isLoading}
              className="ml-2 text-accent underline"
            >
              {fcAuth.isLoading ? 'Signing in...' : 'Sign in to trade'}
            </button>
          )}
          {isSignedIn && (
            <span className="ml-2 text-green-400">Connected</span>
          )}
        </p>
      )}
      {fcAuth.error && (
        <p className="text-xs text-red-400">{fcAuth.error}</p>
      )}

