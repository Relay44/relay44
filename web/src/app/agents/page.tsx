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
  useMarkets,
  useRuntimeMode,
  useSessionState,
} from '@/hooks';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { api, type ExternalCredential, type ExternalCredentialStatus } from '@/lib/api';
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
  const [filterExternalProvider, setFilterExternalProvider] = useState<'limitless' | 'polymarket' | ''>('');

  const [marketId, setMarketId] = useState('');
  const [isYes, setIsYes] = useState(true);
  const [priceBps, setPriceBps] = useState('5500');
  const [size, setSize] = useState('0.10');
  const [cadence, setCadence] = useState('300');
  const [expiryWindow, setExpiryWindow] = useState('1800');
  const [strategy, setStrategy] = useState('web4-research-signal-v1');
  const [externalName, setExternalName] = useState('external-agent');
  const [externalProvider, setExternalProvider] = useState<'limitless' | 'polymarket'>('limitless');
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

  const agents = agentsData?.data ?? [];
  const externalAgents = externalAgentsData?.data ?? [];
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
    if (readOnly || !canManageExternal) {
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
  }, [canManageExternal, externalCredentialId, externalProvider, readOnly]);

  const onCreateAgent = async (event: React.FormEvent) => {
    event.preventDefault();

    if (readOnly) {
      addToast('Agent launch is disabled in read-only mode', 'error');
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
      addToast('External agent launch is disabled in read-only mode', 'error');
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
      await createExternalAgent.mutateAsync({
        name: externalName.trim() || 'external-agent',
        provider: externalProvider,
        marketId: externalMarketId,
        outcome: externalOutcome,
        side: externalSide,
        price: Number(externalPrice),
        quantity: Number(externalQuantity),
        cadenceSeconds: Number(externalCadence),
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
      addToast('Agent execution is disabled in read-only mode', 'error');
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
      addToast('External agent execution is disabled in read-only mode', 'error');
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
          <h1 className="text-2xl font-semibold text-text-primary">Web4 Agent Grid</h1>
          <p className="text-sm text-text-secondary mt-2 max-w-3xl">
            Launch autonomous market agents, monitor execution windows, and operate machine-native
            strategies on Base.
          </p>
          <div className="mt-4 flex flex-wrap gap-2">
            <Link
              href="/settings/credentials"
              className="inline-flex h-9 items-center border border-border px-4 text-xs uppercase tracking-[0.12em] text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
            >
              Manage venue credentials
            </Link>
          </div>
          {readOnly ? (
            <div className="mt-4">
              <ReadOnlyNotice
                title="Agent control is disabled"
                body="You can inspect agent directories and market coverage here, but launch and execute actions are locked in this preview."
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
                {mode === 'onchain' ? 'Onchain lane guide' : 'External lane guide'}
              </h2>
              <span className="text-xs text-text-muted">
                {mode === 'onchain'
                  ? 'Uses Base-native market execution.'
                  : 'Uses BYOK venue credentials and provider-native execution.'}
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
            <section className="grid lg:grid-cols-2 gap-6 mb-8">
            {readOnly ? (
              <ReadOnlyNotice
                title="Onchain agent launch is disabled"
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
              <h2 className="text-lg font-semibold mb-4">Web4 Operating Notes</h2>
              <ul className="space-y-3 text-sm text-text-secondary">
                <li>Agents are persisted in `AgentRuntime` and executable by the network.</li>
                <li>Execution status is calculated from cadence and last execution timestamp.</li>
                <li>Use this directory as the control plane for autonomous market participation.</li>
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

