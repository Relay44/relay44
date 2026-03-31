'use client';

import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import { useHackathonSnapshots } from '@/hooks/useHackathons';
import {
  ResponsiveContainer,
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  CartesianGrid,
} from 'recharts';

interface HackathonPnlChartProps {
  hackathonId: string;
  walletAddress?: string;
}

export function HackathonPnlChart({ hackathonId, walletAddress }: HackathonPnlChartProps) {
  const { data, isLoading } = useHackathonSnapshots(hackathonId, walletAddress);

  const snapshots = data?.snapshots || [];

  const chartData = snapshots.map((s) => ({
    time: new Date(s.snapshotTime).toLocaleDateString(undefined, {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    }),
    pnl: s.netPnlUsdc,
  }));

  return (
    <Card>
      <CardHeader>
        <CardTitle>P&L Over Time</CardTitle>
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <div className="flex items-center justify-center h-48">
            <div className="animate-pulse text-text-secondary">Loading chart...</div>
          </div>
        ) : chartData.length === 0 ? (
          <div className="flex items-center justify-center h-48 text-text-muted text-sm">
            No snapshot data yet
          </div>
        ) : (
          <ResponsiveContainer width="100%" height={240}>
            <LineChart data={chartData}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
              <XAxis
                dataKey="time"
                tick={{ fontSize: 11, fill: 'var(--text-secondary)' }}
                tickLine={false}
              />
              <YAxis
                tick={{ fontSize: 11, fill: 'var(--text-secondary)' }}
                tickLine={false}
                tickFormatter={(v) => `$${v}`}
              />
              <Tooltip
                contentStyle={{
                  background: 'var(--bg-secondary)',
                  border: '1px solid var(--border)',
                  fontSize: 12,
                }}
                formatter={(value) => [`$${Number(value).toFixed(2)}`, 'P&L']}
              />
              <Line
                type="monotone"
                dataKey="pnl"
                stroke="var(--accent)"
                strokeWidth={2}
                dot={false}
              />
            </LineChart>
          </ResponsiveContainer>
        )}
      </CardContent>
    </Card>
  );
}
