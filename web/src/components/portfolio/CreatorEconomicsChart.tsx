'use client';

import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { TabsRoot, TabsList, TabsTrigger } from '@/components/ui';
import type {
  CreatorChartRange,
  CreatorEconomicsMarketDetail,
} from '@/types';
import { formatCurrency, formatDateShort } from '@/lib/utils';

interface CreatorEconomicsChartProps {
  detail: CreatorEconomicsMarketDetail;
  range: CreatorChartRange;
  onRangeChange: (range: CreatorChartRange) => void;
}

export function CreatorEconomicsChart({
  detail,
  range,
  onRangeChange,
}: CreatorEconomicsChartProps) {
  const points = detail.points ?? [];
  const chartData = points.map((point) => ({
    bucket: formatDateShort(point.day),
    inventoryMarkValueUsdc: point.inventoryMarkValueUsdc,
    subsidyBurnUsdc: point.subsidyBurnUsdc,
    cumulativeBootstrapFillsUsdc: point.cumulativeBootstrapFillsUsdc,
  }));

  return (
    <Card>
      <CardHeader className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <CardTitle>Economics trend</CardTitle>
          <p className="mt-1 text-sm text-text-secondary">
            Burn, marked inventory value, and cumulative bootstrap fills over the selected window.
          </p>
        </div>
        <TabsRoot value={range} onValueChange={(value) => onRangeChange(value as CreatorChartRange)}>
          <TabsList>
            <TabsTrigger value="7d">7d</TabsTrigger>
            <TabsTrigger value="30d">30d</TabsTrigger>
            <TabsTrigger value="90d">90d</TabsTrigger>
          </TabsList>
        </TabsRoot>
      </CardHeader>
      <CardContent>
        {chartData.length === 0 ? (
          <div className="flex h-64 items-center justify-center text-sm text-text-secondary">
            No chart data yet for this window.
          </div>
        ) : (
          <ResponsiveContainer width="100%" height={260}>
            <LineChart data={chartData}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
              <XAxis
                dataKey="bucket"
                tick={{ fontSize: 11, fill: 'var(--text-secondary)' }}
                tickLine={false}
              />
              <YAxis
                tick={{ fontSize: 11, fill: 'var(--text-secondary)' }}
                tickLine={false}
                tickFormatter={(value) => formatCurrency(Number(value))}
              />
              <Tooltip
                contentStyle={{
                  background: 'var(--bg-secondary)',
                  border: '1px solid var(--border)',
                  fontSize: 12,
                }}
                formatter={(value: number | string | undefined, name: string | undefined) => [
                  formatCurrency(Number(value ?? 0)),
                  name === 'cumulativeBootstrapFillsUsdc'
                    ? 'Bootstrap fills'
                    : name === 'subsidyBurnUsdc'
                      ? 'Subsidy burn'
                      : 'Marked inventory value',
                ]}
              />
              <Line
                type="monotone"
                dataKey="inventoryMarkValueUsdc"
                stroke="var(--accent)"
                strokeWidth={2}
                dot={false}
              />
              <Line
                type="monotone"
                dataKey="subsidyBurnUsdc"
                stroke="var(--ask)"
                strokeWidth={2}
                dot={false}
              />
              <Line
                type="monotone"
                dataKey="cumulativeBootstrapFillsUsdc"
                stroke="var(--bid)"
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
