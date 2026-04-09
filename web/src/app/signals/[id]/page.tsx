'use client';

import Link from 'next/link';
import { useParams } from 'next/navigation';
import { useMemo } from 'react';
import { PageShell } from '@/components/layout';
import { LoadingScreen } from '@/components/ui';
import { useSignalProviders } from '@/hooks';
import { cn } from '@/lib/utils';
import type { SignalProvider } from '@/types';

function formatBrier(score: number | null | undefined): string {
  if (score == null || !Number.isFinite(score)) return '--';
  return score.toFixed(3);
}

function formatTimestamp(value: string | null | undefined): string {
  if (!value) return '--';
  const d = new Date(value);
  if (!Number.isFinite(d.getTime())) return '--';
  return d.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function StatCard({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone?: 'positive' | 'negative' | 'neutral';
}) {
  return (
    <div className="border border-border bg-bg-secondary/40 px-4 py-3">
      <div className="text-[10px] uppercase tracking-[0.18em] text-text-muted font-mono">
        {label}
      </div>
      <div
        className={cn(
          'mt-1 text-lg font-mono font-medium',
          tone === 'positive' && 'text-green-400',
          tone === 'negative' && 'text-red-400',
          (!tone || tone === 'neutral') && 'text-text-primary',
        )}
      >
        {value}
      </div>
    </div>
  );
}

function brierTone(score: number | null | undefined): 'positive' | 'negative' | 'neutral' {
  if (score == null) return 'neutral';
  if (score <= 0.25) return 'positive';
  if (score >= 0.5) return 'negative';
  return 'neutral';
}

export default function SignalProviderDetailPage() {
  const params = useParams();
  const providerId = decodeURIComponent(params.id as string);

  const { data: providersData, isLoading: loadingProviders } = useSignalProviders({ limit: 200 });
  const provider: SignalProvider | null = useMemo(
    () => providersData?.providers.find((p) => p.id === providerId) ?? null,
    [providersData, providerId],
  );

  if (loadingProviders) {
    return (
      <PageShell>
        <LoadingScreen />
      </PageShell>
    );
  }

  if (!provider) {
    return (
      <PageShell>
        <div className="text-center py-12">
          <h2 className="text-xl font-semibold mb-2">Provider not found</h2>
          <Link href="/signals" className="text-accent hover:text-accent-hover">
            Back to Signals
          </Link>
        </div>
      </PageShell>
    );
  }

  const tone = brierTone(provider.avgBrierScore);

  return (
    <PageShell>
      <Link
        href="/signals"
        className="inline-flex items-center gap-2 p-1 -ml-1 text-text-secondary hover:text-text-primary mb-4"
      >
        <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
        </svg>
        Back to Signals
      </Link>

      <div className="flex flex-wrap items-center gap-3 mb-6">
        <h1 className="text-xl font-semibold text-text-primary">
          {provider.name}
        </h1>
        <span
          className={cn(
            'inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-mono uppercase',
            provider.active
              ? 'border-green-500/30 bg-green-500/10 text-green-400'
              : 'border-border bg-bg-secondary text-text-muted',
          )}
        >
          {provider.active && (
            <span className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
          )}
          {provider.active ? 'active' : 'inactive'}
        </span>
        <span className="text-xs font-mono uppercase tracking-wider text-text-muted">
          {provider.category}
        </span>
      </div>

      {provider.description ? (
        <p className="mb-6 text-sm text-text-secondary">{provider.description}</p>
      ) : null}

      <div className="grid grid-cols-2 sm:grid-cols-3 gap-3 mb-6">
        <StatCard
          label="Brier (lower is better)"
          value={formatBrier(provider.avgBrierScore)}
          tone={tone}
        />
        <StatCard
          label="Scored Signals"
          value={String(provider.scoredSignals ?? 0)}
        />
        <StatCard
          label="Created"
          value={formatTimestamp(provider.createdAt)}
        />
      </div>

      <div className="grid sm:grid-cols-2 gap-6 mb-6">
        <div className="border border-border bg-bg-secondary/40 p-5">
          <h2 className="text-sm font-medium text-text-primary mb-4">
            Provider Details
          </h2>
          <dl className="space-y-3 text-sm">
            <div className="flex justify-between">
              <dt className="text-text-muted">Owner</dt>
              <dd className="text-text-primary font-mono text-xs">
                {provider.owner.slice(0, 6)}...{provider.owner.slice(-4)}
              </dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Category</dt>
              <dd className="text-text-primary font-mono capitalize">
                {provider.category}
              </dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Update Frequency</dt>
              <dd className="text-text-primary font-mono">
                {provider.updateFrequencySecs}s
              </dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Provider ID</dt>
              <dd className="text-text-primary font-mono text-xs">
                {provider.id.slice(0, 8)}...
              </dd>
            </div>
          </dl>
        </div>

        <div className="border border-border bg-bg-secondary/40 p-5">
          <h2 className="text-sm font-medium text-text-primary mb-4">
            Performance
          </h2>
          <dl className="space-y-3 text-sm">
            <div className="flex justify-between">
              <dt className="text-text-muted">Avg Brier Score</dt>
              <dd
                className={cn(
                  'font-mono',
                  tone === 'positive' && 'text-green-400',
                  tone === 'negative' && 'text-red-400',
                  tone === 'neutral' && 'text-text-primary',
                )}
              >
                {formatBrier(provider.avgBrierScore)}
              </dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Scored Signals</dt>
              <dd className="text-text-primary font-mono">
                {provider.scoredSignals ?? 0}
              </dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Active</dt>
              <dd className="text-text-primary font-mono">
                {provider.active ? 'Yes' : 'No'}
              </dd>
            </div>
          </dl>
        </div>
      </div>

      <div className="border border-border bg-bg-secondary/40 p-5">
        <h2 className="text-sm font-medium text-text-primary mb-4">
          Recent Emissions
        </h2>
        <p className="text-xs text-text-muted">
          Emission history is scoped per market. Visit a market page to see this provider's signals for that specific market.
        </p>
      </div>
    </PageShell>
  );
}
