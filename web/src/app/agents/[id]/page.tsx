"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useMemo } from "react";
import { PageShell } from "@/components/layout";
import { LoadingScreen } from "@/components/ui";
import { ReputationPanel } from "@/components/reputation/ReputationPanel";
import { usePublicExternalAgents } from "@/hooks";
import { formatPublicPaperAgentName } from "@/lib/publicPaperAgents";
import { cn } from "@/lib/utils";
import type { ExternalAgentPaperPerformance } from "@/lib/api";

function formatUsd(value: number) {
  if (!Number.isFinite(value) || value === 0) return "$0";
  const abs = Math.abs(value);
  const sign = value < 0 ? "-" : "";
  if (abs >= 1_000_000) return `${sign}$${(abs / 1_000_000).toFixed(1)}M`;
  if (abs >= 1_000) return `${sign}$${(abs / 1_000).toFixed(1)}k`;
  return `${sign}$${abs.toFixed(abs >= 100 ? 0 : 2)}`;
}

function formatTimestamp(value: string | null | undefined) {
  if (!value) return "--";
  const d = new Date(value);
  if (!Number.isFinite(d.getTime())) return "--";
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function winRate(perf: ExternalAgentPaperPerformance | null | undefined) {
  if (!perf || perf.closedPositions === 0) return "--";
  const profitable = perf.realizedPnlUsdc > 0 ? perf.closedPositions : 0;
  return `${Math.round((profitable / perf.closedPositions) * 100)}%`;
}

function StatCard({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone?: "positive" | "negative" | "neutral";
}) {
  return (
    <div className="border border-border bg-bg-secondary/40 px-4 py-3">
      <div className="text-[10px] uppercase tracking-[0.18em] text-text-muted font-mono">
        {label}
      </div>
      <div
        className={cn(
          "mt-1 text-lg font-mono font-medium",
          tone === "positive" && "text-green-400",
          tone === "negative" && "text-red-400",
          (!tone || tone === "neutral") && "text-text-primary",
        )}
      >
        {value}
      </div>
    </div>
  );
}

export default function AgentDetailPage() {
  const params = useParams();
  const agentId = decodeURIComponent(params.id as string);
  const { data, isLoading } = usePublicExternalAgents({ limit: 500 });

  const agent = useMemo(
    () => data?.data.find((a) => a.id === agentId) ?? null,
    [data, agentId],
  );

  if (isLoading) {
    return (
      <PageShell>
        <LoadingScreen />
      </PageShell>
    );
  }

  if (!agent) {
    return (
      <PageShell>
        <div className="text-center py-12">
          <h2 className="text-xl font-semibold mb-2">Agent not found</h2>
          <Link href="/agents" className="text-accent hover:text-accent-hover">
            Back to Agents
          </Link>
        </div>
      </PageShell>
    );
  }

  const displayName = formatPublicPaperAgentName(agent);
  const perf = agent.paper_performance;
  const pnlTone =
    (perf?.netPnlUsdc ?? 0) > 0
      ? "positive"
      : (perf?.netPnlUsdc ?? 0) < 0
        ? "negative"
        : "neutral";

  return (
    <PageShell>
      <Link
        href="/agents"
        className="inline-flex items-center gap-2 p-1 -ml-1 text-text-secondary hover:text-text-primary mb-4"
      >
        <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
        </svg>
        Back to Agents
      </Link>

      <div className="flex flex-wrap items-center gap-3 mb-6">
        <h1 className="text-xl font-semibold text-text-primary">
          {displayName}
        </h1>
        <span
          className={cn(
            "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-mono uppercase",
            agent.active
              ? "border-green-500/30 bg-green-500/10 text-green-400"
              : "border-border bg-bg-secondary text-text-muted",
          )}
        >
          {agent.active && (
            <span className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
          )}
          {agent.active ? "active" : "paused"}
        </span>
        <span className="text-xs font-mono uppercase tracking-wider text-text-muted">
          {agent.execution_mode}
        </span>
      </div>

      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-3 mb-6">
        <StatCard
          label="Net PnL"
          value={formatUsd(perf?.netPnlUsdc ?? 0)}
          tone={pnlTone as "positive" | "negative" | "neutral"}
        />
        <StatCard
          label="Volume"
          value={formatUsd(perf?.volumeUsdc ?? 0)}
        />
        <StatCard
          label="Realized PnL"
          value={formatUsd(perf?.realizedPnlUsdc ?? 0)}
          tone={
            (perf?.realizedPnlUsdc ?? 0) > 0
              ? "positive"
              : (perf?.realizedPnlUsdc ?? 0) < 0
                ? "negative"
                : "neutral"
          }
        />
        <StatCard label="Max Drawdown" value={formatUsd(perf?.maxDrawdownUsdc ?? 0)} />
        <StatCard label="Fills" value={String(perf?.fills ?? 0)} />
      </div>

      <div className="grid sm:grid-cols-2 gap-6 mb-6">
        <div className="border border-border bg-bg-secondary/40 p-5">
          <h2 className="text-sm font-medium text-text-primary mb-4">
            Strategy
          </h2>
          <dl className="space-y-3 text-sm">
            <div className="flex justify-between">
              <dt className="text-text-muted">Strategy</dt>
              <dd className="text-text-primary font-mono">{agent.strategy_label}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Provider</dt>
              <dd className="text-text-primary font-mono capitalize">{agent.provider}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Outcome</dt>
              <dd className={cn("font-mono uppercase", agent.outcome === "yes" ? "text-green-400" : "text-red-400")}>
                {agent.outcome}
              </dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Side</dt>
              <dd className="text-text-primary font-mono uppercase">{agent.side}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Price</dt>
              <dd className="text-text-primary font-mono">{Math.round(agent.price * 100)}¢</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Quantity</dt>
              <dd className="text-text-primary font-mono">{agent.quantity}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Cadence</dt>
              <dd className="text-text-primary font-mono">{agent.cadence_seconds}s</dd>
            </div>
          </dl>
        </div>

        <div className="border border-border bg-bg-secondary/40 p-5">
          <h2 className="text-sm font-medium text-text-primary mb-4">
            Position Summary
          </h2>
          <dl className="space-y-3 text-sm">
            <div className="flex justify-between">
              <dt className="text-text-muted">Open Positions</dt>
              <dd className="text-text-primary font-mono">{perf?.openPositions ?? 0}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Closed Positions</dt>
              <dd className="text-text-primary font-mono">{perf?.closedPositions ?? 0}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Unrealized PnL</dt>
              <dd className={cn("font-mono", (perf?.unrealizedPnlUsdc ?? 0) >= 0 ? "text-green-400" : "text-red-400")}>
                {formatUsd(perf?.unrealizedPnlUsdc ?? 0)}
              </dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Fees</dt>
              <dd className="text-text-primary font-mono">{formatUsd(perf?.feesUsdc ?? 0)}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Last Executed</dt>
              <dd className="text-text-primary font-mono text-xs">{formatTimestamp(agent.last_executed_at)}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Next Execution</dt>
              <dd className="text-text-primary font-mono text-xs">{formatTimestamp(agent.next_execution_at)}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-text-muted">Created</dt>
              <dd className="text-text-primary font-mono text-xs">{formatTimestamp(agent.created_at)}</dd>
            </div>
          </dl>
        </div>
      </div>

      {agent.market_id && (
        <div className="border border-border bg-bg-secondary/40 p-5 mb-6">
          <h2 className="text-sm font-medium text-text-primary mb-3">
            Linked Market
          </h2>
          <Link
            href={`/markets/${encodeURIComponent(agent.market_id)}`}
            className="inline-flex items-center gap-2 text-sm text-accent hover:text-accent-hover font-mono transition-colors"
          >
            {agent.market_id}
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
            </svg>
          </Link>
        </div>
      )}

      {agent.owner && agent.owner.startsWith("0x") && (
        <ReputationPanel wallet={agent.owner} />
      )}
    </PageShell>
  );
}
