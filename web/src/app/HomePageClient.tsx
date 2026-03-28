"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { Header, BottomNav } from "@/components/layout";
import { HeroTicket, type HeroTicketRow } from "@/components/home/HeroTicket";
import { FeaturedSlider } from "@/components/market";
import { useAgents, useMarkets } from "@/hooks";
import { cn } from "@/lib/utils";
import type { HomeLiveFeed } from "@/lib/server/homeLive";
import type { Agent, Market, PaginatedResponse } from "@/types";

interface HomePageClientProps {
  initialMarkets?: PaginatedResponse<Market> | null;
  initialLiveFeed: HomeLiveFeed;
}

const HOME_MARKET_LIMIT = 16;
const FEATURED_MARKET_COUNT = 16;

function formatAgentSize(size: string): string {
  const parsed = Number(size) / 1_000_000;
  if (!Number.isFinite(parsed) || parsed <= 0) return "0";
  if (parsed >= 1000) return `${(parsed / 1000).toFixed(1)}k`;
  if (parsed >= 10) return parsed.toFixed(0);
  return parsed.toFixed(2);
}

function formatRelativeTimestamp(value: string): string {
  if (!value) return "schedule n/a";

  const timestamp = new Date(value).getTime();
  if (!Number.isFinite(timestamp)) return "schedule n/a";

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

function formatUtcTimestamp(value: string): string {
  const timestamp = new Date(value);
  if (Number.isNaN(timestamp.getTime())) {
    return "unknown";
  }

  return new Intl.DateTimeFormat("en-US", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
    timeZone: "UTC",
  }).format(timestamp) + " UTC";
}

function buildHeroRows(
  agents: Agent[],
  signal: HomeLiveFeed["signal"],
  isLoadingAgents: boolean,
  agentsError: Error | null
): HeroTicketRow[] {
  if (agents.length > 0) {
    return agents.slice(0, 3).map((agent, index) => ({
      label: `AGENT ${String(index + 1).padStart(2, "0")}`,
      value: `#${agent.id} ${agent.isYes ? "YES" : "NO"} ${agent.canExecute ? "READY" : agent.status.toUpperCase()}`,
    }));
  }

  return [
    {
      label: "AGENTS",
      value: agentsError ? "FEED UNAVAILABLE" : isLoadingAgents ? "LOADING" : "NONE LIVE",
    },
    {
      label: "MARKETS",
      value: `${signal.marketsTracked} TRACKED`,
    },
    {
      label: "FEEDS",
      value: `${signal.feedsLive}/${signal.feedsExpected} LIVE`,
    },
  ];
}

function AgentPanel({
  agents,
  isLoading,
  error,
  marketLookup,
  signal,
}: {
  agents: Agent[];
  isLoading: boolean;
  error: Error | null;
  marketLookup: Map<string, Market>;
  signal: HomeLiveFeed["signal"];
}) {
  const liveAgents = agents.slice(0, 6);
  const headerState = liveAgents.length > 0 ? `${liveAgents.length} live` : error ? "degraded" : isLoading ? "loading" : "standby";

  return (
    <aside className="hidden lg:flex w-[375px] shrink-0 flex-col border-r border-border">
      <div className="flex items-center justify-between px-4 py-3 border-b border-border font-mono text-[0.75rem]">
        <span className="text-text-muted uppercase tracking-wider">
          {liveAgents.length > 0 ? "Agent Runtime" : "Runtime Status"}
        </span>
        <span className="text-text-primary uppercase">{headerState}</span>
      </div>
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {liveAgents.length > 0 ? (
          liveAgents.map((agent) => {
            const market = marketLookup.get(agent.marketId);
            const scheduleLabel = agent.lastExecutedAt
              ? formatRelativeTimestamp(agent.lastExecutedAt)
              : formatRelativeTimestamp(agent.nextExecutionAt);

            return (
              <Link
                key={agent.id}
                href={`/markets/${encodeURIComponent(agent.marketId)}`}
                className="block border border-border bg-bg-secondary/60 p-3 transition-colors hover:border-border-hover hover:bg-bg-hover"
              >
                <div className="flex items-center justify-between gap-3 font-mono text-[0.68rem] uppercase tracking-[0.12em]">
                  <span className="text-text-primary">Agent #{agent.id}</span>
                  <span
                    className={cn(
                      agent.canExecute
                        ? "text-bid"
                        : agent.status === "cooldown"
                          ? "text-text-secondary"
                          : "text-text-muted"
                    )}
                  >
                    {agent.canExecute ? "ready" : agent.status}
                  </span>
                </div>
                <div className="mt-2 text-sm text-text-primary line-clamp-2">
                  {market?.question || `Market #${agent.marketId}`}
                </div>
                <div className="mt-2 text-[0.72rem] text-text-muted">
                  {agent.isYes ? "YES" : "NO"} @ {Math.round(agent.priceBps / 100)}% · {formatAgentSize(agent.size)} USDC
                </div>
                <div className="text-[0.72rem] text-text-muted">
                  cadence {agent.cadence}s · {scheduleLabel}
                </div>
              </Link>
            );
          })
        ) : (
          <div className="border border-border bg-bg-secondary/60 p-4">
            <div className="font-mono text-[0.72rem] uppercase tracking-[0.14em] text-text-primary">
              {error ? "Agent feed unavailable" : isLoading ? "Loading live agents" : "No live agents"}
            </div>
            <p className="mt-3 text-sm text-text-secondary">
              {error
                ? "The homepage could not reach the live agent runtime feed in this environment."
                : isLoading
                  ? "Waiting for the current live agent set."
                  : "No active onchain agents are currently returned by the runtime."}
            </p>
            <div className="mt-4 space-y-2 font-mono text-[0.72rem] text-text-muted">
              <div>markets tracked: {signal.marketsTracked}</div>
              <div>feeds live: {signal.feedsLive}/{signal.feedsExpected}</div>
              <div>updated: {formatUtcTimestamp(signal.updatedAt)}</div>
            </div>
            <div className="mt-4 flex gap-2">
              <Link
                href="/agents"
                className="inline-flex h-10 items-center border border-accent px-4 text-sm font-medium uppercase tracking-[0.12em] text-accent transition-colors hover:bg-accent/10"
              >
                Open agents
              </Link>
              <Link
                href="/markets"
                className="inline-flex h-10 items-center border border-border px-4 text-sm font-medium uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-hover hover:text-text-primary"
              >
                View markets
              </Link>
            </div>
          </div>
        )}
      </div>
    </aside>
  );
}


function MarketTable({ markets, isLoading }: { markets: Market[]; isLoading: boolean }) {
  if (isLoading) {
    return (
      <div className="p-6 sm:p-8 space-y-0">
        {Array.from({ length: 6 }).map((_, i) => (
          <div
            key={i}
            className="flex items-center gap-6 py-5 border-b border-border animate-pulse"
          >
            <div className="w-12 h-4 bg-bg-secondary hidden sm:block" />
            <div className="flex-1 h-5 bg-bg-secondary" />
            <div className="w-16 h-4 bg-bg-secondary hidden md:block" />
            <div className="w-20 h-4 bg-bg-secondary hidden md:block" />
            <div className="w-16 h-8 bg-bg-secondary" />
          </div>
        ))}
      </div>
    );
  }

  if (markets.length === 0) {
    return (
      <div className="py-16 text-center text-text-muted text-sm font-mono uppercase tracking-wider">
        No active markets
      </div>
    );
  }

  return (
    <div className="p-4 sm:p-6 md:p-8">
      <table className="w-full font-mono border-collapse">
        <thead>
          <tr>
            <th className="text-left py-4 px-4 border-b border-border text-[0.7rem] text-text-muted uppercase">Active Markets</th>
            <th className="text-left py-4 px-4 border-b border-border text-[0.7rem] text-text-muted uppercase hidden md:table-cell">Volume</th>
            <th className="text-left py-4 px-4 border-b border-border text-[0.7rem] text-text-muted uppercase hidden md:table-cell">Ends</th>
            <th className="text-left py-4 px-4 border-b border-border text-[0.7rem] text-text-muted uppercase">Pricing</th>
          </tr>
        </thead>
        <tbody>
          {markets.map((market) => {
            const yesPrice = market.yesPrice != null ? `${Math.round(market.yesPrice * 100)}\u00A2` : '—';
            const noPrice = market.noPrice != null ? `${Math.round(market.noPrice * 100)}\u00A2` : '—';
            const endDate = market.tradingEnd
              ? new Date(market.tradingEnd).toISOString().slice(0, 10).replace(/-/g, '.')
              : '—';
            const volume = market.totalVolume || market.volume24h;

            return (
              <tr
                key={market.id}
                className="border-b border-border transition-colors hover:bg-bg-hover group"
              >
                <td className="py-5 px-4" style={{ fontFamily: 'var(--font-display)', fontWeight: 700, fontSize: '1.1rem' }}>
                  <Link href={`/markets/${market.id}`} className="flex items-center gap-2">
                    <span className="inline-block w-1.5 h-1.5 rounded-full bg-green-500 shrink-0" />
                    <span className="group-hover:underline">{market.question}</span>
                  </Link>
                </td>
                <td className="py-5 px-4 text-[0.85rem] hidden md:table-cell">
                  {volume > 0
                    ? `$${volume / 1_000_000 >= 1 ? `${(volume / 1_000_000).toFixed(1)}M` : `${(volume / 1_000).toFixed(0)}K`}`
                    : '—'}
                </td>
                <td className="py-5 px-4 text-[0.85rem] hidden md:table-cell">{endDate}</td>
                <td className="py-5 px-4">
                  <div className="flex gap-2">
                    <Link
                      href={`/markets/${market.id}`}
                      className="flex h-10 w-24 items-center justify-between gap-2 border border-border px-3 text-sm font-semibold transition-colors hover:bg-text-primary hover:text-text-inverse"
                    >
                      <span>YES</span>
                      <span>{yesPrice}</span>
                    </Link>
                    <Link
                      href={`/markets/${market.id}`}
                      className="flex h-10 w-24 items-center justify-between gap-2 border border-border px-3 text-sm font-semibold transition-colors hover:bg-text-primary hover:text-text-inverse"
                    >
                      <span>NO</span>
                      <span>{noPrice}</span>
                    </Link>
                  </div>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

export default function HomePageClient({
  initialMarkets,
  initialLiveFeed,
}: HomePageClientProps) {
  const [liveFeed, setLiveFeed] = useState(initialLiveFeed);

  const { data: marketsData, isLoading } = useMarkets(
    {
      sort: "volume",
      limit: HOME_MARKET_LIMIT,
    },
    {
      initialData: initialMarkets || undefined,
    },
  );

  const markets = marketsData?.data || [];
  const {
    data: agentsData,
    isLoading: isLoadingAgents,
    error: agentsQueryError,
  } = useAgents({ limit: 6, active: true });
  const liveAgents = agentsData?.data ?? [];
  const agentsError = agentsQueryError instanceof Error ? agentsQueryError : null;
  const marketLookup = useMemo(
    () => new Map(markets.map((market) => [market.id, market])),
    [markets]
  );
  const heroRows = buildHeroRows(liveAgents, liveFeed.signal, isLoadingAgents, agentsError);
  const heroStatus = liveAgents.length > 0
    ? "LIVE"
    : agentsError
      ? "DEGRADED"
      : liveFeed.signal.feedsLive > 0
        ? "LIVE"
        : "STANDBY";
  const heroMode = liveAgents.length > 0
    ? `${liveAgents.length} LIVE AGENTS`
    : agentsError
      ? "AGENT FEED OFFLINE"
      : isLoadingAgents
        ? "LOADING AGENTS"
        : "MARKET MONITOR";

  useEffect(() => {
    setLiveFeed(initialLiveFeed);
  }, [initialLiveFeed]);

  useEffect(() => {
    const refresh = async () => {
      try {
        const response = await fetch("/api/home/live", { cache: "no-store" });
        if (!response.ok) return;
        const payload = (await response.json()) as HomeLiveFeed;
        setLiveFeed(payload);
      } catch {}
    };

    const interval = window.setInterval(() => {
      void refresh();
    }, 5 * 60_000);

    return () => {
      window.clearInterval(interval);
    };
  }, []);

  return (
    <div className="h-screen flex flex-col overflow-hidden">
      <Header />

      <div className="pt-page flex flex-1 overflow-hidden">
        <AgentPanel
          agents={liveAgents}
          isLoading={isLoadingAgents}
          error={agentsError}
          marketLookup={marketLookup}
          signal={liveFeed.signal}
        />

        <main className="flex-1 overflow-y-auto">
          <section className="border-b border-border h-[280px] sm:h-[320px]">
            <HeroTicket
              accessValue="PUBLIC WEB"
              statusValue={heroStatus}
              networkValue="BASE L2"
              modeValue={heroMode}
              detailRows={heroRows}
            />
          </section>

          <section className="py-5 border-b border-border">
            <FeaturedSlider markets={markets.slice(0, FEATURED_MARKET_COUNT)} title="Signal Relay" />
          </section>

          <MarketTable markets={markets} isLoading={isLoading} />
        </main>
      </div>

      <BottomNav />
    </div>
  );
}
