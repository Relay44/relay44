'use client';

import Link from 'next/link';
import { useMemo, useState } from 'react';
import { PageShell } from '@/components/layout';
import { LoadingScreen } from '@/components/ui';
import { useScannerOpportunities } from '@/hooks';
import { cn } from '@/lib/utils';
import type { ScannedOpportunity } from '@/types';

type SortKey = 'score' | 'mispricing' | 'liquidity' | 'volume' | 'recent';

const OPP_TYPES: { value: string; label: string }[] = [
  { value: '', label: 'All types' },
  { value: 'longshot', label: 'Longshot' },
  { value: 'near_certainty', label: 'Near-certainty' },
  { value: 'spread_capture', label: 'Spread capture' },
  { value: 'correlation', label: 'Correlation arb' },
];

const SORTS: { value: SortKey; label: string }[] = [
  { value: 'score', label: 'Score' },
  { value: 'mispricing', label: 'Mispricing' },
  { value: 'liquidity', label: 'Liquidity' },
  { value: 'volume', label: 'Volume' },
  { value: 'recent', label: 'Recent' },
];

function fmtUsd(n: number | null | undefined): string {
  if (n == null || !Number.isFinite(n)) return '--';
  if (n >= 1_000_000) return `$${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `$${(n / 1_000).toFixed(1)}k`;
  return `$${n.toFixed(0)}`;
}

function fmtPct(n: number | null | undefined): string {
  if (n == null || !Number.isFinite(n)) return '--';
  return `${(n * 100).toFixed(1)}¢`;
}

function opportunityTone(type: string): string {
  if (type.startsWith('longshot')) return 'text-yellow-400';
  if (type.startsWith('near_certainty')) return 'text-green-400';
  if (type === 'spread_capture') return 'text-blue-400';
  return 'text-text-primary';
}

function Row({ op }: { op: ScannedOpportunity }) {
  return (
    <Link
      href={`/markets/${encodeURIComponent(op.slug || op.conditionId)}`}
      className="grid grid-cols-[minmax(0,1fr)_repeat(6,auto)] items-center gap-4 border-b border-border px-4 py-3 text-sm transition-colors hover:bg-bg-secondary"
    >
      <div className="min-w-0">
        <div className="truncate font-medium text-text-primary">{op.question}</div>
        <div className="mt-0.5 flex items-center gap-2 text-[10px] uppercase tracking-[0.14em] text-text-muted">
          <span>{op.category || 'general'}</span>
          <span className={cn('font-mono', opportunityTone(op.opportunityType))}>
            {op.opportunityType}
          </span>
        </div>
      </div>
      <div className="w-16 text-right font-mono text-xs text-text-secondary">
        {fmtPct(op.yesPrice)}
      </div>
      <div className="w-16 text-right font-mono text-xs text-text-secondary">
        {fmtPct(op.noPrice)}
      </div>
      <div className="w-20 text-right font-mono text-xs text-text-secondary">
        {fmtUsd(op.liquidityUsdc)}
      </div>
      <div className="w-20 text-right font-mono text-xs text-text-secondary">
        {fmtUsd(op.volumeUsdc)}
      </div>
      <div className="w-16 text-right font-mono text-xs text-text-primary">
        {op.opportunityScore?.toFixed(2) ?? '--'}
      </div>
      <div className="w-16 text-right font-mono text-xs text-text-secondary">
        {op.mispricingScore?.toFixed(2) ?? '--'}
      </div>
    </Link>
  );
}

export default function ScreenerClient() {
  const [opType, setOpType] = useState<string>('');
  const [category, setCategory] = useState<string>('');
  const [minLiquidity, setMinLiquidity] = useState<string>('1000');
  const [minScore, setMinScore] = useState<string>('');
  const [sort, setSort] = useState<SortKey>('score');

  const params = useMemo(() => {
    const p: Parameters<typeof useScannerOpportunities>[0] = {
      limit: 100,
      sort,
    };
    if (opType) p.opportunityType = opType;
    if (category) p.category = category;
    const ml = parseFloat(minLiquidity);
    if (Number.isFinite(ml) && ml > 0) p.minLiquidity = ml;
    const ms = parseFloat(minScore);
    if (Number.isFinite(ms) && ms > 0) p.minScore = ms;
    return p;
  }, [opType, category, minLiquidity, minScore, sort]);

  const { data, isLoading, error, dataUpdatedAt } = useScannerOpportunities(params);
  const opportunities = data?.opportunities ?? [];

  const categories = useMemo(() => {
    const set = new Set<string>();
    for (const op of opportunities) {
      if (op.category) set.add(op.category);
    }
    return Array.from(set).sort();
  }, [opportunities]);

  return (
    <PageShell>
      <div className="py-8 space-y-6">
        <div className="space-y-2">
          <h1 className="text-xl font-semibold">Screener</h1>
          <p className="text-sm text-text-secondary">
            Live Polymarket opportunities. Updates every 30s.
          </p>
        </div>

        <div className="grid gap-3 border border-border bg-bg-secondary/40 p-4 md:grid-cols-5">
          <div>
            <label className="block text-[10px] uppercase tracking-[0.18em] text-text-muted mb-1">
              Type
            </label>
            <select
              value={opType}
              onChange={(e) => setOpType(e.target.value)}
              className="w-full border border-border bg-bg-primary px-2 py-2 text-xs text-text-primary focus:border-accent focus:outline-none"
            >
              {OPP_TYPES.map((t) => (
                <option key={t.value} value={t.value}>
                  {t.label}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="block text-[10px] uppercase tracking-[0.18em] text-text-muted mb-1">
              Category
            </label>
            <select
              value={category}
              onChange={(e) => setCategory(e.target.value)}
              className="w-full border border-border bg-bg-primary px-2 py-2 text-xs text-text-primary focus:border-accent focus:outline-none"
            >
              <option value="">All</option>
              {categories.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="block text-[10px] uppercase tracking-[0.18em] text-text-muted mb-1">
              Min liquidity (USDC)
            </label>
            <input
              type="number"
              min={0}
              step={500}
              value={minLiquidity}
              onChange={(e) => setMinLiquidity(e.target.value)}
              className="w-full border border-border bg-bg-primary px-2 py-2 text-xs text-text-primary placeholder:text-text-muted focus:border-accent focus:outline-none"
            />
          </div>
          <div>
            <label className="block text-[10px] uppercase tracking-[0.18em] text-text-muted mb-1">
              Min score
            </label>
            <input
              type="number"
              min={0}
              step={0.1}
              value={minScore}
              onChange={(e) => setMinScore(e.target.value)}
              placeholder="0"
              className="w-full border border-border bg-bg-primary px-2 py-2 text-xs text-text-primary placeholder:text-text-muted focus:border-accent focus:outline-none"
            />
          </div>
          <div>
            <label className="block text-[10px] uppercase tracking-[0.18em] text-text-muted mb-1">
              Sort
            </label>
            <select
              value={sort}
              onChange={(e) => setSort(e.target.value as SortKey)}
              className="w-full border border-border bg-bg-primary px-2 py-2 text-xs text-text-primary focus:border-accent focus:outline-none"
            >
              {SORTS.map((s) => (
                <option key={s.value} value={s.value}>
                  {s.label}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="flex items-center justify-between text-[10px] uppercase tracking-[0.18em] text-text-muted">
          <span>{opportunities.length} result{opportunities.length === 1 ? '' : 's'}</span>
          {dataUpdatedAt ? (
            <span className="font-mono">
              updated {new Date(dataUpdatedAt).toLocaleTimeString()}
            </span>
          ) : null}
        </div>

        <div className="border border-border">
          <div className="grid grid-cols-[minmax(0,1fr)_repeat(6,auto)] items-center gap-4 border-b border-border bg-bg-secondary px-4 py-2 text-[10px] uppercase tracking-[0.18em] text-text-muted">
            <div>Market</div>
            <div className="w-16 text-right">Yes</div>
            <div className="w-16 text-right">No</div>
            <div className="w-20 text-right">Liquidity</div>
            <div className="w-20 text-right">Volume</div>
            <div className="w-16 text-right">Score</div>
            <div className="w-16 text-right">Mispricing</div>
          </div>

          {isLoading ? (
            <LoadingScreen />
          ) : error ? (
            <div className="py-12 text-center text-text-muted">
              Failed to load opportunities.
            </div>
          ) : opportunities.length === 0 ? (
            <div className="py-12 text-center text-text-muted">
              No markets match these filters.
            </div>
          ) : (
            opportunities.map((op) => <Row key={op.conditionId} op={op} />)
          )}
        </div>
      </div>
    </PageShell>
  );
}
