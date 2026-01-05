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
              <h1 className="mt-2 text-3xl font-semibold text-text-primary">Threshold-driven decision systems</h1>
              <p className="mt-3 text-sm text-text-secondary">
                Turn one decision into linked uncertainty nodes, action scores, alerts, and
                automation rules for attached external agents.
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
              <div className="text-[11px] uppercase tracking-[0.18em] text-text-muted">automation on</div>
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
