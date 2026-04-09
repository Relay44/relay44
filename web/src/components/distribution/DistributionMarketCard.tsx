'use client';

import Link from 'next/link';
import { useMemo } from 'react';
import { ArrowUpRight } from 'lucide-react';
import { cn } from '@/lib/utils';
import { Badge } from '@/components/ui';
import type { DistributionMarket } from '@/types/distribution';

export interface DistributionMarketCardProps {
  market: DistributionMarket;
}

function formatVolume(volume: number): string {
  if (volume >= 1_000_000) return `$${(volume / 1_000_000).toFixed(1)}M`;
  if (volume >= 1_000) return `$${Math.round(volume / 1_000)}k`;
  return `$${volume.toLocaleString()}`;
}

function miniCurvePoints(
  mu: number,
  sigma: number,
  min: number,
  max: number,
  steps: number = 40
): { x: number; y: number }[] {
  const points: { x: number; y: number }[] = [];
  const range = max - min;
  for (let i = 0; i <= steps; i++) {
    const xVal = min + (range * i) / steps;
    const z = (xVal - mu) / sigma;
    const pdf = Math.exp(-0.5 * z * z) / (sigma * Math.sqrt(2 * Math.PI));
    points.push({ x: i / steps, y: pdf });
  }
  const maxY = Math.max(...points.map((p) => p.y)) || 1;
  return points.map((p) => ({ x: p.x, y: p.y / maxY }));
}

function MiniCurve({ mu, sigma, min, max }: { mu: number; sigma: number; min: number; max: number }) {
  const points = useMemo(() => miniCurvePoints(mu, sigma, min, max), [mu, sigma, min, max]);
  const w = 120;
  const h = 48;
  const pad = 2;
  const plotW = w - pad * 2;
  const plotH = h - pad * 2;

  const pathD = useMemo(() => {
    if (points.length < 2) return '';
    const scaled = points.map((p) => ({
      x: pad + p.x * plotW,
      y: pad + plotH - p.y * plotH,
    }));
    const parts = [`M ${scaled[0].x} ${scaled[0].y}`];
    for (let i = 1; i < scaled.length; i++) {
      const prev = scaled[i - 1];
      const curr = scaled[i];
      const cpx1 = prev.x + (curr.x - prev.x) / 3;
      const cpx2 = curr.x - (curr.x - prev.x) / 3;
      parts.push(`C ${cpx1} ${prev.y}, ${cpx2} ${curr.y}, ${curr.x} ${curr.y}`);
    }
    return parts.join(' ');
  }, [points, plotW, plotH]);

  const fillD = useMemo(() => {
    if (!pathD) return '';
    const scaled = points.map((p) => ({
      x: pad + p.x * plotW,
      y: pad + plotH - p.y * plotH,
    }));
    const last = scaled[scaled.length - 1];
    const first = scaled[0];
    return `${pathD} L ${last.x} ${pad + plotH} L ${first.x} ${pad + plotH} Z`;
  }, [pathD, points, plotW, plotH]);

  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="w-full h-[48px]">
      <defs>
        <linearGradient id="mini-curve-fill" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--color-accent)" stopOpacity="0.15" />
          <stop offset="100%" stopColor="var(--color-accent)" stopOpacity="0.02" />
        </linearGradient>
      </defs>
      {fillD && <path d={fillD} fill="url(#mini-curve-fill)" />}
      <path d={pathD} fill="none" stroke="var(--color-accent)" strokeWidth="1.5" />
    </svg>
  );
}

export function DistributionMarketCard({ market }: DistributionMarketCardProps) {
  const mu = market.marketMu ?? (market.outcomeMin + market.outcomeMax) / 2;
  const sigma = market.marketSigma ?? (market.outcomeMax - market.outcomeMin) / 6;

  return (
    <Link
      href={`/distribution/${encodeURIComponent(market.id)}`}
      className="block group"
    >
      <div
        className={cn(
          'relative h-full overflow-hidden border border-border/70 p-4',
          'hover:border-accent hover:bg-bg-secondary',
          'transition-all duration-fast cursor-pointer flex flex-col'
        )}
      >
        {/* Header: badges */}
        <div className="flex items-center gap-1.5 text-[11px] uppercase tracking-[0.16em] text-text-muted mb-2">
          <Badge variant="accent" className="text-[10px]">
            Distribution
          </Badge>
          {market.category && (
            <span className="px-2 py-0.5 border border-border bg-bg-secondary/60">
              {market.category}
            </span>
          )}
        </div>

        {/* Question */}
        <h3 className="font-semibold text-text-primary text-sm leading-snug line-clamp-2 mb-3 group-hover:text-accent transition-colors duration-fast">
          {market.question}
        </h3>

        {/* Mini bell curve */}
        <div className="mb-3">
          <MiniCurve mu={mu} sigma={sigma} min={market.outcomeMin} max={market.outcomeMax} />
        </div>

        {/* Range display */}
        <div className="flex items-center justify-between mb-3 text-xs">
          <span className="text-text-secondary">
            {'\u03BC'} {'\u00B1'} {'\u03C3'}
          </span>
          <span className="font-mono tabular-nums text-text-primary">
            {mu.toFixed(2)} {'\u00B1'} {sigma.toFixed(2)}
            {market.outcomeUnit ? ` ${market.outcomeUnit}` : ''}
          </span>
        </div>

        {/* Footer */}
        <div className="relative flex items-center justify-between pt-3 border-t border-border mt-auto">
          <div className="flex items-center gap-2 text-xs text-text-muted">
            <span className="font-semibold text-text-primary">
              {formatVolume(market.totalVolume)}
            </span>
            <span className="text-text-secondary">vol</span>
          </div>
          <span className="inline-flex items-center gap-1.5 text-xs font-medium uppercase tracking-[0.12em] text-text-muted">
            Open
            <ArrowUpRight className="w-3.5 h-3.5" />
          </span>
        </div>
      </div>
    </Link>
  );
}
