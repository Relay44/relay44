'use client';

import Link from 'next/link';
import { useEffect, useMemo, useState } from 'react';
import { CircleHelp } from 'lucide-react';
import { PageShell } from '@/components/layout';
import { ReadOnlyNotice } from '@/components/runtime/ReadOnlyNotice';
import { Button, Card, Input, Select, useToast } from '@/components/ui';
import {
  useAgents,
  useCreateAgent,
  useCreateExternalAgent,
  useExecuteAgent,
  useExecuteExternalAgent,
  useExternalAgents,
  usePublicExternalAgents,
  usePublicExternalAgentsPerformance,
  useMarkets,
  useRuntimeMode,
  useSessionState,
} from '@/hooks';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { api, type ExternalCredential, type ExternalCredentialStatus } from '@/lib/api';
import { formatPublicPaperAgentName } from '@/lib/publicPaperAgents';
import { cn } from '@/lib/utils';

function truncateAddress(address: string) {
  if (!address) return '';
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

function statusLabel(status: string) {
  if (status === 'ready') return 'Ready';
  if (status === 'cooldown') return 'Cooldown';
  return 'Inactive';
}

function formatCompactUsd(value: number) {
  if (!Number.isFinite(value) || value === 0) return '$0';
  const abs = Math.abs(value);
  const sign = value < 0 ? '-' : '';
  if (abs >= 1_000_000) return `${sign}$${(abs / 1_000_000).toFixed(1)}M`;
  if (abs >= 1_000) return `${sign}$${(abs / 1_000).toFixed(1)}k`;
  return `${sign}$${abs.toFixed(abs >= 100 ? 0 : 2)}`;
}

function formatPublicAgentSchedule(lastExecutedAt?: string | null, nextExecutionAt?: string) {
  const value = lastExecutedAt || nextExecutionAt || '';
  if (!value) return 'schedule n/a';

  const timestamp = new Date(value).getTime();
  if (!Number.isFinite(timestamp)) return 'schedule n/a';

  const diffMs = timestamp - Date.now();
  const absMinutes = Math.max(1, Math.round(Math.abs(diffMs) / 60_000));

  if (absMinutes < 60) {
    return diffMs >= 0 ? `next in ${absMinutes}m` : `last ${absMinutes}m ago`;
  }

  const absHours = Math.round(absMinutes / 60);
  if (absHours < 48) {
    return diffMs >= 0 ? `next in ${absHours}h` : `last ${absHours}h ago`;
  }

  const absDays = Math.round(absHours / 24);
  return diffMs >= 0 ? `next in ${absDays}d` : `last ${absDays}d ago`;
}

function FieldLabel({ label, hint }: { label: string; hint?: string }) {
  return (
    <div className="mb-1 flex items-center gap-1.5">
      <label className="block text-sm font-medium text-text-primary">{label}</label>
      {hint ? (
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              className="inline-flex h-4 w-4 items-center justify-center text-text-muted transition-colors hover:text-text-primary"
              aria-label={`${label} help`}
            >
              <CircleHelp className="h-3.5 w-3.5" />
            </button>
          </TooltipTrigger>
          <TooltipContent className="max-w-xs border border-border bg-bg-base px-3 py-2 text-xs text-text-primary brutal-shadow">
            {hint}
          </TooltipContent>
        </Tooltip>
      ) : null}
    </div>
  );
}

export default function AgentsPage() {
  const { addToast } = useToast();
  const wallet = useBaseWallet();
  const { readOnly } = useRuntimeMode();
  const { hasSession, sessionRestored } = useSessionState();
  const canManageExternal = sessionRestored && hasSession;
  const [mode, setMode] = useState<'onchain' | 'external'>('onchain');

  const createAgent = useCreateAgent();
  const executeAgent = useExecuteAgent();
  const createExternalAgent = useCreateExternalAgent();
  const executeExternalAgent = useExecuteExternalAgent();

  const web4ApiBase = (process.env.NEXT_PUBLIC_API_URL || '/v1').replace(/\/$/, '');
  const { data: marketsData } = useMarkets({ limit: 100, sort: 'newest', source: 'internal' });
  const { data: externalMarketsData } = useMarkets({
    limit: 200,
    sort: 'newest',
    source: 'all',
    tradable: 'agent',
  });

  const [filterMarketId, setFilterMarketId] = useState('');
  const [filterActiveOnly, setFilterActiveOnly] = useState(true);
  const [filterExternalProvider, setFilterExternalProvider] = useState<'limitless' | 'polymarket' | 'aerodrome' | ''>('');

  const [marketId, setMarketId] = useState('');
  const [isYes, setIsYes] = useState(true);
  const [priceBps, setPriceBps] = useState('5500');
  const [size, setSize] = useState('0.10');
  const [cadence, setCadence] = useState('300');
  const [expiryWindow, setExpiryWindow] = useState('1800');
  const [strategy, setStrategy] = useState('web4-research-signal-v1');
  const [externalName, setExternalName] = useState('external-agent');
  const [externalProvider, setExternalProvider] = useState<'limitless' | 'polymarket' | 'aerodrome'>('limitless');
  const [externalMarketId, setExternalMarketId] = useState('');
  const [externalOutcome, setExternalOutcome] = useState<'yes' | 'no'>('yes');
  const [externalSide, setExternalSide] = useState<'buy' | 'sell'>('buy');
  const [externalPrice, setExternalPrice] = useState('0.55');
  const [externalQuantity, setExternalQuantity] = useState('10');
  const [externalCadence, setExternalCadence] = useState('300');
  const [externalStrategy, setExternalStrategy] = useState('cross-venue-momentum-v1');
  const [externalCredentialId, setExternalCredentialId] = useState('');
  const [externalCredentials, setExternalCredentials] = useState<ExternalCredential[]>([]);
  const [externalCredentialStatus, setExternalCredentialStatus] =
    useState<ExternalCredentialStatus | null>(null);

  const marketOptions = useMemo(() => (marketsData?.data ?? []).filter((entry) => !entry.isExternal), [marketsData?.data]);
  const externalMarketOptions = useMemo(
    () => (externalMarketsData?.data ?? []).filter((entry) => entry.isExternal),
    [externalMarketsData?.data]
  );
  const marketSelectOptions = useMemo(
    () =>
      marketOptions.map((market) => ({
        value: market.id,
        label: `#${market.id} ${market.question}`,
      })),
    [marketOptions]
  );
  const marketFilterOptions = useMemo(
    () =>
      marketOptions.map((market) => ({
        value: market.id,
        label: `#${market.id}`,
      })),
    [marketOptions]
  );
  const providerOptions = useMemo(
    () => [
      { value: 'limitless', label: 'limitless' },
      { value: 'polymarket', label: 'polymarket' },
      { value: 'aerodrome', label: 'aerodrome' },
    ],
    []
  );
  const filteredExternalMarketSelectOptions = useMemo(
    () =>
      externalMarketOptions
        .filter((entry) => entry.provider === externalProvider)
        .map((market) => ({
          value: market.id,
          label: `${market.id} ${market.question}`,
        })),
    [externalMarketOptions, externalProvider]
  );
  const externalOutcomeOptions = useMemo(
    () => [
      { value: 'yes', label: 'yes' },
      { value: 'no', label: 'no' },
    ],
    []
  );
  const externalSideOptions = useMemo(
    () => [
      { value: 'buy', label: 'buy' },
      { value: 'sell', label: 'sell' },
    ],
    []
  );
  const externalProviderFilterOptions = useMemo(
    () => [
      { value: 'limitless', label: 'limitless' },
      { value: 'polymarket', label: 'polymarket' },
      { value: 'aerodrome', label: 'aerodrome' },
    ],
    []
  );
  const externalCredentialOptions = useMemo(
    () =>
      externalCredentials.map((entry) => ({
        value: entry.id,
        label: entry.label,
      })),
    [externalCredentials]
  );

  const { data: agentsData, isLoading } = useAgents({
    limit: 50,
    marketId: filterMarketId || undefined,
    active: filterActiveOnly ? true : undefined,
  });
  const { data: externalAgentsData, isLoading: isLoadingExternal } = useExternalAgents({
    limit: 50,
    provider: filterExternalProvider || undefined,
    active: filterActiveOnly ? true : undefined,
    enabled: canManageExternal,
  });
  const { data: publicAgentsData, isLoading: isLoadingPublicAgents } = usePublicExternalAgents({
    limit: 12,
    active: true,
  });
  const { data: publicPerformance, isLoading: isLoadingPublicPerformance } =
    usePublicExternalAgentsPerformance();

  const agents = agentsData?.data ?? [];
  const externalAgents = externalAgentsData?.data ?? [];
  const publicAgents = publicAgentsData?.data ?? [];
  const selectedMarket = marketOptions.find((entry) => entry.id === marketId);
  const selectedExternalMarket = externalMarketOptions.find((entry) => entry.id === externalMarketId);

  useEffect(() => {
    if (readOnly || !canManageExternal) {
      setExternalCredentials([]);
      setExternalCredentialId('');
      return;
    }

    let canceled = false;

    async function loadCredentials() {
      try {
        const creds = await api.getExternalCredentials(externalProvider);
        if (canceled) return;
        setExternalCredentials(creds);
        if (creds.length > 0) {
          setExternalCredentialId((current) => current || creds[0].id);
        }
      } catch {
        if (!canceled) setExternalCredentials([]);
      }
    }

    void loadCredentials();
    return () => {
      canceled = true;
    };
  }, [canManageExternal, externalProvider, readOnly]);

  useEffect(() => {
    if (readOnly || !canManageExternal || externalCredentials.length === 0) {
      setExternalCredentialStatus(null);
      return;
    }

    let canceled = false;

    async function loadStatus() {
      try {
        const status = await api.getExternalCredentialStatus(
          externalProvider,
          externalCredentialId || undefined,
        );
        if (!canceled) {
          setExternalCredentialStatus(status);
        }
      } catch {
        if (!canceled) {
          setExternalCredentialStatus(null);
        }
      }
    }

    void loadStatus();
    return () => {
      canceled = true;
    };
  }, [canManageExternal, externalCredentialId, externalCredentials.length, externalProvider, readOnly]);

  const onCreateAgent = async (event: React.FormEvent) => {
    event.preventDefault();

    if (readOnly) {
      addToast('Agent launch is unavailable in this environment', 'error');
      return;
    }
    if (!wallet.isConnected) {
      addToast('Connect wallet before launching an agent', 'error');
      return;
    }
    if (!marketId) {
      addToast('Select a market', 'error');
      return;
    }

    try {
      await createAgent.mutateAsync({
        marketId,
        isYes,
        priceBps: Number(priceBps),
        size: Number(size),
        cadence: Number(cadence),
        expiryWindow: Number(expiryWindow),
        strategy,
      });
      addToast('Agent launched onchain', 'success');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Agent launch failed';
      addToast(message, 'error');
    }
  };

  const onCreateExternalAgent = async (event: React.FormEvent) => {
    event.preventDefault();

    if (readOnly) {
      addToast('External agent launch is unavailable in this environment', 'error');
      return;
    }
    if (!wallet.isConnected) {
      addToast('Connect wallet before launching an external agent', 'error');
      return;
    }
    if (!externalMarketId) {
      addToast('Select an external market', 'error');
      return;
    }
    if (!canManageExternal) {
      addToast('Authenticate before launching an external agent', 'error');
      return;
    }
    if (!externalCredentialId) {
      addToast('Select a credential', 'error');
      return;
    }
    if (externalCredentialStatus && !externalCredentialStatus.ready) {
      addToast('Selected credential is not ready for live execution', 'error');
      return;
    }

    try {
      const price = Number(externalPrice);
      const quantity = Number(externalQuantity);
      const cadence = Number(externalCadence);

      if (!Number.isFinite(price) || price < 0.01 || price > 0.99) {
        addToast('Price must be between 0.01 and 0.99', 'error');
        return;
      }
      if (!Number.isFinite(quantity) || quantity <= 0) {
        addToast('Quantity must be greater than 0', 'error');
        return;
      }
      if (!Number.isFinite(cadence) || cadence < 1) {
        addToast('Cadence must be at least 1 second', 'error');
        return;
      }

      await createExternalAgent.mutateAsync({
        name: externalName.trim() || 'external-agent',
        provider: externalProvider,
        marketId: externalMarketId,
        outcome: externalOutcome,
        side: externalSide,
        price,
        quantity,
        cadenceSeconds: cadence,
        strategy: externalStrategy.trim() || 'external',
        credentialId: externalCredentialId,
        active: true,
      });
      addToast('External agent created', 'success');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'External agent launch failed';
      addToast(message, 'error');
    }
  };

  const onExecuteAgent = async (agentId: string) => {
    if (readOnly) {
      addToast('Agent execution is unavailable in this environment', 'error');
      return;
    }
    if (!wallet.isConnected) {
      addToast('Connect wallet before executing an agent', 'error');
      return;
    }

    try {
      await executeAgent.mutateAsync(agentId);
      addToast(`Agent ${agentId} executed`, 'success');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Execution failed';
      addToast(message, 'error');
    }
  };

  const onExecuteExternalAgent = async (agentId: string) => {
    if (readOnly) {
      addToast('External agent execution is unavailable in this environment', 'error');
      return;
    }
    if (!canManageExternal) {
      addToast('Authenticate before executing an external agent', 'error');
      return;
    }
    try {
      await executeExternalAgent.mutateAsync({ agentId, force: true });
      addToast(`External agent ${agentId} executed`, 'success');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'External execution failed';
      addToast(message, 'error');
    }
  };

  return (
    <TooltipProvider delayDuration={200}>
      <PageShell>
        <section className="mb-6">
          <h1 className="text-2xl font-semibold text-text-primary">Agents</h1>
          <p className="text-sm text-text-secondary mt-2 max-w-3xl">
            Launch, monitor, and manage market agents across onchain and external venues.
          </p>
          <div className="mt-4 flex flex-wrap gap-2">
            <Link
              href="/settings/credentials"
              className="inline-flex h-10 items-center border border-border px-4 text-sm font-medium uppercase tracking-[0.12em] text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
            >
              Manage venue credentials
            </Link>
          </div>
          {readOnly ? (
            <div className="mt-4">
              <ReadOnlyNotice
                title="Agent control is currently unavailable"
                body="You can inspect agent status and market coverage here, but launch and execute actions are unavailable in this environment."
              />
            </div>
          ) : null}
        </section>

        <section className="mb-6">
          <div className="inline-flex w-full overflow-hidden border border-border sm:w-auto">
            <button
              type="button"
              onClick={() => setMode('onchain')}
              className={cn(
                'h-9 flex-1 border-r border-border px-4 text-sm sm:flex-none',
                mode === 'onchain' ? 'text-accent bg-accent/10' : 'text-text-secondary'
              )}
            >
              Onchain Agents
            </button>
            <button
              type="button"
              onClick={() => setMode('external')}
              className={cn(
                'h-9 flex-1 px-4 text-sm sm:flex-none',
                mode === 'external' ? 'text-accent bg-accent/10' : 'text-text-secondary'
              )}
            >
              External Agents
            </button>
          </div>
        </section>

        <section className="mb-8">
          <Card className="space-y-4">
            <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
              <h2 className="text-sm font-semibold uppercase tracking-[0.12em] text-text-primary">
                {mode === 'onchain' ? 'Onchain guide' : 'External guide'}
              </h2>
              <span className="text-xs text-text-muted">
                {mode === 'onchain'
                  ? 'Runs on Base markets.'
                  : 'Uses saved venue credentials and provider order APIs.'}
              </span>
            </div>
            {mode === 'onchain' ? (
              <div className="grid gap-3 text-sm text-text-secondary md:grid-cols-3">
                <div>
                  <p className="font-medium text-text-primary">1. Pick a market and side</p>
                  <p className="mt-1">
                    Choose the market, then decide whether the agent should quote YES or NO.
                  </p>
                </div>
                <div>
                  <p className="font-medium text-text-primary">2. Set execution parameters</p>
                  <p className="mt-1">
                    Price is in basis points, size is the order notional, cadence sets how often the
                    agent can run.
                  </p>
                </div>
                <div>
                  <p className="font-medium text-text-primary">3. Launch, then monitor status</p>
                  <p className="mt-1">
                    Ready agents can execute now. Cooldown means the cadence window has not elapsed yet.
                  </p>
                </div>
              </div>
            ) : (
              <div className="grid gap-3 text-sm text-text-secondary md:grid-cols-3">
                <div>
                  <p className="font-medium text-text-primary">1. Add provider credentials</p>
                  <p className="mt-1">
                    External agents need a stored venue credential before they can submit live orders.
                  </p>
                </div>
                <div>
                  <p className="font-medium text-text-primary">2. Define the venue trade</p>
                  <p className="mt-1">
                    Pick provider, market, outcome, and side. Buy and sell are venue-native order
                    actions.
                  </p>
                </div>
                <div>
                  <p className="font-medium text-text-primary">3. Use cadence carefully</p>
                  <p className="mt-1">
                    External runs will reuse the saved trading instruction on each eligible execution
                    window.
                  </p>
                </div>
              </div>
            )}
          </Card>
        </section>

        {mode === 'onchain' ? (
          <>
            <section className="grid lg:grid-cols-2 gap-4 sm:gap-6 mb-8">
            {readOnly ? (
              <ReadOnlyNotice
                title="Onchain agent launch is currently unavailable"
                body="Directory data remains live, but new onchain agents cannot be launched or forced from this environment."
              />
            ) : (
              <Card>
                <h2 className="text-lg font-semibold mb-4">Launch Agent</h2>

                <form onSubmit={onCreateAgent} className="space-y-3">
                  <div>
                    <FieldLabel
                      label="Market"
                      hint="The agent will place and manage orders only on this market."
                    />
                    <Select
                      value={marketId || undefined}
                      onChange={(event) => setMarketId(event.target.value)}
                      options={marketSelectOptions}
                      placeholder="Select market"
                    />
                    {selectedMarket ? (
                      <p className="text-xs text-text-muted mt-1">
                        Trading closes {new Date(selectedMarket.tradingEnd).toLocaleString()}
                      </p>
                    ) : null}
                  </div>

                  <div className="grid grid-cols-2 gap-2">
                    <button
                      type="button"
                      className={cn(
                        'h-10 border text-sm font-medium',
                        isYes ? 'border-bid text-bid bg-bid-muted' : 'border-border text-text-secondary'
                      )}
                      onClick={() => setIsYes(true)}
                    >
                      YES Agent
                    </button>
                    <button
                      type="button"
                      className={cn(
                        'h-10 border text-sm font-medium',
                        !isYes ? 'border-ask text-ask bg-ask-muted' : 'border-border text-text-secondary'
                      )}
                      onClick={() => setIsYes(false)}
                    >
                      NO Agent
                    </button>
                  </div>

                  <div className="grid sm:grid-cols-2 gap-3">
                    <Input
                      label="Price (bps)"
                      type="number"
                      value={priceBps}
                      onChange={(event) => setPriceBps(event.target.value)}
                      min="1"
                      max="9999"
                    />
                    <Input
                      label="Order Size (USDC)"
                      type="number"
                      value={size}
                      onChange={(event) => setSize(event.target.value)}
                      step="0.01"
                      min="0.01"
                    />
                    <Input
                      label="Cadence (sec)"
                      type="number"
                      value={cadence}
                      onChange={(event) => setCadence(event.target.value)}
                      min="1"
                    />
                    <Input
                      label="Expiry Window (sec)"
                      type="number"
                      value={expiryWindow}
                      onChange={(event) => setExpiryWindow(event.target.value)}
                      min="1"
                    />
                  </div>
                  <p className="text-xs text-text-muted">
                    Price uses basis points. `5500` means a 55.00% quote.
                  </p>

                  <Input
                    label="Strategy"
                    value={strategy}
                    onChange={(event) => setStrategy(event.target.value)}
                    placeholder="signal-source + risk profile"
                    hint="Internal label for the trading logic or signal profile you want this agent to represent."
                  />

                  <Button type="submit" className="w-full" loading={createAgent.isPending}>
                    Launch Onchain Agent
                  </Button>
                </form>
              </Card>
            )}

            <Card>
              <h2 className="text-lg font-semibold mb-4">Operating notes</h2>
              <ul className="space-y-3 text-sm text-text-secondary">
                <li>Agents are persisted in `AgentRuntime` and executable by the network.</li>
                <li>Run status is derived from cadence and the last execution timestamp.</li>
                <li>Use this directory to monitor market participation and execution state.</li>
              </ul>
              <div className="mt-6 pt-4 border-t border-border text-sm">
                <div className="flex flex-wrap gap-3">
                  <Link href="/docs/api" className="text-accent hover:text-accent-hover">
                    API Reference
                  </Link>
                  <a
                    href={`${web4ApiBase}/web4/mcp`}
                    className="text-accent hover:text-accent-hover"
                    target="_blank"
                    rel="noreferrer"
                  >
                    MCP Manifest
                  </a>
                  <a
                    href={`${web4ApiBase}/web4/agent-card`}
                    className="text-accent hover:text-accent-hover"
                    target="_blank"
                    rel="noreferrer"
                  >
                    Agent Card
                  </a>
                </div>
              </div>
            </Card>
          </section>

          <section>
            <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
              <h2 className="text-lg font-semibold">Agent Directory</h2>
              <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
                <Select
                  value={filterMarketId || undefined}
                  onChange={(event) => setFilterMarketId(event.target.value)}
                  options={marketFilterOptions}
                  placeholder="All markets"
                  className="w-full text-sm sm:w-[11rem]"
                />
                <button
                  type="button"
                  onClick={() => setFilterActiveOnly((prev) => !prev)}
                  className={cn(
                    'h-9 w-full border px-3 text-sm sm:w-auto',
                    filterActiveOnly
                      ? 'border-accent text-accent bg-accent/10'
                      : 'border-border text-text-secondary'
                  )}
                >
                  Active only
                </button>
              </div>
            </div>

            {isLoading ? (
              <Card>
                <div className="flex items-center gap-3 text-sm text-text-secondary">
                  <div className="h-4 w-4 animate-spin border-2 border-border border-t-accent" />
                  Loading agents...
                </div>
              </Card>
            ) : agents.length === 0 ? (
              <Card className="text-center py-12">
                <p className="text-text-secondary">
                  {filterMarketId || filterActiveOnly
                    ? 'No agents match the current filter.'
                    : 'No onchain agents launched yet.'}
                </p>
                <p className="mt-2 text-sm text-text-muted">
                  {filterMarketId || filterActiveOnly
                    ? 'Try removing filters or switching to a different market.'
                    : 'Use the launch form above to create your first onchain agent.'}
                </p>
              </Card>
            ) : (
              <div className="grid gap-3">
                {agents.map((agent) => (
                  <Card key={agent.id} className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
                    <div className="space-y-1">
                      <div className="flex items-center gap-2">
                        <span className="text-sm text-text-muted">#{agent.id}</span>
                        <span
                          className={cn(
                            'text-xs px-2 py-1 border',
                            agent.status === 'ready'
                              ? 'border-bid text-bid'
                              : agent.status === 'cooldown'
                                ? 'border-border-hover text-text-secondary'
                                : 'border-border text-text-muted'
                          )}
                        >
                          {statusLabel(agent.status)}
                        </span>
                      </div>
                      <p className="text-sm text-text-primary">
                        Market #{agent.marketId} · {agent.isYes ? 'YES' : 'NO'} · {agent.priceBps} bps
                      </p>
                      <p className="text-xs text-text-muted">
                        Owner {truncateAddress(agent.owner)} · Size {Number(agent.size) / 1_000_000} USDC · Cadence {agent.cadence}s
                      </p>
                      {agent.identityTier !== undefined || agent.reputationScoreBps !== undefined ? (
                        <p className="text-xs text-text-muted">
                          Identity {agent.identityTier ?? 'n/a'} · Reputation {agent.reputationScoreBps ?? 'n/a'} bps
                        </p>
                      ) : null}
                      <p className="text-xs text-text-muted">Strategy: {agent.strategy || 'n/a'}</p>
                    </div>

                    <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
                      <Link href={`/markets/${encodeURIComponent(agent.marketId)}`} className="flex h-9 items-center justify-center border border-border px-3 text-sm sm:w-auto">
                        Open Market
                      </Link>
                      <Button
                        type="button"
                        variant={agent.isYes ? 'bid' : 'ask'}
                        size="sm"
                        className="w-full sm:w-auto"
                        disabled={readOnly || !agent.canExecute || executeAgent.isPending}
                        loading={executeAgent.isPending}
                        onClick={() => onExecuteAgent(agent.id)}
                      >
                        Execute
                      </Button>
                    </div>
                  </Card>
                ))}
              </div>
            )}
          </section>
          </>
        ) : (
          <>
            <section className="grid lg:grid-cols-2 gap-4 sm:gap-6 mb-8">
            {readOnly ? (
              <ReadOnlyNotice
                title="External agent launch is currently unavailable"
                body="Venue market data remains visible, but credential-backed agent execution is unavailable in this environment."
              />
            ) : (
              <Card>
                <h2 className="text-lg font-semibold mb-4">Launch External Agent</h2>
                {!canManageExternal ? (
                  <div className="mb-4 border border-border p-3 text-sm text-text-secondary">
                    Authenticate your wallet session before loading venue credentials or launching
                    external agents.
                  </div>
                ) : null}
                <form onSubmit={onCreateExternalAgent} className="space-y-3">
                  <Input
                    label="Name"
                    value={externalName}
                    onChange={(event) => setExternalName(event.target.value)}
                    hint="Operator-facing label for this saved execution profile."
                  />
                  <div>
                    <FieldLabel
                      label="Provider"
                      hint="The execution venue the agent will trade on. Provider choice also determines the available market list."
                    />
                    <Select
                      value={externalProvider}
                      onChange={(event) => setExternalProvider(event.target.value as 'limitless' | 'polymarket' | 'aerodrome')}
                      options={providerOptions}
                    />
                  </div>
                  <div>
                    <FieldLabel
                      label="Market"
                      hint="Only markets from the selected provider appear here. The agent will keep reusing this venue market on each run."
                    />
                    <Select
                      value={externalMarketId || undefined}
                      onChange={(event) => setExternalMarketId(event.target.value)}
                      options={filteredExternalMarketSelectOptions}
                      placeholder="Select market"
                    />
                    {selectedExternalMarket ? (
                      <p className="text-xs text-text-muted mt-1">
                        Chain {selectedExternalMarket.chainId} · closes {new Date(selectedExternalMarket.tradingEnd).toLocaleString()}
                      </p>
                    ) : null}
                  </div>

                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <FieldLabel
                        label="Outcome"
                        hint="The binary outcome leg on the selected venue market."
                      />
                      <Select
                        value={externalOutcome}
                        onChange={(event) => setExternalOutcome(event.target.value as 'yes' | 'no')}
                        options={externalOutcomeOptions}
                      />
                    </div>
                    <div>
                      <FieldLabel
                        label="Side"
                        hint="Buy opens or adds to exposure on the chosen outcome. Sell reduces or takes the opposite venue-side action."
                      />
                      <Select
                        value={externalSide}
                        onChange={(event) => setExternalSide(event.target.value as 'buy' | 'sell')}
                        options={externalSideOptions}
                      />
                    </div>
                  </div>

                  <div className="grid sm:grid-cols-2 gap-3">
                    <Input
                      label="Price"
                      type="number"
                      value={externalPrice}
                      onChange={(event) => setExternalPrice(event.target.value)}
                      min="0.01"
                      max="0.99"
                      step="0.01"
                    />
                    <Input
                      label="Quantity"
                      type="number"
                      value={externalQuantity}
                      onChange={(event) => setExternalQuantity(event.target.value)}
                      min="0.01"
                      step="0.01"
                    />
                    <Input
                      label="Cadence (sec)"
                      type="number"
                      value={externalCadence}
                      onChange={(event) => setExternalCadence(event.target.value)}
                      min="1"
                      hint="Minimum delay between eligible automatic executions."
                    />
                    <div className="space-y-1.5 sm:col-span-2">
                      <FieldLabel
                        label="Credential"
                        hint="Stored venue credential used to sign and submit provider-native orders."
                      />
                      <Select
                        value={externalCredentialId || undefined}
                        onChange={(event) => setExternalCredentialId(event.target.value)}
                        options={externalCredentialOptions}
                        placeholder="Select credential"
                      />
                    </div>
                  </div>

                  {externalCredentialStatus ? (
                    <div className="border border-border p-3 text-xs text-text-secondary">
                      <div className="font-medium text-text-primary">
                        {externalCredentialStatus.ready
                          ? 'Credential ready'
                          : 'Credential not ready'}
                      </div>
                      {externalCredentialStatus.base_wallet ? (
                        <div className="mt-1">
                          Base wallet: {externalCredentialStatus.base_wallet}
                        </div>
                      ) : null}
                      {externalCredentialStatus.checks
                        .filter((check) => !check.ok)
                        .map((check) => (
                          <div key={check.code} className="mt-1">
                            {check.message}
                          </div>
                        ))}
                    </div>
                  ) : null}

                  {externalProvider === 'polymarket' ? (
                    <div className="border border-border p-3 text-xs text-text-secondary">
                      Polymarket agent runs use saved CLOB credentials and the same provider
                      readiness checks as direct orders. Use a browser-wallet account path if you
                      want the connected wallet to sign normally.
                    </div>
                  ) : null}

                  {externalProvider === 'aerodrome' ? (
                    <div className="border border-border p-3 text-xs text-text-secondary">
                      Aerodrome agents execute on-chain swaps via Aerodrome Slipstream on Base.
                      Requires a funded Base wallet with USDC and token approvals for the swap router.
                    </div>
                  ) : null}

                  <Input
                    label="Strategy"
                    value={externalStrategy}
                    onChange={(event) => setExternalStrategy(event.target.value)}
                    hint="Internal label for the venue execution logic or strategy family."
                  />

                  <Button
                    type="submit"
                    className="w-full"
                    loading={createExternalAgent.isPending}
                    disabled={
                      !canManageExternal ||
                      createExternalAgent.isPending ||
                      !externalCredentialId ||
                      (externalCredentialStatus ? !externalCredentialStatus.ready : false)
                    }
                  >
                    Launch External Agent
                  </Button>
                </form>
              </Card>
            )}

            <Card>
              <h2 className="text-lg font-semibold mb-4">External Execution Notes</h2>
              <ul className="space-y-3 text-sm text-text-secondary">
                <li>External agents use saved venue credentials and provider order APIs.</li>
                <li>Funding and allowance checks run before each execution request.</li>
                <li>Launch scope is currently limited to YES/NO markets.</li>
              </ul>
            </Card>
          </section>

          <section>
            <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
              <h2 className="text-lg font-semibold">External Agent Directory</h2>
              <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
                <Select
                  value={filterExternalProvider || undefined}
                  onChange={(event) =>
                    setFilterExternalProvider(event.target.value as 'limitless' | 'polymarket' | 'aerodrome' | '')
                  }
                  options={externalProviderFilterOptions}
                  placeholder="All providers"
                  className="w-full text-sm sm:w-[11rem]"
                />
                <button
                  type="button"
                  onClick={() => setFilterActiveOnly((prev) => !prev)}
                  className={cn(
                    'h-9 w-full border px-3 text-sm sm:w-auto',
                    filterActiveOnly
                      ? 'border-accent text-accent bg-accent/10'
                      : 'border-border text-text-secondary'
                  )}
                >
                  Active only
                </button>
              </div>
            </div>

            {isLoadingExternal ? (
              <Card>
                <div className="flex items-center gap-3 text-sm text-text-secondary">
                  <div className="h-4 w-4 animate-spin border-2 border-border border-t-accent" />
                  Loading external agents...
                </div>
              </Card>
            ) : externalAgents.length === 0 ? (
              <Card className="text-center py-12">
                <p className="text-text-secondary">
                  {filterExternalProvider || filterActiveOnly
                    ? 'No external agents match the current filter.'
                    : 'No external agents launched yet.'}
                </p>
                <p className="mt-2 text-sm text-text-muted">
                  {filterExternalProvider || filterActiveOnly
                    ? 'Try removing filters or switching providers.'
                    : 'Add venue credentials, then use the launch form above to create one.'}
                </p>
              </Card>
            ) : (
              <div className="grid gap-3">
                {externalAgents.map((agent) => (
                  <Card key={agent.id} className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
                    <div className="space-y-1">
                      <p className="text-sm text-text-primary">
                        {agent.name} · {agent.provider} · {agent.market_id}
                      </p>
                      <p className="text-xs text-text-muted">
                        {agent.outcome.toUpperCase()} {agent.side.toUpperCase()} · price {agent.price} · qty {agent.quantity}
                      </p>
                      <p className="text-xs text-text-muted">
                        Cadence {agent.cadence_seconds}s · Strategy {agent.strategy}
                        {agent.consecutive_failures > 0 && (
                          <span className="ml-2 text-[0.65rem] text-red-400" title={agent.last_error_code ?? undefined}>
                            {agent.consecutive_failures} failure{agent.consecutive_failures > 1 ? 's' : ''}
                          </span>
                        )}
                      </p>
                    </div>
                    <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
                      <Link href={`/markets/${encodeURIComponent(agent.market_id)}`} className="flex h-9 items-center justify-center border border-border px-3 text-sm sm:w-auto">
                        Open Market
                      </Link>
                      <Button
                        type="button"
                        size="sm"
                        className="w-full sm:w-auto"
                        disabled={readOnly || !canManageExternal || executeExternalAgent.isPending}
                        loading={executeExternalAgent.isPending}
                        onClick={() => onExecuteExternalAgent(agent.id)}
                      >
                        Execute
                      </Button>
                    </div>
                  </Card>
                ))}
              </div>
            )}
          </section>
          </>
        )}
        <section className="mt-12 border-t border-border pt-8">
          <Card className="space-y-5">
            <div className="flex flex-col gap-2 sm:flex-row sm:items-end sm:justify-between">
              <div>
                <h2 className="text-lg font-semibold text-text-primary">Relay44 Paper Lab</h2>
                <p className="mt-1 text-sm text-text-secondary">
                  Relay44-run paper agents continuously research, prove, and optimize live venue markets.
                </p>
              </div>
              <span className="font-mono text-[0.72rem] uppercase tracking-[0.14em] text-accent">
                Public cohort
              </span>
            </div>

            {isLoadingPublicPerformance ? (
              <div className="grid gap-2 sm:gap-3 md:grid-cols-5">
                {Array.from({ length: 5 }).map((_, index) => (
                  <div key={index} className="border border-border p-4 animate-pulse">
                    <div className="h-3 w-20 bg-bg-secondary" />
                    <div className="mt-3 h-6 w-16 bg-bg-secondary" />
                  </div>
                ))}
              </div>
            ) : (
              <div className="grid gap-2 sm:gap-3 md:grid-cols-5">
                <div className="border border-border p-4">
                  <div className="font-mono text-[0.68rem] uppercase tracking-[0.12em] text-text-muted">
                    Active agents
                  </div>
                  <div className="mt-2 text-2xl font-semibold text-text-primary">
                    {publicPerformance?.totals.activeAgents ?? 0}
                  </div>
                </div>
                <div className="border border-border p-4">
                  <div className="font-mono text-[0.68rem] uppercase tracking-[0.12em] text-text-muted">
                    Open positions
                  </div>
                  <div className="mt-2 text-2xl font-semibold text-text-primary">
                    {publicPerformance?.totals.openPositions ?? 0}
                  </div>
                </div>
                <div className="border border-border p-4">
                  <div className="font-mono text-[0.68rem] uppercase tracking-[0.12em] text-text-muted">
                    Fills
                  </div>
                  <div className="mt-2 text-2xl font-semibold text-text-primary">
                    {publicPerformance?.totals.fills ?? 0}
                  </div>
                </div>
                <div className="border border-border p-4">
                  <div className="font-mono text-[0.68rem] uppercase tracking-[0.12em] text-text-muted">
                    Volume
                  </div>
                  <div className="mt-2 text-2xl font-semibold text-text-primary">
                    {formatCompactUsd(publicPerformance?.totals.volumeUsdc ?? 0)}
                  </div>
                </div>
                <div className="border border-border p-4">
                  <div className="font-mono text-[0.68rem] uppercase tracking-[0.12em] text-text-muted">
                    Net PnL
                  </div>
                  <div
                    className={cn(
                      'mt-2 text-2xl font-semibold',
                      (publicPerformance?.totals.netPnlUsdc ?? 0) >= 0 ? 'text-bid' : 'text-ask'
                    )}
                  >
                    {formatCompactUsd(publicPerformance?.totals.netPnlUsdc ?? 0)}
                  </div>
                </div>
              </div>
            )}

            {isLoadingPublicAgents ? (
              <div className="grid gap-3 lg:grid-cols-2">
                {Array.from({ length: 4 }).map((_, index) => (
                  <div key={index} className="border border-border p-4 animate-pulse">
                    <div className="h-4 w-24 bg-bg-secondary" />
                    <div className="mt-3 h-5 w-3/4 bg-bg-secondary" />
                    <div className="mt-2 h-4 w-1/2 bg-bg-secondary" />
                  </div>
                ))}
              </div>
            ) : publicAgents.length === 0 ? (
              <div className="border border-border p-4 text-sm text-text-secondary">
                Relay44 paper agents are warming up. This section will populate as soon as the public cohort is seeded.
              </div>
            ) : (
              <div className="grid gap-3 lg:grid-cols-2">
                {publicAgents.map((agent) => (
                  <Card
                    key={agent.id}
                    className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between"
                  >
                    <div className="space-y-1">
                      <p className="text-sm font-medium text-text-primary">
                        {formatPublicPaperAgentName(agent)}
                      </p>
                      <div className="flex items-center gap-2">
                        <span className="border border-accent px-2 py-1 text-[0.68rem] uppercase tracking-[0.12em] text-accent">
                          {agent.strategy_label}
                        </span>
                        <span className="text-xs text-text-muted">{agent.provider}</span>
                      </div>
                      <p className="text-sm text-text-primary">{agent.market_id}</p>
                      <p className="text-xs text-text-muted">
                        {agent.outcome.toUpperCase()} {agent.side.toUpperCase()} · price {Math.round(agent.price * 100)}% · qty {agent.quantity}
                      </p>
                      <p className="text-xs text-text-muted">
                        Cadence {agent.cadence_seconds}s · {formatPublicAgentSchedule(agent.last_executed_at, agent.next_execution_at)}
                      </p>
                      <div className="grid grid-cols-2 gap-2 pt-1 text-xs sm:grid-cols-4">
                        <div className="border border-border px-2 py-1">
                          <div className="font-mono uppercase tracking-[0.12em] text-text-muted">Fills</div>
                          <div className="mt-1 text-sm text-text-primary">
                            {agent.paper_performance?.fills ?? 0}
                          </div>
                        </div>
                        <div className="border border-border px-2 py-1">
                          <div className="font-mono uppercase tracking-[0.12em] text-text-muted">Open</div>
                          <div className="mt-1 text-sm text-text-primary">
                            {agent.paper_performance?.openPositions ?? 0}
                          </div>
                        </div>
                        <div className="border border-border px-2 py-1">
                          <div className="font-mono uppercase tracking-[0.12em] text-text-muted">Net PnL</div>
                          <div
                            className={cn(
                              'mt-1 text-sm',
                              (agent.paper_performance?.netPnlUsdc ?? 0) >= 0 ? 'text-bid' : 'text-ask'
                            )}
                          >
                            {formatCompactUsd(agent.paper_performance?.netPnlUsdc ?? 0)}
                          </div>
                        </div>
                        <div className="border border-border px-2 py-1">
                          <div className="font-mono uppercase tracking-[0.12em] text-text-muted">Drawdown</div>
                          <div className="mt-1 text-sm text-text-primary">
                            {formatCompactUsd(agent.paper_performance?.maxDrawdownUsdc ?? 0)}
                          </div>
                        </div>
                      </div>
                    </div>
                    <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
                      <Link
                        href={`/markets/${encodeURIComponent(agent.market_id)}`}
                        className="flex h-9 items-center justify-center border border-border px-3 text-sm sm:w-auto"
                      >
                        Open Market
                      </Link>
                    </div>
                  </Card>
                ))}
              </div>
            )}
          </Card>
        </section>
      </PageShell>
    </TooltipProvider>
  );
}
