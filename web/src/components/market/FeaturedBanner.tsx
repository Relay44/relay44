'use client';

import { useEffect, useId, useMemo, useState } from 'react';
import type { SignalSnapshot } from '@/lib/server/homeLive';

export interface SignalChartProps {
  initialSignal: SignalSnapshot;
}

interface ChartPoint {
  x: number;
  y: number;
}

const CHART_WIDTH = 1000;
const CHART_HEIGHT = 36;
const TRACE_SAMPLES = 150;
const SWEEP_WIDTH = 80;

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function toPath(points: ChartPoint[]): string {
  if (points.length === 0) {
    return '';
  }

  return points
    .map((point, index) => `${index === 0 ? 'M' : 'L'} ${point.x.toFixed(2)} ${point.y.toFixed(2)}`)
    .join(' ');
}

function sampleValue(values: number[], position: number): number {
  if (values.length === 0) {
    return 50;
  }

  const length = values.length;
  const wrapped = ((position % length) + length) % length;
  const index = Math.floor(wrapped);
  const fraction = wrapped - index;
  const current = values[index];
  const next = values[(index + 1) % length] ?? current;

  return current + (next - current) * fraction;
}

function buildSeries(
  values: number[],
  phase: number,
  latencyMs: number,
  marketsTracked: number,
  variant: 'primary' | 'echo'
): ChartPoint[] {
  const offset = variant === 'primary' ? 0 : 0.9;
  const scale = variant === 'primary' ? 1 : 0.62;
  const drift = phase * (0.62 + marketsTracked / 180) + offset;
  const tremorGain = clamp(0.9 + latencyMs / 120, 0.9, variant === 'primary' ? 1.9 : 1.35);
  const baselineLift = clamp(marketsTracked / 260, 0.05, 0.22);

  return Array.from({ length: TRACE_SAMPLES }, (_, index) => {
    const ratio = index / Math.max(TRACE_SAMPLES - 1, 1);
    const x = ratio * CHART_WIDTH;
    const anchor = ratio * Math.max(values.length - 1, 1) + drift;
    const value = sampleValue(values, anchor);
    const previous = sampleValue(values, anchor - 0.45);
    const next = sampleValue(values, anchor + 0.45);
    const slope = (next - previous) / 100;
    const energy = Math.abs(value - 50) / 50;
    const lowBand = Math.sin(index * 0.42 + phase * 1.65 + offset) * (0.8 + energy * 1.1);
    const midBand = Math.sin(index * 1.26 + phase * 3.9 + value / 22 + offset) * (0.45 + energy * 0.95);
    const burstGate = Math.max(0, Math.sin(index * 0.11 + phase * 0.92 + offset + value / 36));
    const burst =
      burstGate *
      burstGate *
      Math.sin(index * 3.45 + phase * 6.1 + offset) *
      (0.9 + energy * 4.6 + Math.abs(slope) * 8.5);
    const baseline = (value - 50) * (0.1 + baselineLift);
    const offsetY = baseline + (lowBand + midBand) * tremorGain * scale + burst * 0.52 * scale + slope * 4.8 * scale;

    return {
      x,
      y: clamp(CHART_HEIGHT / 2 - offsetY, 2, CHART_HEIGHT - 2),
    };
  });
}

function formatTimestamp(value: string): string {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return 'now';
  }

  return parsed.toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
    timeZone: 'UTC',
  }) + ' UTC';
}

export function SignalChart({ initialSignal }: SignalChartProps) {
  const [signal, setSignal] = useState(initialSignal);
  const [phase, setPhase] = useState(0);
  const gradientId = useId();

  useEffect(() => {
    setSignal(initialSignal);
  }, [initialSignal]);

  useEffect(() => {
    let active = true;

    const refresh = async () => {
      try {
        const response = await fetch('/api/home/live', { cache: 'no-store' });
        if (!response.ok) {
          return;
        }
        const payload = (await response.json()) as { signal?: SignalSnapshot };
        if (active && payload.signal) {
          setSignal(payload.signal);
        }
      } catch {
        // Keep the last good snapshot.
      }
    };

    const interval = window.setInterval(() => {
      void refresh();
    }, 60_000);

    return () => {
      active = false;
      window.clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    let frameId = 0;
    let lastTime = 0;
    const speed = clamp(0.55, 1.45, 75 / Math.max(signal.latencyMs, 24) + signal.marketsTracked / 180);

    const tick = (timestamp: number) => {
      if (!lastTime) {
        lastTime = timestamp;
      }

      const delta = Math.min(32, timestamp - lastTime);
      lastTime = timestamp;

      setPhase((current) => (current + delta * 0.00135 * speed) % (Math.PI * 2));
      frameId = window.requestAnimationFrame(tick);
    };

    frameId = window.requestAnimationFrame(tick);

    return () => {
      window.cancelAnimationFrame(frameId);
    };
  }, [signal.latencyMs, signal.marketsTracked]);

  const chart = useMemo(() => {
    const values = signal.points;
    if (values.length === 0) {
      return {
        primaryPath: '',
        echoPath: '',
        glowPath: '',
        scanX: 0,
      };
    }

    const primaryPoints = buildSeries(values, phase, signal.latencyMs, signal.marketsTracked, 'primary');
    const echoPoints = buildSeries(values, phase + 0.5, signal.latencyMs, signal.marketsTracked, 'echo');
    const progress = phase / (Math.PI * 2);

    return {
      primaryPath: toPath(primaryPoints),
      echoPath: toPath(echoPoints),
      glowPath: toPath(primaryPoints),
      scanX: progress * CHART_WIDTH,
    };
  }, [phase, signal.latencyMs, signal.marketsTracked, signal.points]);

  return (
    <div className="grid gap-3 border border-border bg-bg-primary px-4 py-3 brutal-shadow sm:h-20 sm:grid-cols-[minmax(0,260px)_minmax(0,1fr)_minmax(0,240px)] sm:items-center sm:gap-4 sm:px-5 sm:py-0">
      <div className="min-w-0">
        <p className="text-[11px] uppercase tracking-[0.18em] text-text-secondary">
          SIGNAL_INPUT: {signal.label}
        </p>
        <p className="mt-1 truncate text-[11px] uppercase tracking-[0.18em] text-text-muted">
          LATENCY: {signal.latencyMs}MS
        </p>
      </div>

      <div className="min-w-0">
        <svg viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`} className="h-[30px] w-full sm:h-[34px]" preserveAspectRatio="none">
          <defs>
            <linearGradient id={`${gradientId}-beam`} x1="0" y1="0" x2="1" y2="0">
              <stop offset="0%" stopColor="var(--color-accent)" stopOpacity="0" />
              <stop offset="50%" stopColor="var(--color-accent)" stopOpacity="0.18" />
              <stop offset="100%" stopColor="var(--color-accent)" stopOpacity="0" />
            </linearGradient>
          </defs>
          <rect
            x={chart.scanX - SWEEP_WIDTH / 2}
            y="0"
            width={SWEEP_WIDTH}
            height={CHART_HEIGHT}
            fill={`url(#${gradientId}-beam)`}
            opacity="0.45"
          />
          <line
            x1={chart.scanX}
            y1="2"
            x2={chart.scanX}
            y2={CHART_HEIGHT - 2}
            stroke="var(--color-accent)"
            strokeOpacity="0.12"
            strokeWidth="1"
          />
          <path
            d={chart.glowPath}
            fill="none"
            stroke="var(--color-accent)"
            strokeOpacity="0.09"
            strokeWidth="4.8"
            strokeLinecap="round"
          />
          <path
            d={chart.echoPath}
            fill="none"
            stroke="var(--color-accent)"
            strokeOpacity="0.22"
            strokeWidth="1.15"
            strokeLinecap="round"
          />
          <path
            d={chart.primaryPath}
            fill="none"
            stroke="var(--color-accent)"
            strokeWidth="1.95"
            strokeLinecap="round"
          />
        </svg>
      </div>

      <div className="min-w-0 sm:text-right">
        <p className="truncate text-[11px] uppercase tracking-[0.18em] text-text-primary">
          {signal.source}
        </p>
        <p className="mt-1 truncate text-[11px] uppercase tracking-[0.18em] text-text-muted">
          {signal.marketsTracked} live markets | {signal.feedsLive}/{signal.feedsExpected} feeds live | {signal.stageLabel} | {formatTimestamp(signal.updatedAt)}
        </p>
      </div>
    </div>
  );
}

export function FeaturedBanner({ initialSignal }: SignalChartProps) {
  return <SignalChart initialSignal={initialSignal} />;
}
