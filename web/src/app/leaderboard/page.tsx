'use client';

import { PageShell } from '@/components/layout';
import { LeaderboardTable } from '@/components/leaderboard/LeaderboardTable';

export default function LeaderboardPage() {
  return (
    <PageShell>
      <div className="py-8 space-y-8">
        <div className="space-y-2">
          <h1 className="text-3xl font-bold">Leaderboard</h1>
          <p className="text-text-secondary">
            Top traders ranked by performance across all markets.
          </p>
        </div>

        <LeaderboardTable
          initialPeriod="weekly"
          initialMetric="pnl"
          limit={50}
          showControls
        />
      </div>
    </PageShell>
  );
}
