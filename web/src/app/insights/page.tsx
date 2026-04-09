"use client";

import { useMemo, useState } from "react";
import { PageShell } from "@/components/layout";
import { LoadingScreen, ErrorMessage } from "@/components/ui";
import { useScannerOpportunities, useScannerCalibration, useScannerRuns } from "@/hooks";
import { cn } from "@/lib/utils";
import type { ScannedOpportunity, CalibrationBucket, ScanRun } from "@/types";

// ── Opportunity type metadata ──

type OpportunityFilter = "all" | "longshot" | "near_certainty" | "spread_capture";

const FILTER_OPTIONS: { value: OpportunityFilter; label: string }[] = [
  { value: "all", label: "All opportunities" },
  { value: "longshot", label: "Longshots" },
  { value: "near_certainty", label: "Near certainties" },
  { value: "spread_capture", label: "Spread capture" },
];

const SORT_OPTIONS = [
  { value: "score", label: "Opportunity score" },
  { value: "mispricing", label: "Mispricing" },
  { value: "volume", label: "Volume" },
  { value: "spread", label: "Spread" },
] as const;

type SortKey = (typeof SORT_OPTIONS)[number]["value"];

function opportunityColor(type: string): string {
  if (type.startsWith("longshot")) {
    return "text-purple-400 border-purple-500/30 bg-purple-500/10";
  }
  if (type.startsWith("near_certainty")) {
    return "text-green-400 border-green-500/30 bg-green-500/10";
  }
  if (type === "spread_capture") {
    return "text-blue-400 border-blue-500/30 bg-blue-500/10";
  }
  return "text-text-muted border-border bg-bg-secondary/40";
}

function opportunityLabel(type: string): string {
  if (type.startsWith("longshot")) return "Longshot";
  if (type.startsWith("near_certainty")) return "Near certainty";
  if (type === "spread_capture") return "Spread capture";
  return type;
}

function directionFromType(type: string): "yes" | "no" | null {
  if (type.endsWith("_yes")) return "yes";
  if (type.endsWith("_no")) return "no";
  return null;
}

function formatUsdc(value: number): string {
  if (value >= 1_000_000) return `$${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `$${(value / 1_000).toFixed(1)}k`;
  return `$${value.toFixed(0)}`;
}

function formatTimeAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

// ── Tabs ──

type InsightsTab = "opportunities" | "calibration" | "history";

const TABS: { value: InsightsTab; label: string }[] = [
  { value: "opportunities", label: "Opportunities" },
  { value: "calibration", label: "Calibration" },
  { value: "history", label: "Scan history" },
];

// ── Main component ──

export default function InsightsPage() {
  const [tab, setTab] = useState<InsightsTab>("opportunities");
  const [filter, setFilter] = useState<OpportunityFilter>("all");
  const [sort, setSort] = useState<SortKey>("score");

  const {
    data: oppData,
    isLoading: oppLoading,
    error: oppError,
  } = useScannerOpportunities({ limit: 200 });

  const {
    data: calData,
    isLoading: calLoading,
    error: calError,
  } = useScannerCalibration(tab === "calibration");

  const {
    data: runsData,
    isLoading: runsLoading,
    error: runsError,
  } = useScannerRuns(tab === "history");

  const opportunities: ScannedOpportunity[] = oppData?.opportunities ?? [];

  const filtered = useMemo(() => {
    let result =
      filter === "all"
        ? opportunities
        : opportunities.filter((o) => o.opportunityType.startsWith(filter));

    if (sort === "mispricing") {
      result = [...result].sort((a, b) => b.mispricingScore - a.mispricingScore);
    } else if (sort === "volume") {
      result = [...result].sort((a, b) => b.volumeUsdc - a.volumeUsdc);
    } else if (sort === "spread") {
      result = [...result].sort((a, b) => b.spreadBps - a.spreadBps);
    }
    // Default "score" sort is already from backend (ORDER BY opportunity_score DESC)

    return result;
  }, [opportunities, filter, sort]);

  if (oppLoading && tab === "opportunities") {
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
          Edge scanner opportunities from Polymarket calibration analysis,
          longshot detection, and spread capture.
        </p>
      </div>

      {/* Tab bar */}
      <div className="flex gap-1.5 mb-6 border-b border-border pb-px">
        {TABS.map((t) => (
          <button
            key={t.value}
            type="button"
            onClick={() => setTab(t.value)}
            className={cn(
              "h-8 px-3 text-[0.65rem] uppercase tracking-[0.12em] font-mono border-b-2 transition-colors -mb-px",
              tab === t.value
                ? "border-accent text-accent"
                : "border-transparent text-text-muted hover:text-text-primary",
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* Opportunities tab */}
      {tab === "opportunities" && (
        <OpportunitiesTab
          opportunities={filtered}
          filter={filter}
          sort={sort}
          onFilterChange={setFilter}
          onSortChange={setSort}
          error={oppError}
        />
      )}

      {/* Calibration tab */}
      {tab === "calibration" && (
        <CalibrationTab
          buckets={calData?.calibrationBuckets ?? []}
          isLoading={calLoading}
          error={calError}
        />
      )}

      {/* History tab */}
      {tab === "history" && (
        <HistoryTab
          runs={runsData?.runs ?? []}
          isLoading={runsLoading}
          error={runsError}
        />
      )}
    </PageShell>
  );
}

// ── Opportunities tab ──

function OpportunitiesTab({
  opportunities,
  filter,
  sort,
  onFilterChange,
  onSortChange,
  error,
}: {
  opportunities: ScannedOpportunity[];
  filter: OpportunityFilter;
  sort: SortKey;
  onFilterChange: (f: OpportunityFilter) => void;
  onSortChange: (s: SortKey) => void;
  error: Error | null;
}) {
  if (error) {
    return (
      <ErrorMessage
        message="Failed to load scanner opportunities. The scanner may be disabled or temporarily unavailable."
      />
    );
  }

  return (
    <>
      <div className="flex flex-wrap items-center gap-3 mb-6">
        <div className="flex flex-wrap gap-1.5">
          {FILTER_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => onFilterChange(opt.value)}
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
          <span className="text-xs text-text-muted font-mono uppercase">
            Sort:
          </span>
          <select
            value={sort}
            onChange={(e) => onSortChange(e.target.value as SortKey)}
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

      {opportunities.length === 0 ? (
        <div className="text-center py-16">
          <p className="text-text-secondary">
            No opportunities detected for the current filter.
          </p>
          <p className="mt-1 text-xs text-text-muted">
            The scanner runs periodically. Check back later or try a different filter.
          </p>
        </div>
      ) : (
        <div className="grid gap-3">
          {opportunities.map((opp) => (
            <OpportunityCard key={`${opp.conditionId}-${opp.opportunityType}`} opportunity={opp} />
          ))}
        </div>
      )}
    </>
  );
}

function OpportunityCard({ opportunity: opp }: { opportunity: ScannedOpportunity }) {
  const direction = directionFromType(opp.opportunityType);
  const polymarketUrl = `https://polymarket.com/event/${opp.slug}`;

  return (
    <a
      href={polymarketUrl}
      target="_blank"
      rel="noopener noreferrer"
      className="block border border-border bg-bg-secondary/40 p-4 transition-colors hover:border-border-hover hover:bg-bg-hover"
    >
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2 mb-2">
            <span
              className={cn(
                "inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-mono uppercase",
                opportunityColor(opp.opportunityType),
              )}
            >
              {opportunityLabel(opp.opportunityType)}
            </span>
            {direction && (
              <span
                className={cn(
                  "inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-mono uppercase",
                  direction === "yes"
                    ? "text-green-400 border-green-500/30"
                    : "text-red-400 border-red-500/30",
                )}
              >
                {direction}
              </span>
            )}
            <span className="text-[10px] text-text-muted font-mono uppercase">
              {opp.category}
            </span>
          </div>

          <h3 className="text-sm font-medium text-text-primary line-clamp-2">
            {opp.question}
          </h3>

          <div className="mt-1.5 flex flex-wrap items-center gap-3 text-xs text-text-secondary">
            <span>
              Mispricing: {(opp.mispricingScore * 100).toFixed(1)}%
            </span>
            <span>Spread: {opp.spreadBps}bps</span>
            <span>Fee: {opp.feeRateBps}bps</span>
            {opp.lastScannedAt && (
              <span>Scanned {formatTimeAgo(opp.lastScannedAt)}</span>
            )}
          </div>
        </div>

        <div className="flex flex-col items-end gap-1 shrink-0 text-right">
          <div className="text-lg font-mono font-medium text-text-primary">
            {Math.round(opp.yesPrice * 100)}/{Math.round(opp.noPrice * 100)}
          </div>
          <div className="text-[10px] text-text-muted font-mono">
            YES / NO
          </div>
          <div className="text-[10px] text-text-muted font-mono">
            {formatUsdc(opp.volumeUsdc)} vol
          </div>
          <div className="text-[10px] text-text-muted font-mono">
            {formatUsdc(opp.liquidityUsdc)} liq
          </div>
        </div>
      </div>

      <div className="mt-3 flex items-center gap-4">
        <div className="flex-1 h-1 bg-border rounded-full overflow-hidden">
          <div
            className="h-full bg-accent rounded-full transition-all"
            style={{
              width: `${Math.min(100, Math.round(opp.opportunityScore * 100))}%`,
            }}
          />
        </div>
        <span className="text-[10px] text-text-muted font-mono shrink-0">
          {Math.round(opp.opportunityScore * 100)}% score
        </span>
      </div>
    </a>
  );
}

// ── Calibration tab ──

function CalibrationTab({
  buckets,
  isLoading,
  error,
}: {
  buckets: CalibrationBucket[];
  isLoading: boolean;
  error: Error | null;
}) {
  if (isLoading) return <LoadingScreen />;

  if (error) {
    return (
      <ErrorMessage
        message="Failed to load calibration data."
      />
    );
  }

  if (buckets.length === 0) {
    return (
      <div className="text-center py-16">
        <p className="text-text-secondary">
          No calibration data available yet.
        </p>
        <p className="mt-1 text-xs text-text-muted">
          Calibration buckets are built as the scanner accumulates resolved market data.
        </p>
      </div>
    );
  }

  // Group by category
  const grouped = buckets.reduce<Record<string, CalibrationBucket[]>>((acc, b) => {
    const key = b.category || "uncategorized";
    if (!acc[key]) acc[key] = [];
    acc[key].push(b);
    return acc;
  }, {});

  return (
    <div className="space-y-6">
      <p className="text-sm text-text-secondary">
        Calibration compares implied probability (market price) against actual
        historical win rates. Positive mispricing means the market underprices
        the true probability.
      </p>

      {Object.entries(grouped).map(([category, catBuckets]) => (
        <div key={category}>
          <h3 className="text-sm font-mono font-semibold text-text-primary uppercase tracking-wide mb-3">
            {category}
          </h3>
          <div className="overflow-x-auto">
            <table className="w-full text-xs font-mono">
              <thead>
                <tr className="border-b border-border text-text-muted text-left">
                  <th className="pb-2 pr-4">Price range</th>
                  <th className="pb-2 pr-4">Positions</th>
                  <th className="pb-2 pr-4">Wins</th>
                  <th className="pb-2 pr-4">Actual win %</th>
                  <th className="pb-2 pr-4">Implied prob</th>
                  <th className="pb-2 pr-4">Mispricing</th>
                </tr>
              </thead>
              <tbody>
                {catBuckets.map((b) => {
                  const mispricingColor =
                    b.mispricingPct > 5
                      ? "text-green-400"
                      : b.mispricingPct < -5
                        ? "text-red-400"
                        : "text-text-secondary";

                  return (
                    <tr
                      key={`${b.category}-${b.priceBucketLow}-${b.priceBucketHigh}`}
                      className="border-b border-border/50"
                    >
                      <td className="py-2 pr-4 text-text-primary">
                        {Math.round(b.priceBucketLow * 100)}-
                        {Math.round(b.priceBucketHigh * 100)}%
                      </td>
                      <td className="py-2 pr-4 text-text-secondary">
                        {b.totalPositions}
                      </td>
                      <td className="py-2 pr-4 text-text-secondary">
                        {b.wins}
                      </td>
                      <td className="py-2 pr-4 text-text-primary">
                        {(b.actualWinRate * 100).toFixed(1)}%
                      </td>
                      <td className="py-2 pr-4 text-text-secondary">
                        {(b.impliedProbability * 100).toFixed(1)}%
                      </td>
                      <td className={cn("py-2 pr-4", mispricingColor)}>
                        {b.mispricingPct > 0 ? "+" : ""}
                        {b.mispricingPct.toFixed(1)}%
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      ))}
    </div>
  );
}

// ── History tab ──

function HistoryTab({
  runs,
  isLoading,
  error,
}: {
  runs: ScanRun[];
  isLoading: boolean;
  error: Error | null;
}) {
  if (isLoading) return <LoadingScreen />;

  if (error) {
    return (
      <ErrorMessage
        message="Failed to load scan history."
      />
    );
  }

  if (runs.length === 0) {
    return (
      <div className="text-center py-16">
        <p className="text-text-secondary">No scan runs recorded yet.</p>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      <p className="text-sm text-text-secondary mb-4">
        Recent automated scan cycles and their results.
      </p>

      {runs.map((run) => {
        const duration =
          run.completedAt && run.startedAt
            ? Math.round(
                (new Date(run.completedAt).getTime() -
                  new Date(run.startedAt).getTime()) /
                  1000,
              )
            : null;
        const hasError = !!run.error;

        return (
          <div
            key={run.id}
            className={cn(
              "border p-4 bg-bg-secondary/40",
              hasError
                ? "border-red-500/30"
                : "border-border",
            )}
          >
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div>
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-sm font-mono text-text-primary">
                    {new Date(run.startedAt).toLocaleString()}
                  </span>
                  {hasError && (
                    <span className="inline-flex items-center rounded-full border border-red-500/30 bg-red-500/10 px-2 py-0.5 text-[10px] font-mono uppercase text-red-400">
                      Error
                    </span>
                  )}
                </div>
                {hasError && (
                  <p className="text-xs text-red-400 mt-1">{run.error}</p>
                )}
              </div>

              <div className="text-right text-[10px] font-mono text-text-muted">
                {duration !== null && <div>{duration}s duration</div>}
                <div>{run.marketsScanned} markets scanned</div>
              </div>
            </div>

            <div className="mt-3 flex flex-wrap gap-4 text-xs font-mono">
              <div>
                <span className="text-text-muted">Opportunities: </span>
                <span className="text-text-primary">{run.opportunitiesFound}</span>
              </div>
              <div>
                <span className="text-text-muted">Longshots: </span>
                <span className="text-purple-400">{run.longshotsFound}</span>
              </div>
              <div>
                <span className="text-text-muted">Near certainties: </span>
                <span className="text-green-400">{run.nearCertaintiesFound}</span>
              </div>
              <div>
                <span className="text-text-muted">Spreads: </span>
                <span className="text-blue-400">{run.spreadCapturesFound}</span>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
