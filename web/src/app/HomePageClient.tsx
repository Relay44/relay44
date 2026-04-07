"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { Header, BottomNav } from "@/components/layout";
import { HeroTicket, type HeroTicketRow } from "@/components/home/HeroTicket";
import { OnboardingBanner } from "@/components/home/OnboardingBanner";
import { FeaturedSlider } from "@/components/market";
import { useAgents, useMarkets, usePublicExternalAgents } from "@/hooks";
import { formatPublicPaperAgentName } from "@/lib/publicPaperAgents";
import { getMockPlatformStats } from "@/lib/mock-data";
import { cn } from "@/lib/utils";
import type { HomeLiveFeed } from "@/lib/server/homeLive";
import type { ExternalAgentRecord } from "@/lib/api";
import type { Agent, Market, PaginatedResponse } from "@/types";

interface HomePageClientProps {
  initialMarkets?: PaginatedResponse<Market> | null;
  initialLiveFeed: HomeLiveFeed;
  heroImageSrcs: string[];
  heroInitialIndex: number;
}

const HOME_MARKET_LIMIT = 16;
const HOME_LOOKUP_LIMIT = 100;
const FEATURED_MARKET_COUNT = 16;

interface LiveAgentFeedEntry {
  id: string;
  href: string;
  label: string;
  title: string;
  subtitle: string;
  meta: string;
  summary: string;
  scheduleLabel: string;
  statusLabel: string;
  sourceLabel: "onchain" | "relay44";
  sourceTone: "default" | "accent";
  ready: boolean;
  muted: boolean;
  lastActivityAt?: number;
  nextExecutionAt?: number;
}

function formatPriceForTape(value: number | null | undefined): string {
  if (typeof value !== "number" || Number.isNaN(value)) {
    return "--";
  }

  return `${Math.round(value * 100)}c`;
}

function truncateTapeQuestion(question: string): string {
  const normalized = question.trim();
  if (normalized.length <= 72) {
    return normalized;
  }

  return `${normalized.slice(0, 69).trimEnd()}...`;
}

function formatAgentSize(size: string): string {
  const parsed = Number(size) / 1_000_000;
  if (!Number.isFinite(parsed) || parsed <= 0) return "0";
  if (parsed >= 1000) return `${(parsed / 1000).toFixed(1)}k`;
  if (parsed >= 10) return parsed.toFixed(0);
  return parsed.toFixed(2);
}

function formatPaperQuantity(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0";
  if (value >= 1000) return `${(value / 1000).toFixed(1)}k`;
  if (value >= 10) return value.toFixed(0);
  return value.toFixed(2);
}

function formatPricePercent(value: number): string {
  if (!Number.isFinite(value)) return "--";
  return `${Math.round(value * 100)}%`;
}

function formatCompactUsd(value: number): string {
  if (!Number.isFinite(value) || value === 0) return "$0";

  const abs = Math.abs(value);
  const formatter = new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    notation: abs >= 1000 ? "compact" : "standard",
    maximumFractionDigits: abs >= 100 ? 0 : 2,
  });
  const formatted = formatter.format(abs);
  return value > 0 ? `+${formatted}` : `-${formatted}`;
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
  agents: LiveAgentFeedEntry[],
  signal: HomeLiveFeed["signal"],
  isLoadingAgents: boolean,
  agentsError: Error | null
): HeroTicketRow[] {
  if (agents.length > 0) {
    return agents.slice(0, 3).map((agent, index) => ({
      label: `AGENT ${String(index + 1).padStart(2, "0")}`,
      value: agent.summary,
    }));
  }

  const mockStats = getMockPlatformStats();
  return [
    {
      label: "AGENTS",
      value: agentsError ? "FEED UNAVAILABLE" : isLoadingAgents ? "LOADING" : `${mockStats.activeAgents} ACTIVE`,
    },
    {
      label: "MARKETS",
      value: `${signal.marketsTracked || mockStats.totalMarkets} TRACKED`,
    },
    {
      label: "FEEDS",
      value: `${signal.feedsLive}/${signal.feedsExpected} LIVE`,
    },
  ];
}

function compareLiveAgents(a: LiveAgentFeedEntry, b: LiveAgentFeedEntry): number {
  const aHasLast = Number.isFinite(a.lastActivityAt);
  const bHasLast = Number.isFinite(b.lastActivityAt);

  if (aHasLast && bHasLast) {
    return (b.lastActivityAt ?? 0) - (a.lastActivityAt ?? 0);
  }
  if (aHasLast) return -1;
  if (bHasLast) return 1;

  const aNext = a.nextExecutionAt ?? Number.POSITIVE_INFINITY;
  const bNext = b.nextExecutionAt ?? Number.POSITIVE_INFINITY;
  return aNext - bNext;
}

function toLiveFeedEntry(
  agent: Agent,
  marketLookup: Map<string, Market>,
): LiveAgentFeedEntry {
  const market = marketLookup.get(agent.marketId);
  const scheduleLabel = agent.lastExecutedAt
    ? formatRelativeTimestamp(agent.lastExecutedAt)
    : formatRelativeTimestamp(agent.nextExecutionAt);

  return {
    id: `onchain-${agent.id}`,
    href: `/markets/${encodeURIComponent(agent.marketId)}`,
    label: `Agent #${agent.id}`,
    title: market?.question || `Market #${agent.marketId}`,
    subtitle: `${agent.isYes ? "YES" : "NO"} @ ${Math.round(agent.priceBps / 100)}% · ${formatAgentSize(agent.size)} USDC`,
    meta: `cadence ${agent.cadence}s · ${scheduleLabel}`,
    summary: `ONCHAIN ${agent.isYes ? "YES" : "NO"} ${agent.canExecute ? "READY" : agent.status.toUpperCase()}`,
    scheduleLabel,
    statusLabel: agent.canExecute ? "ready" : agent.status,
    sourceLabel: "onchain",
    sourceTone: "default",
    ready: agent.canExecute,
    muted: !agent.canExecute && agent.status !== "cooldown",
    lastActivityAt: agent.lastExecutedAt ? new Date(agent.lastExecutedAt).getTime() : undefined,
    nextExecutionAt: agent.nextExecutionAt ? new Date(agent.nextExecutionAt).getTime() : undefined,
  };
}

function toPublicPaperFeedEntry(
  agent: ExternalAgentRecord,
  marketLookup: Map<string, Market>,
): LiveAgentFeedEntry {
  const market = marketLookup.get(agent.market_id);
  const displayName = formatPublicPaperAgentName(agent);
  const performance = agent.paper_performance;
  const scheduleLabel = agent.last_executed_at
    ? formatRelativeTimestamp(agent.last_executed_at)
    : formatRelativeTimestamp(agent.next_execution_at);
  const fills = performance?.fills ?? 0;
  const fillsLabel = `${fills} fills`;
  const netPnl = performance?.netPnlUsdc ?? 0;
  const pnlLabel = formatCompactUsd(netPnl);
  const heroSuffix = netPnl > 0 ? pnlLabel : fills > 0 ? `${fills} fills` : "ACTIVE";

  return {
    id: `relay44-${agent.id}`,
    href: `/markets/${encodeURIComponent(agent.market_id)}`,
    label: displayName,
    title: market?.question || agent.market_id,
    subtitle: `${agent.strategy_label} · ${agent.outcome.toUpperCase()} ${agent.side.toUpperCase()} @ ${formatPricePercent(agent.price)} · qty ${formatPaperQuantity(agent.quantity)}`,
    meta: `${fillsLabel} · ${pnlLabel} · ${scheduleLabel}`,
    summary: `RELAY44 ${displayName.toUpperCase()} · ${heroSuffix}`,
    scheduleLabel,
    statusLabel: agent.active ? "active" : "inactive",
    sourceLabel: "relay44",
    sourceTone: "accent",
    ready: agent.active,
    muted: !agent.active,
    lastActivityAt: agent.last_executed_at ? new Date(agent.last_executed_at).getTime() : undefined,
    nextExecutionAt: agent.next_execution_at ? new Date(agent.next_execution_at).getTime() : undefined,
  };
}

function AgentPanel({
  agents,
  isLoading,
  error,
  signal,
}: {
  agents: LiveAgentFeedEntry[];
  isLoading: boolean;
  error: Error | null;
  signal: HomeLiveFeed["signal"];
}) {
  const liveAgents = agents.slice(0, 6);
  const headerState = liveAgents.length > 0 ? `${liveAgents.length} live` : error ? "degraded" : isLoading ? "loading" : "standby";

  return (
    <aside className="hidden lg:flex w-[375px] shrink-0 flex-col border-r border-border">
      <div className="flex items-center justify-between px-4 py-3 border-b border-border font-mono text-[0.75rem]">
        <span className="text-text-muted uppercase tracking-wider">Live Agents</span>
        <span className="text-text-primary uppercase">{headerState}</span>
      </div>
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {liveAgents.length > 0 ? (
          liveAgents.map((agent) => {
            return (
              <Link
                key={agent.id}
                href={agent.href}
                className="block border border-border bg-bg-secondary/60 p-3 transition-colors hover:border-border-hover hover:bg-bg-hover"
              >
                <div className="flex items-center justify-between gap-3 font-mono text-[0.68rem] uppercase tracking-[0.12em]">
                  <span className="text-text-primary">{agent.label}</span>
                  <div className="flex items-center gap-2">
                    <span
                      className={cn(
                        "px-2 py-0.5",
                        agent.sourceTone === "accent"
                          ? "text-text-muted"
                          : "border border-border text-text-muted"
                      )}
                    >
                      {agent.sourceLabel}
                    </span>
                    <span
                      className={cn(
                        agent.sourceLabel === "relay44" && agent.statusLabel === "active"
                          ? "text-text-primary"
                          : agent.ready
                          ? "text-bid"
                          : agent.muted
                            ? "text-text-muted"
                            : "text-text-secondary"
                      )}
                    >
                      {agent.statusLabel}
                    </span>
                  </div>
                </div>
                <div className="mt-2 text-sm text-text-primary line-clamp-2">
                  {agent.title}
                </div>
                <div className="mt-2 text-[0.72rem] text-text-muted">{agent.subtitle}</div>
                <div className="text-[0.72rem] text-text-muted">{agent.meta}</div>
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
                ? "The homepage could not reach the live agent feeds in this environment."
                : isLoading
                  ? "Waiting for the current live agent set."
                  : "Relay44 paper agents are warming up and no active onchain agents are currently returned by the runtime."}
            </p>
            <div className="mt-4 space-y-2 font-mono text-[0.72rem] text-text-muted">
              <div>markets tracked: {signal.marketsTracked}</div>
              <div>feeds live: {signal.feedsLive}/{signal.feedsExpected}</div>
              <div>updated: {formatUtcTimestamp(signal.updatedAt)}</div>
            </div>
            <div className="mt-4 flex gap-2">
              <Link
                href="/agents"
                className="inline-flex h-10 items-center border border-accent px-4 text-[0.7rem] font-medium uppercase tracking-[0.12em] text-accent transition-colors hover:bg-accent/10"
              >
                Open agents
              </Link>
              <Link
                href="/markets"
                className="inline-flex h-10 items-center border border-border px-4 text-[0.7rem] font-medium uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-hover hover:text-text-primary"
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
    <div className="p-4 sm:p-6 md:p-8 overflow-x-auto">
      <table className="w-full font-mono border-collapse">
        <thead>
          <tr>
            <th className="text-left py-4 px-3 sm:px-4 border-b border-border text-[0.7rem] text-text-muted uppercase">Active Markets</th>
            <th className="text-left py-4 px-3 sm:px-4 border-b border-border text-[0.7rem] text-text-muted uppercase hidden md:table-cell">Volume</th>
            <th className="text-left py-4 px-3 sm:px-4 border-b border-border text-[0.7rem] text-text-muted uppercase hidden md:table-cell">Ends</th>
            <th className="text-left py-4 px-3 sm:px-4 border-b border-border text-[0.7rem] text-text-muted uppercase">Pricing</th>
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
                <td className="px-3 py-4 sm:px-4 sm:py-5 font-display text-sm font-bold">
                  <Link href={`/markets/${market.id}`} className="flex items-center gap-2">
                    <span className="inline-block w-1.5 h-1.5 rounded-full bg-green-500 shrink-0" />
                    <span className="group-hover:underline">{market.question}</span>
                  </Link>
                </td>
                <td className="py-4 px-3 sm:py-5 sm:px-4 text-[0.85rem] hidden md:table-cell">
                  {volume > 0
                    ? `$${volume / 1_000_000 >= 1 ? `${(volume / 1_000_000).toFixed(1)}M` : `${(volume / 1_000).toFixed(0)}K`}`
                    : '—'}
                </td>
                <td className="py-4 px-3 sm:py-5 sm:px-4 text-[0.85rem] hidden md:table-cell">{endDate}</td>
                <td className="py-4 px-3 sm:py-5 sm:px-4">
                  <div className="flex gap-2">
                    <Link
                      href={`/markets/${market.id}`}
                      className="flex h-10 w-20 sm:w-24 items-center justify-between gap-2 border border-border px-3 text-[0.7rem] font-semibold transition-colors hover:bg-text-primary hover:text-text-inverse"
                    >
                      <span>YES</span>
                      <span>{yesPrice}</span>
                    </Link>
                    <Link
                      href={`/markets/${market.id}`}
                      className="flex h-10 w-20 sm:w-24 items-center justify-between gap-2 border border-border px-3 text-[0.7rem] font-semibold transition-colors hover:bg-text-primary hover:text-text-inverse"
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

function formatStatNumber(n: number): string {
  if (n >= 1_000_000) return `$${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `$${(n / 1_000).toFixed(0)}K`;
  return n.toLocaleString();
}

function PlatformStatsBar({ markets, agentCount }: { markets: Market[]; agentCount: number }) {
  const totalVolume = markets.reduce((sum, m) => sum + (m.totalVolume ?? 0), 0);
  const activeMarkets = markets.filter((m) => m.status === "active").length;
  const mockStats = getMockPlatformStats();

  const stats = [
    { label: "Markets", value: activeMarkets > 0 ? activeMarkets.toString() : mockStats.totalMarkets.toString() },
    { label: "Volume", value: totalVolume > 0 ? formatStatNumber(totalVolume) : formatStatNumber(mockStats.totalVolume) },
    { label: "Traders", value: mockStats.totalTraders.toLocaleString() },
    { label: "Agents", value: agentCount > 0 ? agentCount.toString() : mockStats.activeAgents.toString() },
  ];

  return (
    <section className="border-b border-border px-4 py-3 sm:px-6">
      <div className="flex items-center justify-between gap-4 overflow-x-auto scrollbar-hide">
        {stats.map((s) => (
          <div key={s.label} className="flex flex-col items-center min-w-0">
            <span className="text-lg font-bold text-text-primary tabular-nums">{s.value}</span>
            <span className="text-[0.6rem] uppercase tracking-[0.14em] text-text-secondary">{s.label}</span>
          </div>
        ))}
      </div>
    </section>
  );
}

function HomeMarketTape({
  markets,
  signal,
}: {
  markets: Market[];
  signal: HomeLiveFeed["signal"];
}) {
  const items = [
    `Relay44 live on Base`,
    `${signal.marketsTracked} markets tracked`,
    `${signal.feedsLive}/${signal.feedsExpected} sources live`,
    `updated ${formatUtcTimestamp(signal.updatedAt)}`,
    ...markets.slice(0, 8).map((market) => {
      const yesPrice = formatPriceForTape(market.yesPrice);
      const noPrice = formatPriceForTape(market.noPrice);
      return `${truncateTapeQuestion(market.question)} | YES ${yesPrice} | NO ${noPrice}`;
    }),
  ];

  const tickerText = `${items.join(" • ")} • `;

  return (
    <div className="tape-stripe fixed bottom-0 left-0 right-0 z-30 hidden overflow-hidden border-t border-border bg-bg-primary md:block">
      <div className="overflow-hidden py-2.5 whitespace-nowrap">
        <span className="animate-marquee relative inline-block text-[11px] font-mono uppercase tracking-[0.16em] text-accent">
          {tickerText}
          {tickerText}
        </span>
      </div>
    </div>
  );
}

const HERO_SHUFFLE_MS = 10_000;

function useHeroSlider(srcs: string[], initialIndex: number) {
  const [index, setIndex] = useState(initialIndex);

  useEffect(() => {
    if (srcs.length <= 1) return;

    const interval = window.setInterval(() => {
      setIndex((prev) => (prev + 1) % srcs.length);
    }, HERO_SHUFFLE_MS);

    return () => window.clearInterval(interval);
  }, [srcs.length]);

  useEffect(() => {
    if (srcs.length <= 1) return;
    const nextSrc = srcs[(index + 1) % srcs.length];
    const img = new Image();
    img.src = nextSrc;
  }, [index, srcs]);

  return srcs[index % srcs.length];
}

export default function HomePageClient({
  initialMarkets,
  initialLiveFeed,
  heroImageSrcs,
  heroInitialIndex,
}: HomePageClientProps) {
  const [liveFeed, setLiveFeed] = useState(initialLiveFeed);
  const heroBackgroundImageSrc = useHeroSlider(heroImageSrcs, heroInitialIndex);

  const { data: marketsData, isLoading } = useMarkets(
    {
      sort: "volume",
      limit: HOME_LOOKUP_LIMIT,
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
  const {
    data: publicAgentsData,
    isLoading: isLoadingPublicAgents,
    error: publicAgentsQueryError,
  } = usePublicExternalAgents({ limit: 6, active: true });
  const onchainAgents = agentsData?.data ?? [];
  const publicPaperAgents = publicAgentsData?.data ?? [];
  const agentsError = agentsQueryError instanceof Error ? agentsQueryError : null;
  const publicAgentsError = publicAgentsQueryError instanceof Error ? publicAgentsQueryError : null;
  const marketLookup = useMemo(
    () => new Map(markets.map((market) => [market.id, market])),
    [markets]
  );
  const liveAgents = useMemo(
    () =>
      [...onchainAgents.map((agent) => toLiveFeedEntry(agent, marketLookup)), ...publicPaperAgents.map((agent) => toPublicPaperFeedEntry(agent, marketLookup))]
        .sort(compareLiveAgents)
        .slice(0, 6),
    [marketLookup, onchainAgents, publicPaperAgents]
  );
  const agentFeedError = liveAgents.length === 0 ? agentsError ?? publicAgentsError : null;
  const isLoadingLiveAgents = isLoadingAgents || isLoadingPublicAgents;
  const heroRows = buildHeroRows(liveAgents, liveFeed.signal, isLoadingLiveAgents, agentFeedError);
  const heroStatus = liveAgents.length > 0
    ? "LIVE"
    : agentFeedError
      ? "DEGRADED"
      : liveFeed.signal.feedsLive > 0
        ? "LIVE"
        : "STANDBY";
  const heroMode = liveAgents.length > 0
    ? `${liveAgents.length} LIVE AGENTS`
    : agentFeedError
      ? "AGENT FEED OFFLINE"
      : isLoadingLiveAgents
        ? "LOADING AGENTS"
        : "MARKET MONITOR";
  const displayMarkets = markets.slice(0, HOME_MARKET_LIMIT);
  const featuredMarkets = markets.slice(0, FEATURED_MARKET_COUNT);

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

      <div className="pt-header flex flex-1 overflow-hidden">
        <AgentPanel
          agents={liveAgents}
          isLoading={isLoadingLiveAgents}
          error={agentFeedError}
          signal={liveFeed.signal}
        />

        <main className="flex-1 overflow-y-auto md:pb-10">
          <OnboardingBanner />
          <section className="border-b border-border h-[280px] sm:h-[320px]">
            <HeroTicket
              accessValue="PUBLIC WEB"
              statusValue={heroStatus}
              networkValue="BASE L2"
              modeValue={heroMode}
              detailRows={heroRows}
              backgroundImageSrc={heroBackgroundImageSrc}
            />
          </section>

          <section className="py-5 border-b border-border">
            <FeaturedSlider markets={featuredMarkets} title="Signal Relay" />
          </section>

          <PlatformStatsBar markets={markets} agentCount={liveAgents.length} />

          <section className="border-b border-border px-4 py-4 sm:px-6">
            <div className="flex flex-wrap gap-3">
              <Link
                href="/markets"
                className="inline-flex h-10 items-center border border-border px-4 text-[0.7rem] uppercase tracking-[0.12em] text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
              >
                Browse markets
              </Link>
              <Link
                href="/how-it-works"
                className="inline-flex h-10 items-center border border-border px-4 text-[0.7rem] uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
              >
                How it works
              </Link>
              <Link
                href="/legal/disclaimer"
                className="inline-flex h-10 items-center border border-border px-4 text-[0.7rem] uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
              >
                Risk disclosure
              </Link>
            </div>
          </section>

          <MarketTable markets={displayMarkets} isLoading={isLoading} />
        </main>
      </div>

      <HomeMarketTape markets={displayMarkets} signal={liveFeed.signal} />
      <BottomNav />
    </div>
  );
}
