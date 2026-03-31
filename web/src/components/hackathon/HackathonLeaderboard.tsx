'use client';

import Link from 'next/link';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import { useHackathonLeaderboard } from '@/hooks/useHackathons';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { cn } from '@/lib/utils';
import type { HackathonLeaderboardEntry } from '@/types';

function formatNumber(num: number): string {
  const abs = Math.abs(num);
  if (abs >= 1_000_000) return `${(num / 1_000_000).toFixed(2)}M`;
  if (abs >= 1_000) return `${(num / 1_000).toFixed(1)}K`;
  return num.toFixed(2);
}

function truncateAddress(address: string): string {
  if (!address || address.length < 8) return address || '';
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

function RankBadge({ rank }: { rank: number }) {
  if (rank === 1) {
    return (
      <div className="w-8 h-8 bg-yellow-500/20 flex items-center justify-center">
        <span className="text-yellow-500 font-bold">1</span>
      </div>
    );
  }
  if (rank === 2) {
    return (
      <div className="w-8 h-8 bg-gray-400/20 flex items-center justify-center">
        <span className="text-gray-400 font-bold">2</span>
      </div>
    );
  }
  if (rank === 3) {
    return (
      <div className="w-8 h-8 bg-amber-700/20 flex items-center justify-center">
        <span className="text-amber-700 font-bold">3</span>
      </div>
    );
  }
  return (
    <div className="w-8 h-8 flex items-center justify-center">
      <span className="text-text-secondary">{rank}</span>
    </div>
  );
}

interface HackathonLeaderboardProps {
  hackathonId: string;
}

export function HackathonLeaderboard({ hackathonId }: HackathonLeaderboardProps) {
  const { data, isLoading } = useHackathonLeaderboard(hackathonId);
  const { address } = useBaseWallet();
  const currentWallet = address?.toLowerCase();

  if (isLoading) {
    return (
      <Card>
        <CardContent className="flex items-center justify-center h-64">
          <div className="animate-pulse text-text-secondary">Loading leaderboard...</div>
        </CardContent>
      </Card>
    );
  }

  const entries = data?.entries || [];

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between">
          <CardTitle>Leaderboard</CardTitle>
          {data?.updatedAt && (
            <span className="text-xs text-text-muted">
              Updated {new Date(data.updatedAt).toLocaleTimeString()}
            </span>
          )}
        </div>
      </CardHeader>

      <CardContent>
        {entries.length === 0 ? (
          <div className="text-center py-8 text-text-secondary">
            No leaderboard data yet. Snapshots are taken periodically once the hackathon is active.
          </div>
        ) : (
          <div className="space-y-1">
            {/* Header */}
            <div className="grid grid-cols-[auto_1fr_auto_auto_auto] items-center gap-2 border-b border-border py-2 text-xs text-text-secondary">
              <span>Rank</span>
              <span>Trader</span>
              <span className="text-right">P&L</span>
              <span className="text-right hidden sm:block">Volume</span>
              <span className="text-right hidden sm:block">Win Rate</span>
            </div>

            {entries.map((entry: HackathonLeaderboardEntry) => {
              const isCurrentUser = currentWallet === entry.walletAddress.toLowerCase();
              const isPositive = entry.netPnlUsdc >= 0;

              return (
                <Link
                  key={entry.walletAddress}
                  href={`/profile/${entry.walletAddress}`}
                  className={cn(
                    'grid grid-cols-[auto_1fr_auto_auto_auto] items-center gap-2 py-2 hover:bg-bg-secondary transition-colors duration-fast cursor-pointer',
                    isCurrentUser && 'bg-accent/5 border border-accent/20',
                  )}
                >
                  <RankBadge rank={entry.rank} />

                  <div className="min-w-0">
                    <span className="block truncate font-medium text-text-primary">
                      {truncateAddress(entry.walletAddress)}
                      {isCurrentUser && (
                        <span className="ml-2 text-xs text-accent">(you)</span>
                      )}
                    </span>
                  </div>

                  <div className="text-right">
                    <span className={cn('font-medium', isPositive ? 'text-bid' : 'text-ask')}>
                      {isPositive ? '+' : ''}${formatNumber(entry.netPnlUsdc)}
                    </span>
                  </div>

                  <div className="text-right hidden sm:block">
                    <span className="text-text-secondary text-sm">
                      ${formatNumber(entry.totalVolumeUsdc)}
                    </span>
                  </div>

                  <div className="text-right hidden sm:block">
                    <span className="text-text-secondary text-sm">
                      {(entry.winRateBps / 100).toFixed(1)}%
                    </span>
                  </div>
                </Link>
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
