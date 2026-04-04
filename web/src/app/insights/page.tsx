"use client";

import Link from "next/link";
import { useMemo, useState } from "react";
import { PageShell } from "@/components/layout";
import { LoadingScreen } from "@/components/ui";
import { useMarkets } from "@/hooks";
import { cn } from "@/lib/utils";
import type { Market } from "@/types";

type SignalType = "high_conviction" | "time_decay" | "volume_surge" | "contrarian";

interface Signal {
  market: Market;
  type: SignalType;
  direction: "yes" | "no";
  confidence: number;
  edge: number;
  summary: string;
}

const SIGNAL_META: Record<
  SignalType,
  { label: string; color: string; description: string }
> = {
  high_conviction: {
    label: "High Conviction",
    color: "text-green-400 border-green-500/30 bg-green-500/10",
    description: "Market showing strong directional consensus",
  },
  time_decay: {
    label: "Time Decay",
    color: "text-yellow-400 border-yellow-500/30 bg-yellow-500/10",
    description: "Approaching deadline with pricing gap",
  },
  volume_surge: {
    label: "Volume Surge",
    color: "text-blue-400 border-blue-500/30 bg-blue-500/10",
    description: "Unusual volume relative to market age",
  },
  contrarian: {
    label: "Contrarian",
    color: "text-purple-400 border-purple-500/30 bg-purple-500/10",
    description: "Price diverges from volume-weighted direction",
  },
};

function deriveSignals(markets: Market[]): Signal[] {
  const signals: Signal[] = [];
  const now = Date.now();

  for (const m of markets) {
    if (m.status !== "active") continue;

    const yesP = m.yesPrice;
    const noP = m.noPrice;
    const deadline = new Date(m.resolutionDeadline).getTime();
    const hoursLeft = Math.max(0, (deadline - now) / 3_600_000);

    // High conviction: price above 85% or below 15%
    if (yesP >= 0.85) {
      signals.push({
        market: m,
        type: "high_conviction",
        direction: "yes",
        confidence: yesP,
        edge: yesP - 0.5,
        summary: `Trading at ${Math.round(yesP * 100)}¢ YES — strong consensus toward resolution YES.`,
      });
    } else if (noP >= 0.85) {
      signals.push({
        market: m,
        type: "high_conviction",
        direction: "no",
        confidence: noP,
        edge: noP - 0.5,
        summary: `Trading at ${Math.round(noP * 100)}¢ NO — strong consensus toward resolution NO.`,
      });
    }

    // Time decay: within 48h of deadline with price 30-70%
    if (hoursLeft > 0 && hoursLeft <= 48 && yesP > 0.3 && yesP < 0.7) {
      signals.push({
        market: m,
        type: "time_decay",
        direction: yesP >= 0.5 ? "yes" : "no",
        confidence: Math.abs(yesP - 0.5) + 0.5,
        edge: (48 - hoursLeft) / 48,
        summary: `${Math.round(hoursLeft)}h to deadline at ${Math.round(yesP * 100)}¢ — theta pressure building.`,
      });
    }

    // Volume surge: high 24h volume relative to total
    if (m.totalVolume > 0 && m.volume24h / m.totalVolume > 0.3 && m.volume24h > 100) {
      signals.push({
        market: m,
        type: "volume_surge",
        direction: yesP >= 0.5 ? "yes" : "no",
        confidence: Math.min(0.95, 0.5 + m.volume24h / m.totalVolume * 0.3),
        edge: m.volume24h / m.totalVolume,
        summary: `24h volume is ${Math.round((m.volume24h / m.totalVolume) * 100)}% of lifetime volume — unusual activity.`,
      });
    }

    // Contrarian: price near extremes but volume concentrated in opposite direction
    if (yesP >= 0.75 && m.volume24h > 50 && m.noSupply > m.yesSupply * 1.5) {
      signals.push({
        market: m,
        type: "contrarian",
        direction: "no",
        confidence: 0.6,
        edge: 0.15,
        summary: `YES at ${Math.round(yesP * 100)}¢ but NO supply outweighs YES — potential divergence.`,
      });
    } else if (noP >= 0.75 && m.volume24h > 50 && m.yesSupply > m.noSupply * 1.5) {
      signals.push({
        market: m,
        type: "contrarian",
        direction: "yes",
        confidence: 0.6,
        edge: 0.15,
        summary: `NO at ${Math.round(noP * 100)}¢ but YES supply outweighs NO — potential divergence.`,
      });
    }
  }

  return signals.sort((a, b) => b.edge - a.edge);
}

const FILTER_OPTIONS: { value: SignalType | "all"; label: string }[] = [
  { value: "all", label: "All signals" },
  { value: "high_conviction", label: "High conviction" },
  { value: "time_decay", label: "Time decay" },
  { value: "volume_surge", label: "Volume surge" },
  { value: "contrarian", label: "Contrarian" },
];

const SORT_OPTIONS = [
  { value: "edge", label: "Edge magnitude" },
  { value: "confidence", label: "Confidence" },
  { value: "volume", label: "24h volume" },
] as const;

type SortKey = (typeof SORT_OPTIONS)[number]["value"];

export default function InsightsPage() {
  const { data, isLoading } = useMarkets({ limit: 200, status: "active" });
  const [filter, setFilter] = useState<SignalType | "all">("all");
  const [sort, setSort] = useState<SortKey>("edge");

  const signals = useMemo(() => {
    if (!data?.data) return [];
    return deriveSignals(data.data);
  }, [data]);

  const filtered = useMemo(() => {
    let result = filter === "all" ? signals : signals.filter((s) => s.type === filter);

    if (sort === "confidence") {
      result = [...result].sort((a, b) => b.confidence - a.confidence);
    } else if (sort === "volume") {
      result = [...result].sort((a, b) => b.market.volume24h - a.market.volume24h);
    }

    return result;
  }, [signals, filter, sort]);

  if (isLoading) {
    return (
      <PageShell>
        <LoadingScreen />
      </PageShell>
    );
  }

  return (
    <PageShell>
      <div className="mb-6">
        <h1 className="text-xl font-semibold text-text-primary">
          Market Insights
        </h1>
        <p className="mt-1 text-sm text-text-secondary">
          Automated edge signals derived from price, volume, and time decay
          across active markets.
        </p>
      </div>

      <div className="flex flex-wrap items-center gap-3 mb-6">
        <div className="flex flex-wrap gap-1.5">
          {FILTER_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => setFilter(opt.value)}
              className={cn(
                "h-8 px-3 text-[0.65rem] uppercase tracking-[0.12em] font-mono border transition-colors",
                filter === opt.value
                  ? "border-accent text-accent bg-accent/10"
                  : "border-border text-text-muted hover:text-text-primary hover:border-border-hover",
              )}
            >
              {opt.label}
            </button>
          ))}
        </div>

        <div className="ml-auto flex items-center gap-2">
          <span className="text-xs text-text-muted font-mono uppercase">Sort:</span>
          <select
            value={sort}
            onChange={(e) => setSort(e.target.value as SortKey)}
            className="h-8 bg-transparent border border-border px-2 text-xs font-mono text-text-primary focus:outline-none focus:border-border-hover"
          >
            {SORT_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>
      </div>

      {filtered.length === 0 ? (
        <div className="text-center py-16">
          <p className="text-text-secondary">
            No signals detected for the current filter.
          </p>
        </div>
      ) : (
        <div className="grid gap-3">
          {filtered.map((signal, i) => {
            const meta = SIGNAL_META[signal.type];
            const deadline = new Date(signal.market.resolutionDeadline);
            const hoursLeft = Math.max(
              0,
              (deadline.getTime() - Date.now()) / 3_600_000,
            );

            return (
              <Link
                key={`${signal.market.id}-${signal.type}-${i}`}
                href={`/markets/${encodeURIComponent(signal.market.id)}`}
                className="block border border-border bg-bg-secondary/40 p-4 transition-colors hover:border-border-hover hover:bg-bg-hover"
              >
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2 mb-2">
                      <span
                        className={cn(
                          "inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-mono uppercase",
                          meta.color,
                        )}
                      >
                        {meta.label}
                      </span>
                      <span
                        className={cn(
                          "inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-mono uppercase",
                          signal.direction === "yes"
                            ? "text-green-400 border-green-500/30"
                            : "text-red-400 border-red-500/30",
                        )}
                      >
                        {signal.direction}
                      </span>
                      <span className="text-[10px] text-text-muted font-mono uppercase">
                        {signal.market.category}
                      </span>
                    </div>

                    <h3 className="text-sm font-medium text-text-primary line-clamp-2">
                      {signal.market.question}
                    </h3>

                    <p className="mt-1.5 text-xs text-text-secondary">
                      {signal.summary}
                    </p>
                  </div>

                  <div className="flex flex-col items-end gap-1 shrink-0 text-right">
                    <div className="text-lg font-mono font-medium text-text-primary">
                      {Math.round(signal.market.yesPrice * 100)}¢
                    </div>
                    <div className="text-[10px] text-text-muted font-mono">
                      {hoursLeft < 24
                        ? `${Math.round(hoursLeft)}h left`
                        : `${Math.round(hoursLeft / 24)}d left`}
                    </div>
                    <div className="text-[10px] text-text-muted font-mono">
                      ${signal.market.volume24h >= 1000
                        ? `${(signal.market.volume24h / 1000).toFixed(1)}k`
                        : signal.market.volume24h.toFixed(0)}{" "}
                      24h vol
                    </div>
                  </div>
                </div>

                <div className="mt-3 flex items-center gap-4">
                  <div className="flex-1 h-1 bg-border rounded-full overflow-hidden">
                    <div
                      className="h-full bg-accent rounded-full transition-all"
                      style={{
                        width: `${Math.round(signal.confidence * 100)}%`,
                      }}
                    />
                  </div>
                  <span className="text-[10px] text-text-muted font-mono shrink-0">
                    {Math.round(signal.confidence * 100)}% confidence
                  </span>
                </div>
              </Link>
            );
          })}
        </div>
      )}
    </PageShell>
  );
}
