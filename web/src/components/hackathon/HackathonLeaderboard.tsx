'use client';

import React, { useState } from 'react';
import Link from 'next/link';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { useHackathonLeaderboard } from '@/hooks/useHackathons';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { cn } from '@/lib/utils';
import type { HackathonLeaderboardEntry, HackathonScoringMethod } from '@/types';

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

const SCORING_METHODS: {
  id: HackathonScoringMethod;
  label: string;
  tooltip: string;
  format: (entry: HackathonLeaderboardEntry) => string;
  colorFn: (entry: HackathonLeaderboardEntry) => string;
}[] = [
  {
    id: 'net_pnl',
    label: 'P&L',
    tooltip: 'Net profit & loss in USDC. Sum of realized and unrealized gains across all positions during the hackathon period.',
    format: (e) => `${e.netPnlUsdc >= 0 ? '+' : ''}$${formatNumber(e.netPnlUsdc)}`,
    colorFn: (e) => (e.netPnlUsdc >= 0 ? 'text-bid' : 'text-ask'),
  },
  {
    id: 'sharpe_ratio',
    label: 'Sharpe',
    tooltip: 'Risk-adjusted return metric. Calculated as mean return divided by standard deviation of returns. Higher values indicate better risk-adjusted performance. Requires at least 2 closed positions.',
    format: (e) => (e.sharpeRatioBps / 100).toFixed(2),
    colorFn: (e) => (e.sharpeRatioBps >= 0 ? 'text-bid' : 'text-ask'),
  },
  {
    id: 'win_rate',
    label: 'Win Rate',
    tooltip: 'Percentage of profitable trades. Calculated as the number of positions with positive realized PnL divided by total trade count.',
    format: (e) => `${(e.winRateBps / 100).toFixed(1)}%`,
    colorFn: (e) => (e.winRateBps >= 5000 ? 'text-bid' : e.winRateBps < 5000 ? 'text-ask' : 'text-text-primary'),
  },
];

interface HackathonLeaderboardProps {
  hackathonId: string;
}

function InfoIcon({ className }: { className?: string }) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 16 16"
      fill="currentColor"
      className={cn('w-3.5 h-3.5', className)}
    >
      <path
        fillRule="evenodd"
        d="M15 8A7 7 0 1 1 1 8a7 7 0 0 1 14 0Zm-6 3.5a1 1 0 1 1-2 0v-3a1 1 0 1 1 2 0v3ZM8 5.5A1 1 0 1 0 8 3.5a1 1 0 0 0 0 2Z"
        clipRule="evenodd"
      />
    </svg>
  );
}

export function HackathonLeaderboard({ hackathonId }: HackathonLeaderboardProps) {
  const [scoringMethod, setScoringMethod] = useState<HackathonScoringMethod>('net_pnl');
  const { data, isLoading } = useHackathonLeaderboard(hackathonId, scoringMethod);
  const { address } = useBaseWallet();
  const currentWallet = address?.toLowerCase();

  const activeMethod = SCORING_METHODS.find((m) => m.id === scoringMethod) || SCORING_METHODS[0];

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
    <TooltipProvider>
      <Card>
        <CardHeader>
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex items-center gap-2">
              <CardTitle>Leaderboard</CardTitle>
              {data?.updatedAt && (
                <span className="text-xs text-text-muted">
                  Updated {new Date(data.updatedAt).toLocaleTimeString()}
                </span>
              )}
            </div>

            {/* Scoring method selector */}
            <div className="flex gap-1 p-1 bg-bg-tertiary">
              {SCORING_METHODS.map((method) => (
                <Tooltip key={method.id}>
                  <TooltipTrigger asChild>
                    <button
                      type="button"
                      onClick={() => setScoringMethod(method.id)}
                      className={cn(
                        'px-3 py-1 text-sm transition-colors duration-fast cursor-pointer whitespace-nowrap',
                        scoringMethod === method.id
                          ? 'bg-accent text-text-inverse'
                          : 'text-text-secondary hover:text-text-primary'
                      )}
                    >
                      {method.label}
                    </button>
                  </TooltipTrigger>
                  <TooltipContent side="bottom" className="max-w-xs">
                    <p>{method.tooltip}</p>
                  </TooltipContent>
                </Tooltip>
              ))}
            </div>
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
                  <th className="py-2 text-right font-normal">
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <span className="inline-flex items-center gap-1 cursor-help">
                          {activeMethod.label}
                          <InfoIcon className="text-text-muted" />
                        </span>
                      </TooltipTrigger>
                      <TooltipContent side="bottom" className="max-w-xs">
                        <p>{activeMethod.tooltip}</p>
                      </TooltipContent>
                    </Tooltip>
                  </th>
                  <th className="py-2 text-right font-normal hidden sm:table-cell">Volume</th>
                  <th className="py-2 text-right font-normal hidden md:table-cell">Trades</th>
                </tr>
              </thead>
              <tbody>
                {entries.map((entry: HackathonLeaderboardEntry) => {
                  const isCurrentUser = currentWallet === entry.walletAddress.toLowerCase();

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
                        <span className={cn('font-medium', activeMethod.colorFn(entry))}>
                          {activeMethod.format(entry)}
                        </span>
                      </td>
                      <td className="py-2 text-right hidden sm:table-cell">
                        <span className="text-text-secondary text-sm">
                          ${formatNumber(entry.totalVolumeUsdc)}
                        </span>
                      </td>
                      <td className="py-2 text-right hidden md:table-cell">
                        <span className="text-text-secondary text-sm">
                          {entry.tradeCount}
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
    </TooltipProvider>
  );
}
