'use client';

import Link from 'next/link';
import { useEffect, useMemo, useState } from 'react';

import { DecisionAccessGate, DecisionScoreBar } from '@/components/decision';
import { PageShell } from '@/components/layout';
import { Button, Card, Input, Select, useToast } from '@/components/ui';
import {
  useAddDecisionNode,
  useAttachDecisionAgent,
  useAttachDecisionMarket,
  useDecisionCell,
  useExternalAgents,
  useMarkets,
  useRecalculateDecisionCell,
  useSessionState,
  useUpdateDecisionAutomation,
  useUpdateDecisionNode,
  useUpsertDecisionAlert,
} from '@/hooks';
import type { ExternalAgentRecord } from '@/lib/api';
import type {
  DecisionAutomationPolicy,
  DecisionCell,
  DecisionNode,
  DecisionNodeEffect,
  DecisionNodeSourceType,
  DecisionTriggerMode,
} from '@/types';

const SOURCE_TYPE_OPTIONS = [
  { value: 'draft_market', label: 'Draft node' },
  { value: 'internal_market', label: 'Internal market' },
  { value: 'external_market', label: 'External market' },
] as const;

const EFFECT_OPTIONS = [
  { value: 'support', label: 'Support' },
  { value: 'oppose', label: 'Oppose' },
  { value: 'neutral', label: 'Neutral' },
] as const;

const TRIGGER_MODE_OPTIONS = [
  { value: 'on_recommendation_gain', label: 'Recommendation gain' },
  { value: 'on_threshold_cross', label: 'Threshold cross' },
  { value: 'on_confidence_gain', label: 'Confidence gain' },
] as const;

const PROVIDER_OPTIONS = [
  { value: 'all', label: 'Any provider' },
  { value: 'limitless', label: 'Limitless' },
  { value: 'polymarket', label: 'Polymarket' },
] as const;

const DIRECTION_OPTIONS = [
  { value: 'above', label: 'Cross above' },
  { value: 'below', label: 'Cross below' },
] as const;

function toPercent(bps: number) {
  return (bps / 100).toFixed(1);
}

function parsePercent(value: string, fallback = 0) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  return Math.max(0, Math.min(10000, Math.round(parsed * 100)));
}

function formatState(state: string) {
  return state.replace(/_/g, ' ');
}

function summarizePayload(payload: Record<string, unknown>) {
  const pairs = Object.entries(payload)
    .slice(0, 3)
    .map(([key, value]) => `${key}: ${typeof value === 'object' ? JSON.stringify(value) : String(value)}`);
  return pairs.join(' • ');
}

function defaultNodeEffects(cell: DecisionCell) {
  return Object.fromEntries(
    cell.actions.map((action, index) => [action.label, index === 0 ? 'support' : 'neutral']),
  ) as Record<string, DecisionNodeEffect>;
}

interface DecisionNodeCardProps {
  cell: DecisionCell;
  node: DecisionNode;
  internalMarketOptions: Array<{ value: string; label: string }>;
  externalMarketOptions: Array<{ value: string; label: string }>;
  agentOptions: ExternalAgentRecord[];
}

function DecisionNodeCard({
  cell,
  node,
  internalMarketOptions,
  externalMarketOptions,
  agentOptions,
}: DecisionNodeCardProps) {
  const { addToast } = useToast();
  const updateNode = useUpdateDecisionNode(cell.id, node.id);
  const attachMarket = useAttachDecisionMarket(cell.id, node.id);
  const attachAgent = useAttachDecisionAgent(cell.id, node.id);

  const [label, setLabel] = useState(node.label);
  const [description, setDescription] = useState(node.description);
  const [weightBps, setWeightBps] = useState(String(node.weightBps));
  const [sourceType, setSourceType] = useState<DecisionNodeSourceType>(node.sourceType);
  const [sourceRef, setSourceRef] = useState(node.sourceRef || '');
  const [selectedAgentId, setSelectedAgentId] = useState('');
  const [triggerMode, setTriggerMode] = useState<DecisionTriggerMode>('on_threshold_cross');
  const [actionEffects, setActionEffects] = useState<Record<string, DecisionNodeEffect>>(() => {
    const next = { ...defaultNodeEffects(cell) };
    for (const action of cell.actions) {
      const effect = node.actionEffects[action.label];
      if (effect === 'support' || effect === 'oppose' || effect === 'neutral') {
        next[action.label] = effect;
      }
    }
    return next;
  });

  useEffect(() => {
    setLabel(node.label);
    setDescription(node.description);
    setWeightBps(String(node.weightBps));
    setSourceType(node.sourceType);
    setSourceRef(node.sourceRef || '');
    setActionEffects((current) => {
      const next = { ...current };
      for (const action of cell.actions) {
        const effect = node.actionEffects[action.label];
        next[action.label] =
          effect === 'support' || effect === 'oppose' || effect === 'neutral'
            ? effect
            : next[action.label] || 'neutral';
      }
      return next;
    });
  }, [cell.actions, node]);

  const marketOptions = sourceType === 'internal_market' ? internalMarketOptions : externalMarketOptions;

  const handleSaveNode = async () => {
    try {
      await updateNode.mutateAsync({
        label,
        description,
        weightBps: Math.max(0, Math.min(10000, Number(weightBps) || 0)),
        sourceType: sourceType === 'draft_market' ? 'draft_market' : undefined,
        sourceRef: sourceType === 'draft_market' ? undefined : node.sourceRef,
        actionEffects,
      });
      addToast('Node updated.', 'success');
    } catch (error) {
      addToast(error instanceof Error ? error.message : 'Failed to update node.', 'error');
    }
  };

  const handleAttachMarket = async () => {
    if (sourceType === 'draft_market' || !sourceRef) {
      addToast('Choose a live market before attaching.', 'error');
      return;
    }

    try {
      await attachMarket.mutateAsync({ sourceType, sourceRef });
      addToast('Market attached to node.', 'success');
    } catch (error) {
      addToast(error instanceof Error ? error.message : 'Failed to attach market.', 'error');
    }
  };

  const handleAttachAgent = async () => {
    if (!selectedAgentId) {
      addToast('Choose an external agent first.', 'error');
      return;
    }

    try {
      await attachAgent.mutateAsync({
        externalAgentId: selectedAgentId,
        triggerMode,
        active: true,
      });
      setSelectedAgentId('');
      addToast('External agent attached.', 'success');
    } catch (error) {
      addToast(error instanceof Error ? error.message : 'Failed to attach external agent.', 'error');
    }
  };

  return (
    <Card className="space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="text-[11px] uppercase tracking-[0.18em] text-text-muted">{formatState(node.sourceType)}</p>
          <h3 className="mt-2 text-lg font-semibold text-text-primary">{node.label}</h3>
          <p className="mt-2 text-sm text-text-secondary">{node.description}</p>
        </div>
        <div className="text-right text-sm">
          <div className="text-text-secondary">Weight</div>
          <div className="font-semibold text-text-primary">{toPercent(node.weightBps)}%</div>
          {typeof node.lastProbabilityBps === 'number' ? (
            <div className="mt-1 text-text-secondary">P(yes): {toPercent(node.lastProbabilityBps)}%</div>
          ) : null}
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <Input label="Node label" value={label} onChange={(event) => setLabel(event.target.value)} />
        <Input
          label="Weight (%)"
          type="number"
          min="0"
          max="100"
          step="0.1"
          value={String((Number(weightBps) || 0) / 100)}
          onChange={(event) => setWeightBps(String(parsePercent(event.target.value, node.weightBps)))}
        />
      </div>

      <div className="space-y-1.5">
        <label className="block text-sm font-medium text-text-primary">Description</label>
        <textarea
          value={description}
          onChange={(event) => setDescription(event.target.value)}
          rows={3}
          className="flex w-full border border-border bg-bg-secondary px-3 py-2 text-base text-text-primary placeholder:text-text-muted transition-all duration-fast focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-1 focus-visible:ring-offset-bg-base focus-visible:border-accent"
        />
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <div className="space-y-1.5">
          <label className="block text-sm font-medium text-text-primary">Source type</label>
          <Select
            value={sourceType}
            onChange={(event) => setSourceType(event.target.value as DecisionNodeSourceType)}
            options={SOURCE_TYPE_OPTIONS.map((option) => ({ value: option.value, label: option.label }))}
          />
        </div>
        {sourceType === 'draft_market' ? (
          <div className="space-y-1.5">
            <label className="block text-sm font-medium text-text-primary">Draft market</label>
            <Link
              href={`/markets/create?question=${encodeURIComponent(label)}&description=${encodeURIComponent(description)}`}
              className="inline-flex h-10 items-center border border-accent px-4 text-sm uppercase tracking-[0.12em] text-accent transition-colors hover:bg-accent/10"
            >
              Open market draft
            </Link>
          </div>
        ) : (
          <div className="space-y-1.5">
            <label className="block text-sm font-medium text-text-primary">Linked market</label>
            <Select
              value={sourceRef}
              onChange={(event) => setSourceRef(event.target.value)}
              placeholder="Choose a live market"
              options={marketOptions}
            />
          </div>
        )}
      </div>

      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
        {cell.actions.map((action) => (
          <div key={action.id} className="space-y-1.5">
            <label className="block text-sm font-medium text-text-primary">{action.label}</label>
            <Select
              value={actionEffects[action.label] || 'neutral'}
              onChange={(event) => {
                const next = event.target.value as DecisionNodeEffect;
                setActionEffects((current) => ({ ...current, [action.label]: next }));
              }}
              options={EFFECT_OPTIONS.map((option) => ({ value: option.value, label: option.label }))}
            />
          </div>
        ))}
      </div>

      <div className="flex flex-wrap gap-3">
        <Button type="button" onClick={() => void handleSaveNode()} loading={updateNode.isPending}>
          Save node
        </Button>
        {sourceType !== 'draft_market' ? (
          <button
            type="button"
            onClick={() => void handleAttachMarket()}
            disabled={attachMarket.isPending}
            className="inline-flex h-10 items-center border border-border px-4 text-sm uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary disabled:cursor-not-allowed disabled:opacity-50"
          >
            {attachMarket.isPending ? 'Attaching...' : 'Attach market'}
          </button>
        ) : null}
      </div>

      <div className="space-y-3 border-t border-border pt-4">
        <div>
          <h4 className="text-sm font-medium text-text-primary">Attached automations</h4>
          <p className="mt-1 text-sm text-text-secondary">
            Only attached external agents can auto-trigger from this node.
          </p>
        </div>

        {node.agents.length > 0 ? (
          <div className="space-y-2">
            {node.agents.map((agent) => (
              <div key={agent.id} className="border border-border bg-bg-secondary px-4 py-3 text-sm">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div>
                    <div className="font-medium text-text-primary">{agent.name || agent.externalAgentId}</div>
                    <div className="mt-1 text-text-secondary">
                      {agent.provider || 'unknown'} • {formatState(agent.triggerMode)} •{' '}
                      {agent.active ? 'active' : 'inactive'}
                    </div>
                  </div>
                  <div className="text-[11px] uppercase tracking-[0.18em] text-text-muted">
                    {agent.agentActive ? 'agent live' : 'agent paused'}
                  </div>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-sm text-text-secondary">No external agents attached to this node yet.</div>
        )}

        <div className="grid gap-4 md:grid-cols-[minmax(0,2fr),minmax(0,1fr),auto] md:items-end">
          <div className="space-y-1.5">
            <label className="block text-sm font-medium text-text-primary">External agent</label>
            <Select
              value={selectedAgentId}
              onChange={(event) => setSelectedAgentId(event.target.value)}
              placeholder="Choose an external agent"
              options={agentOptions.map((agent) => ({
                value: agent.id,
                label: `${agent.name} • ${agent.provider} • ${agent.market_id}`,
              }))}
            />
          </div>
          <div className="space-y-1.5">
            <label className="block text-sm font-medium text-text-primary">Trigger mode</label>
            <Select
              value={triggerMode}
              onChange={(event) => setTriggerMode(event.target.value as DecisionTriggerMode)}
              options={TRIGGER_MODE_OPTIONS.map((option) => ({ value: option.value, label: option.label }))}
            />
          </div>
          <Button type="button" onClick={() => void handleAttachAgent()} loading={attachAgent.isPending}>
            Attach agent
          </Button>
        </div>
      </div>
    </Card>
  );
}

interface AutomationEditorProps {
  policy: DecisionAutomationPolicy;
  onSave: (payload: {
    automationEnabled: boolean;
    maxAgentNotionalUsdc: number;
    maxTriggersPerDay: number;
    minTriggerIntervalSeconds: number;
    allowedProvider?: 'limitless' | 'polymarket';
    requireConfidenceBps: number;
    active: boolean;
  }) => Promise<void>;
  saving: boolean;
}

function AutomationEditor({ policy, onSave, saving }: AutomationEditorProps) {
  const [automationEnabled, setAutomationEnabled] = useState(policy.automationEnabled);
  const [active, setActive] = useState(policy.active);
  const [maxAgentNotionalUsdc, setMaxAgentNotionalUsdc] = useState(String(policy.maxAgentNotionalUsdc));
  const [maxTriggersPerDay, setMaxTriggersPerDay] = useState(String(policy.maxTriggersPerDay));
  const [minTriggerIntervalSeconds, setMinTriggerIntervalSeconds] = useState(
    String(policy.minTriggerIntervalSeconds),
  );
  const [requireConfidenceBps, setRequireConfidenceBps] = useState(String(policy.requireConfidenceBps / 100));
  const [allowedProvider, setAllowedProvider] = useState(policy.allowedProvider || 'all');

  useEffect(() => {
    setAutomationEnabled(policy.automationEnabled);
    setActive(policy.active);
    setMaxAgentNotionalUsdc(String(policy.maxAgentNotionalUsdc));
    setMaxTriggersPerDay(String(policy.maxTriggersPerDay));
    setMinTriggerIntervalSeconds(String(policy.minTriggerIntervalSeconds));
    setRequireConfidenceBps(String(policy.requireConfidenceBps / 100));
    setAllowedProvider(policy.allowedProvider || 'all');
  }, [policy]);

  return (
    <Card className="space-y-4">
      <div>
        <p className="text-[11px] uppercase tracking-[0.22em] text-text-muted">automation policy</p>
        <h2 className="mt-2 text-xl font-semibold text-text-primary">External-agent trigger limits</h2>
        <p className="mt-2 text-sm text-text-secondary">
          Decision cells can only auto-trigger attached external agents in V1. They cannot place
          direct user trades or move funds.
        </p>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <label className="flex items-center gap-3 border border-border bg-bg-secondary px-4 py-3 text-sm text-text-primary">
          <input
            type="checkbox"
            checked={automationEnabled}
            onChange={(event) => setAutomationEnabled(event.target.checked)}
            className="h-4 w-4 accent-[var(--accent)]"
          />
          Automation enabled
        </label>
        <label className="flex items-center gap-3 border border-border bg-bg-secondary px-4 py-3 text-sm text-text-primary">
          <input
            type="checkbox"
            checked={active}
            onChange={(event) => setActive(event.target.checked)}
            className="h-4 w-4 accent-[var(--accent)]"
          />
          Policy active
        </label>
      </div>

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        <Input
          label="Max notional (USDC)"
          type="number"
          min="0"
          step="0.01"
          value={maxAgentNotionalUsdc}
          onChange={(event) => setMaxAgentNotionalUsdc(event.target.value)}
        />
        <Input
          label="Max triggers per day"
          type="number"
          min="0"
          step="1"
          value={maxTriggersPerDay}
          onChange={(event) => setMaxTriggersPerDay(event.target.value)}
        />
        <Input
          label="Min trigger interval (seconds)"
          type="number"
          min="0"
          step="1"
          value={minTriggerIntervalSeconds}
          onChange={(event) => setMinTriggerIntervalSeconds(event.target.value)}
        />
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <Input
          label="Required confidence (%)"
          type="number"
          min="0"
          max="100"
          step="0.1"
          value={requireConfidenceBps}
          onChange={(event) => setRequireConfidenceBps(event.target.value)}
        />
        <div className="space-y-1.5">
          <label className="block text-sm font-medium text-text-primary">Allowed provider</label>
          <Select
            value={allowedProvider}
            onChange={(event) => setAllowedProvider(event.target.value)}
            options={PROVIDER_OPTIONS.map((option) => ({ value: option.value, label: option.label }))}
          />
        </div>
      </div>

      <Button
        type="button"
        loading={saving}
        onClick={() =>
          void onSave({
            automationEnabled,
            maxAgentNotionalUsdc: Number(maxAgentNotionalUsdc) || 0,
            maxTriggersPerDay: Math.max(0, Math.round(Number(maxTriggersPerDay) || 0)),
            minTriggerIntervalSeconds: Math.max(0, Math.round(Number(minTriggerIntervalSeconds) || 0)),
            allowedProvider:
              allowedProvider === 'all' ? undefined : (allowedProvider as 'limitless' | 'polymarket'),
            requireConfidenceBps: parsePercent(requireConfidenceBps, policy.requireConfidenceBps),
            active,
          })
        }
      >
        Save automation policy
      </Button>
    </Card>
  );
}

export default function DecisionCellPageClient({ cellId }: { cellId: string }) {
  const { addToast } = useToast();
  const { hasSession, sessionRestored } = useSessionState();
  const { data: cell, isLoading, error } = useDecisionCell(cellId, hasSession && sessionRestored);
  const recalculate = useRecalculateDecisionCell(cellId);
  const updateAutomation = useUpdateDecisionAutomation(cellId);
  const upsertAlert = useUpsertDecisionAlert(cellId);
  const addNode = useAddDecisionNode(cellId);

  const { data: marketsData } = useMarkets(
    { limit: 200, source: 'all', includeLowLiquidity: true, sort: 'newest' },
    { enabled: hasSession && sessionRestored },
  );
  const { data: externalAgentsData } = useExternalAgents({
    limit: 200,
    enabled: hasSession && sessionRestored,
  });

  const [newNodeLabel, setNewNodeLabel] = useState('');
  const [newNodeDescription, setNewNodeDescription] = useState('');
  const [newNodeWeight, setNewNodeWeight] = useState('20');
  const [newNodeSourceType, setNewNodeSourceType] = useState<DecisionNodeSourceType>('draft_market');

  const [recommendationAlertActive, setRecommendationAlertActive] = useState(true);
  const [confidenceAlertActive, setConfidenceAlertActive] = useState(false);
  const [confidenceAlertThreshold, setConfidenceAlertThreshold] = useState('55');
  const [leadAlertActive, setLeadAlertActive] = useState(false);
  const [leadAlertThreshold, setLeadAlertThreshold] = useState('7.5');
  const [nodeAlertActive, setNodeAlertActive] = useState(false);
  const [nodeAlertThreshold, setNodeAlertThreshold] = useState('65');
  const [nodeAlertNodeId, setNodeAlertNodeId] = useState('');
  const [nodeAlertDirection, setNodeAlertDirection] = useState<'above' | 'below'>('above');

  useEffect(() => {
    if (!cell) {
      return;
    }

    const recommendationAlert = cell.alerts.find((alert) => alert.kind === 'recommendation_changed');
    const confidenceAlert = cell.alerts.find((alert) => alert.kind === 'confidence_below');
    const leadAlert = cell.alerts.find((alert) => alert.kind === 'action_lead_above');
    const probabilityAlert = cell.alerts.find((alert) => alert.kind === 'node_probability_cross');

    setRecommendationAlertActive(recommendationAlert?.active ?? true);
    setConfidenceAlertActive(confidenceAlert?.active ?? false);
    setConfidenceAlertThreshold(
      String(((Number(confidenceAlert?.threshold?.bps) || 5500) / 100).toFixed(1)),
    );
    setLeadAlertActive(leadAlert?.active ?? false);
    setLeadAlertThreshold(String(((Number(leadAlert?.threshold?.bps) || 750) / 100).toFixed(1)));
    setNodeAlertActive(probabilityAlert?.active ?? false);
    setNodeAlertThreshold(
      String(((Number(probabilityAlert?.threshold?.bps) || 6500) / 100).toFixed(1)),
    );
    setNodeAlertNodeId(String(probabilityAlert?.threshold?.nodeId || cell.nodes[0]?.id || ''));
    setNodeAlertDirection(
      String(probabilityAlert?.threshold?.direction || 'above') === 'below' ? 'below' : 'above',
    );
  }, [cell]);

  const internalMarketOptions = useMemo(
    () =>
      (marketsData?.data || [])
        .filter((market) => !market.isExternal)
        .map((market) => ({ value: market.id, label: `${market.question} • ${market.category}` })),
    [marketsData],
  );

  const externalMarketOptions = useMemo(
    () =>
      (marketsData?.data || [])
        .filter((market) => market.isExternal)
        .map((market) => ({ value: market.id, label: `${market.question} • ${market.provider}` })),
    [marketsData],
  );

  const externalAgents = externalAgentsData?.data || [];
  const attachedAutomations = useMemo(
    () =>
      (cell?.nodes || []).flatMap((node) =>
        node.agents.map((agent) => ({ nodeId: node.id, nodeLabel: node.label, ...agent })),
      ),
    [cell],
  );

  const handleRecalculate = async () => {
    try {
      await recalculate.mutateAsync();
      addToast('Decision cell recalculated.', 'success');
    } catch (error) {
      addToast(error instanceof Error ? error.message : 'Failed to recalculate cell.', 'error');
    }
  };

  const handleSaveAutomation = async (payload: {
    automationEnabled: boolean;
    maxAgentNotionalUsdc: number;
    maxTriggersPerDay: number;
    minTriggerIntervalSeconds: number;
    allowedProvider?: 'limitless' | 'polymarket';
    requireConfidenceBps: number;
    active: boolean;
  }) => {
    try {
      await updateAutomation.mutateAsync(payload);
      addToast('Automation policy saved.', 'success');
    } catch (error) {
      addToast(error instanceof Error ? error.message : 'Failed to save automation policy.', 'error');
    }
  };

  const handleAddNode = async () => {
    if (!cell || !newNodeLabel.trim()) {
      addToast('Node label is required.', 'error');
      return;
    }

    try {
      await addNode.mutateAsync({
        label: newNodeLabel.trim(),
        description: newNodeDescription.trim(),
        weightBps: parsePercent(newNodeWeight, 2000),
        sourceType: newNodeSourceType,
        status: newNodeSourceType === 'draft_market' ? 'draft' : 'live',
        actionEffects: defaultNodeEffects(cell),
      });
      setNewNodeLabel('');
      setNewNodeDescription('');
      setNewNodeWeight('20');
      setNewNodeSourceType('draft_market');
      addToast('Node added.', 'success');
    } catch (error) {
      addToast(error instanceof Error ? error.message : 'Failed to add node.', 'error');
    }
  };

  const saveAlert = async (kind: string, threshold?: Record<string, unknown>, active = true) => {
    try {
      await upsertAlert.mutateAsync({ kind, threshold, active });
      addToast('Alert updated.', 'success');
    } catch (error) {
      addToast(error instanceof Error ? error.message : 'Failed to update alert.', 'error');
    }
