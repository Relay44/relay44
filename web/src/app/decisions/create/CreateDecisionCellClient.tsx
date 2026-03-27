'use client';

import { useMemo, useState } from 'react';
import { useRouter } from 'next/navigation';

import { DecisionAccessGate } from '@/components/decision';
import { PageShell } from '@/components/layout';
import { Button, Card, Input, Select, useToast } from '@/components/ui';
import { useCreateDecisionCell } from '@/hooks';
import type { DecisionType } from '@/types';

const DECISION_TYPE_OPTIONS = [
  { value: 'timing', label: 'Timing' },
  { value: 'choice', label: 'Choice' },
  { value: 'hedge', label: 'Hedge' },
  { value: 'allocation', label: 'Allocation' },
] as const;

const STARTER_NODE_COPY: Record<DecisionType, string[]> = {
  timing: ['Catalyst confirmed', 'Negative blocker emerges', 'Broader trend persists'],
  choice: ['Outcome driver A', 'Cost or risk driver', 'External validation'],
  hedge: ['Downside event', 'Hedge cost pressure', 'Correlation breakdown'],
  allocation: ['Upside catalyst', 'Downside catalyst', 'Liquidity or exit condition'],
};

function toIsoDatetime(value: string): string | undefined {
  if (!value.trim()) {
    return undefined;
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return undefined;
  }

  return parsed.toISOString();
}

export default function CreateDecisionCellClient() {
  const router = useRouter();
  const { addToast } = useToast();
  const createCell = useCreateDecisionCell();

  const [title, setTitle] = useState('');
  const [statement, setStatement] = useState('');
  const [decisionType, setDecisionType] = useState<DecisionType>('timing');
  const [horizonAt, setHorizonAt] = useState('');
  const [actions, setActions] = useState(['', '', '']);

  const starterNodes = useMemo(() => STARTER_NODE_COPY[decisionType], [decisionType]);
  const usesDefaultTimingActions = decisionType === 'timing';

  const handleActionChange = (index: number, next: string) => {
    setActions((current) => current.map((value, valueIndex) => (valueIndex === index ? next : value)));
  };

  const handleSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    const trimmedTitle = title.trim();
    const trimmedStatement = statement.trim();
    if (!trimmedTitle || !trimmedStatement) {
      addToast('Title and statement are required.', 'error');
      return;
    }

    const nextActions = usesDefaultTimingActions
      ? undefined
      : actions.map((value) => value.trim()).filter(Boolean);

    if (!usesDefaultTimingActions && (nextActions?.length ?? 0) < 2) {
      addToast('Choice, hedge, and allocation cells need at least two actions.', 'error');
      return;
    }

    try {
      const cell = await createCell.mutateAsync({
        title: trimmedTitle,
        statement: trimmedStatement,
        decisionType,
        horizonAt: toIsoDatetime(horizonAt),
        actions: nextActions,
      });
      router.push(`/decisions/${encodeURIComponent(cell.id)}`);
    } catch (error) {
      addToast(error instanceof Error ? error.message : 'Failed to create decision cell.', 'error');
    }
  };

  return (
    <PageShell>
      <DecisionAccessGate>
        <div className="mx-auto max-w-4xl space-y-6">
          <div className="max-w-2xl">
            <p className="text-[11px] uppercase tracking-[0.22em] text-text-muted">decision cells</p>
            <h1 className="mt-2 text-3xl font-semibold text-text-primary">Create a decision cell</h1>
            <p className="mt-3 text-sm text-text-secondary">
              Define the decision, choose the action set, and create starter nodes you can connect
              to live markets and external agents.
            </p>
          </div>

          <form className="grid gap-6 xl:grid-cols-[minmax(0,2fr),minmax(320px,1fr)]" onSubmit={handleSubmit}>
            <Card className="space-y-5">
              <Input
                label="Title"
                value={title}
                onChange={(event) => setTitle(event.target.value)}
                placeholder="Ship the Base campaign in April"
                maxLength={160}
              />

              <div className="space-y-1.5">
                <label className="block text-sm font-medium text-text-primary" htmlFor="decision-statement">
                  Problem statement
                </label>
                <textarea
                  id="decision-statement"
                  value={statement}
                  onChange={(event) => setStatement(event.target.value)}
                  rows={5}
                  maxLength={2000}
                  placeholder="Describe the decision, what is at stake, and what the cell is supposed to optimize."
                  className="flex w-full border border-border bg-bg-secondary px-3 py-2 text-base text-text-primary placeholder:text-text-muted transition-all duration-fast focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-1 focus-visible:ring-offset-bg-base focus-visible:border-accent"
                />
              </div>

              <div className="grid gap-4 sm:grid-cols-2">
                <div className="space-y-1.5">
                  <label className="block text-sm font-medium text-text-primary">Decision type</label>
                  <Select
                    value={decisionType}
                    onChange={(event) => setDecisionType(event.target.value as DecisionType)}
                    options={DECISION_TYPE_OPTIONS.map((option) => ({ value: option.value, label: option.label }))}
                  />
                </div>
                <Input
                  label="Horizon"
                  type="datetime-local"
                  value={horizonAt}
                  onChange={(event) => setHorizonAt(event.target.value)}
                />
              </div>

              <div className="space-y-3">
                <div>
                  <h2 className="text-sm font-medium text-text-primary">Actions</h2>
                  <p className="mt-1 text-sm text-text-secondary">
                    {usesDefaultTimingActions
                      ? 'Timing cells default to “act now” and “wait”.'
                      : 'Set two or three actions the score engine can rank.'}
                  </p>
                </div>

                {usesDefaultTimingActions ? (
                  <div className="grid gap-3 sm:grid-cols-2">
                    <div className="border border-border bg-bg-secondary px-3 py-3 text-sm text-text-primary">act now</div>
                    <div className="border border-border bg-bg-secondary px-3 py-3 text-sm text-text-primary">wait</div>
                  </div>
                ) : (
                  <div className="grid gap-3">
                    <Input
                      label="Action 1"
                      value={actions[0]}
                      onChange={(event) => handleActionChange(0, event.target.value)}
                      placeholder="Increase allocation"
                    />
                    <Input
                      label="Action 2"
                      value={actions[1]}
                      onChange={(event) => handleActionChange(1, event.target.value)}
                      placeholder="Hold flat"
                    />
                    <Input
                      label="Action 3 (optional)"
                      value={actions[2]}
                      onChange={(event) => handleActionChange(2, event.target.value)}
                      placeholder="Reduce exposure"
                    />
                  </div>
                )}
              </div>

              <div className="flex flex-wrap gap-3">
                <Button type="submit" loading={createCell.isPending}>
                  Create decision cell
                </Button>
                <button
                  type="button"
                  onClick={() => router.push('/decisions')}
                  className="inline-flex h-10 items-center border border-border px-4 text-sm uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
                >
                  Cancel
                </button>
              </div>
            </Card>

            <Card className="space-y-4">
              <div>
                <p className="text-[11px] uppercase tracking-[0.22em] text-text-muted">starter nodes</p>
                <h2 className="mt-2 text-lg font-semibold text-text-primary">Initial node set</h2>
              </div>
              <div className="space-y-3">
                {starterNodes.map((label, index) => (
                  <div key={label} className="border border-border bg-bg-secondary px-4 py-3">
                    <div className="text-[11px] uppercase tracking-[0.18em] text-text-muted">node {index + 1}</div>
                    <div className="mt-1 text-sm font-medium text-text-primary">{label}</div>
                  </div>
                ))}
              </div>
              <p className="text-sm text-text-secondary">
                After creation, attach live markets, set action effects, configure alerts, and
                connect external agents.
              </p>
            </Card>
          </form>
        </div>
      </DecisionAccessGate>
    </PageShell>
  );
}
