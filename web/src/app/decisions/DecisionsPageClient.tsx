'use client';

import Link from 'next/link';

import { DecisionAccessGate, DecisionCellCard } from '@/components/decision';
import { PageShell } from '@/components/layout';
import { Card } from '@/components/ui';
import { useDecisionCells, useSessionState } from '@/hooks';

export default function DecisionsPageClient() {
  const { hasSession, sessionRestored } = useSessionState();
  const { data, isLoading, error } = useDecisionCells({
    limit: 50,
    enabled: hasSession && sessionRestored,
  });

  const cells = data?.data ?? [];
  const automated = cells.filter((cell) => cell.automationEnabled).length;
  const insufficientSignal = cells.filter(
    (cell) => cell.recommendation.state === 'insufficient_signal',
  ).length;

  return (
    <PageShell>
      <DecisionAccessGate>
        <div className="space-y-6">
          <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
            <div className="max-w-3xl">
              <p className="text-[11px] uppercase tracking-[0.22em] text-text-muted">decision cells</p>
              <h1 className="mt-2 text-3xl font-semibold text-text-primary">Decision cells</h1>
              <p className="mt-3 text-sm text-text-secondary">
                Private workspaces for linking markets, alerts, and external agent actions around
                one decision.
              </p>
            </div>
            <Link
              href="/decisions/create"
              className="inline-flex h-10 items-center border border-accent bg-accent px-4 text-sm font-medium uppercase tracking-[0.12em] text-white transition-colors hover:bg-accent-hover"
            >
              Create decision cell
            </Link>
          </div>

          <div className="grid gap-4 md:grid-cols-3">
            <Card>
              <div className="text-[11px] uppercase tracking-[0.18em] text-text-muted">cells</div>
              <div className="mt-2 text-3xl font-semibold text-text-primary">{cells.length}</div>
            </Card>
            <Card>
              <div className="text-[11px] uppercase tracking-[0.18em] text-text-muted">automation active</div>
              <div className="mt-2 text-3xl font-semibold text-text-primary">{automated}</div>
            </Card>
            <Card>
              <div className="text-[11px] uppercase tracking-[0.18em] text-text-muted">needs signal</div>
              <div className="mt-2 text-3xl font-semibold text-text-primary">{insufficientSignal}</div>
            </Card>
          </div>

          {isLoading ? (
            <Card>Loading decision cells...</Card>
          ) : error ? (
            <Card className="text-ask">
              {error instanceof Error ? error.message : 'Failed to load decision cells'}
            </Card>
          ) : cells.length === 0 ? (
            <Card className="space-y-4">
              <h2 className="text-xl font-semibold text-text-primary">No decision cells yet</h2>
              <p className="text-sm text-text-secondary">
                Start with a timing, choice, hedge, or allocation decision. The app will create
                starter nodes you can connect to markets and agents.
              </p>
              <Link
                href="/decisions/create"
                className="inline-flex h-10 items-center border border-accent bg-accent px-4 text-sm font-medium uppercase tracking-[0.12em] text-white transition-colors hover:bg-accent-hover"
              >
                Create your first cell
              </Link>
            </Card>
          ) : (
            <div className="grid gap-4 xl:grid-cols-2">
              {cells.map((cell) => (
                <DecisionCellCard key={cell.id} cell={cell} />
              ))}
            </div>
          )}
        </div>
      </DecisionAccessGate>
    </PageShell>
  );
}
