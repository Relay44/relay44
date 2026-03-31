'use client';

import React from 'react';
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

const RankBadge = React.memo(function RankBadge({ rank }: { rank: number }) {
  const styles: Record<number, string> = {
    1: 'bg-yellow-500/20 text-yellow-500',
    2: 'bg-gray-400/20 text-gray-400',
    3: 'bg-amber-700/20 text-amber-700',
  };

  const style = styles[rank];

  return (
    <div className={cn('w-8 h-8 flex items-center justify-center', style)}>
      <span className={cn('font-bold', !style && 'text-text-secondary')}>{rank}</span>
    </div>
  );
});

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
          <table className="w-full" aria-label="Hackathon leaderboard">
            <thead>
              <tr className="border-b border-border text-xs text-text-secondary">
                <th className="py-2 text-left font-normal w-10">Rank</th>
                <th className="py-2 text-left font-normal">Trader</th>
                <th className="py-2 text-right font-normal">P&L</th>
                <th className="py-2 text-right font-normal hidden sm:table-cell">Volume</th>
                <th className="py-2 text-right font-normal hidden sm:table-cell">Win Rate</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((entry: HackathonLeaderboardEntry) => {
                const isCurrentUser = currentWallet === entry.walletAddress.toLowerCase();
                const isPositive = entry.netPnlUsdc >= 0;

                return (
                  <tr
                    key={entry.walletAddress}
                    className={cn(
                      'hover:bg-bg-secondary transition-colors duration-fast',
                      isCurrentUser && 'bg-accent/5',
                    )}
                  >
                    <td className="py-2">
                      <RankBadge rank={entry.rank} />
                    </td>
                    <td className="py-2 min-w-0">
                      <Link
                        href={`/profile/${entry.walletAddress}`}
                        className="block truncate font-medium text-text-primary hover:text-accent transition-colors"
                      >
                        {truncateAddress(entry.walletAddress)}
                        {isCurrentUser && (
                          <span className="ml-2 text-xs text-accent">(you)</span>
                        )}
                      </Link>
                    </td>
                    <td className="py-2 text-right">
                      <span className={cn('font-medium', isPositive ? 'text-bid' : 'text-ask')}>
                        {isPositive ? '+' : ''}${formatNumber(entry.netPnlUsdc)}
                      </span>
                    </td>
                    <td className="py-2 text-right hidden sm:table-cell">
                      <span className="text-text-secondary text-sm">
                        ${formatNumber(entry.totalVolumeUsdc)}
                      </span>
                    </td>
                    <td className="py-2 text-right hidden sm:table-cell">
                      <span className="text-text-secondary text-sm">
                        {(entry.winRateBps / 100).toFixed(1)}%
                      </span>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </CardContent>
    </Card>
  );
}
