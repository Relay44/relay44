'use client';

import Link from 'next/link';
import { useMemo, useState } from 'react';
import { PageShell } from '@/components/layout';
import { Button, LoadingScreen } from '@/components/ui';
import {
  useSignalProviders,
  useCreateSignalProvider,
  useSessionState,
} from '@/hooks';
import { useToast } from '@/components/ui';
import { cn } from '@/lib/utils';
import type { SignalProvider, SignalProviderFilters } from '@/types';

function formatBrier(score: number | null | undefined): string {
  if (score == null || !Number.isFinite(score)) return '--';
  return score.toFixed(3);
}

function brierTone(score: number | null | undefined): 'positive' | 'negative' | 'neutral' {
  if (score == null) return 'neutral';
  if (score <= 0.25) return 'positive';
  if (score >= 0.5) return 'negative';
  return 'neutral';
}

function ProviderCard({ provider }: { provider: SignalProvider }) {
  const tone = brierTone(provider.avgBrierScore);

  return (
    <Link
      href={`/signals/${encodeURIComponent(provider.id)}`}
      className="block border border-border bg-bg-primary p-5 transition-colors hover:border-border-hover hover:bg-bg-secondary"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-sm font-medium uppercase tracking-[0.12em] text-text-primary">
            {provider.name}
          </h3>
          {provider.description ? (
            <p className="mt-1 line-clamp-2 text-xs text-text-muted">
              {provider.description}
            </p>
          ) : null}
        </div>
        <span className="shrink-0 border border-border bg-bg-secondary px-2 py-0.5 text-[10px] uppercase tracking-[0.18em] text-text-muted">
          {provider.category}
        </span>
      </div>

      <div className="mt-4 grid grid-cols-2 gap-3">
        <div>
          <div className="text-[10px] uppercase tracking-[0.18em] text-text-muted font-mono">
            Brier (lower is better)
          </div>
          <div
            className={cn(
              'mt-0.5 text-sm font-mono font-medium',
              tone === 'positive' && 'text-green-400',
              tone === 'negative' && 'text-red-400',
              tone === 'neutral' && 'text-text-primary',
            )}
          >
            {formatBrier(provider.avgBrierScore)}
          </div>
        </div>
        <div>
          <div className="text-[10px] uppercase tracking-[0.18em] text-text-muted font-mono">
            Scored
          </div>
          <div className="mt-0.5 text-sm font-mono font-medium text-text-primary">
            {provider.scoredSignals ?? 0}
          </div>
        </div>
      </div>
    </Link>
  );
}

export default function SignalsPageClient() {
  const [categoryFilter, setCategoryFilter] = useState<string>('');
  const { hasSession } = useSessionState();
  const { addToast } = useToast();

  const filters: SignalProviderFilters = useMemo(() => {
    const f: SignalProviderFilters = { limit: 100 };
    if (categoryFilter) f.category = categoryFilter;
    return f;
  }, [categoryFilter]);

  const { data, isLoading, error } = useSignalProviders(filters);
  const providers = data?.providers ?? [];

  const [showCreateForm, setShowCreateForm] = useState(false);
  const [newName, setNewName] = useState('');
  const [newDescription, setNewDescription] = useState('');
  const [newCategory, setNewCategory] = useState('');
  const createMutation = useCreateSignalProvider();

  const categories = useMemo(() => {
    const cats = new Set<string>();
    for (const p of providers) {
      if (p.category) cats.add(p.category);
    }
    return Array.from(cats).sort();
  }, [providers]);

  async function handleCreate() {
    if (!newName.trim()) return;
    try {
      await createMutation.mutateAsync({
        name: newName.trim(),
        description: newDescription.trim() || undefined,
        category: newCategory.trim() || undefined,
      });
      setNewName('');
      setNewDescription('');
      setNewCategory('');
      setShowCreateForm(false);
    } catch (err) {
      addToast((err as Error)?.message ?? 'Failed to create provider', 'error');
    }
  }

  return (
    <PageShell>
      <div className="py-8 space-y-8">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div className="space-y-2">
            <h1 className="text-xl font-semibold">Signals</h1>
            <p className="text-text-secondary">
              Signal providers with Brier-scored prediction track records.
            </p>
          </div>
          {hasSession ? (
            <Button
              variant="primary"
              size="sm"
              onClick={() => setShowCreateForm((v) => !v)}
            >
              {showCreateForm ? 'Cancel' : 'Become a Provider'}
            </Button>
          ) : null}
        </div>

        {showCreateForm ? (
          <div className="border border-border bg-bg-secondary/40 p-5 space-y-4">
            <h2 className="text-sm font-medium uppercase tracking-[0.14em] text-text-primary">
              Register as Signal Provider
            </h2>
            <div className="grid sm:grid-cols-3 gap-4">
              <div>
                <label className="block text-[10px] uppercase tracking-[0.18em] text-text-muted mb-1">
                  Name
                </label>
                <input
                  type="text"
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  placeholder="My Signal Feed"
                  maxLength={128}
                  className="w-full border border-border bg-bg-primary px-3 py-2 text-sm text-text-primary placeholder:text-text-muted focus:border-accent focus:outline-none"
                />
              </div>
              <div>
                <label className="block text-[10px] uppercase tracking-[0.18em] text-text-muted mb-1">
                  Category
                </label>
                <input
                  type="text"
                  value={newCategory}
                  onChange={(e) => setNewCategory(e.target.value)}
                  placeholder="general"
                  className="w-full border border-border bg-bg-primary px-3 py-2 text-sm text-text-primary placeholder:text-text-muted focus:border-accent focus:outline-none"
                />
              </div>
              <div className="flex items-end">
                <Button
                  variant="primary"
                  size="sm"
                  loading={createMutation.isPending}
                  onClick={handleCreate}
                  disabled={!newName.trim()}
                >
                  Register
                </Button>
              </div>
            </div>
            <div>
              <label className="block text-[10px] uppercase tracking-[0.18em] text-text-muted mb-1">
                Description
              </label>
              <input
                type="text"
                value={newDescription}
                onChange={(e) => setNewDescription(e.target.value)}
                placeholder="Optional description of your signal methodology"
                className="w-full border border-border bg-bg-primary px-3 py-2 text-sm text-text-primary placeholder:text-text-muted focus:border-accent focus:outline-none"
              />
            </div>
            {createMutation.isError ? (
              <p className="text-xs text-red-400">
                {(createMutation.error as Error)?.message ?? 'Failed to create provider'}
              </p>
            ) : null}
          </div>
        ) : null}

        {categories.length > 0 ? (
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={categoryFilter}
              onChange={(e) => setCategoryFilter(e.target.value)}
              className="border border-border bg-bg-primary px-3 py-2 text-xs uppercase tracking-[0.14em] text-text-secondary focus:border-accent focus:outline-none"
            >
              <option value="">All categories</option>
              {categories.map((cat) => (
                <option key={cat} value={cat}>
                  {cat}
                </option>
              ))}
            </select>
          </div>
        ) : null}

        {isLoading ? (
          <LoadingScreen />
        ) : error ? (
          <div className="py-12 text-center text-text-muted">
            Failed to load signal providers.
          </div>
        ) : providers.length === 0 ? (
          <div className="py-12 text-center text-text-muted">
            No signal providers found.
          </div>
        ) : (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {providers.map((provider) => (
              <ProviderCard key={provider.id} provider={provider} />
            ))}
          </div>
        )}
      </div>
    </PageShell>
  );
}
