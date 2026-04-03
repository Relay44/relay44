import Link from "next/link";
import Image from "next/image";
import { ArrowUpRight, RefreshCw } from "lucide-react";
import { MarketArtwork } from "@/components/market/MarketArtwork";
import { getBootstrapStatusLabel } from "@/lib/bootstrap";
import { cn } from "@/lib/utils";
import type { Market } from "@/types";

const PROVIDER_LOGOS: Record<string, string> = {
  limitless: "/limitless.svg",
  polymarket: "/polymarket.svg",
};

export interface MarketCardProps {
  market: Market;
  compact?: boolean;
}

function formatVolume(volume: number): string {
  if (volume >= 1_000_000) {
    return `$${(volume / 1_000_000).toFixed(1)}M`;
  }
  if (volume >= 1_000) {
    return `$${Math.round(volume / 1_000)}k`;
  }
  return `$${volume.toLocaleString()}`;
}

function formatFrequency(frequency?: string): string {
  if (!frequency) return "";
  return frequency.charAt(0).toUpperCase() + frequency.slice(1);
}

export function MarketCard({ market }: MarketCardProps) {
  const outcomes = market.outcomes || [
    { label: "Yes", probability: market.yesPrice },
    { label: "No", probability: market.noPrice },
  ];
  const displayOutcomes = outcomes.slice(0, 2);
  const bootstrapLabel = getBootstrapStatusLabel(market);

  return (
    <Link
      href={`/markets/${encodeURIComponent(market.id)}`}
      className="block group"
    >
      <div
        className={cn(
          "relative h-full overflow-hidden micro-surface border border-border/70 p-4",
          "hover:border-accent hover:shadow-md hover:-translate-y-0.5",
          "transition-all duration-fast cursor-pointer flex flex-col",
        )}
      >
        {/* Header: Image + Question */}
        <div className="relative flex items-start gap-3 mb-4">
          <MarketArtwork
            market={market}
            className="h-12 w-12 shrink-0"
            sizes="48px"
          />
          <div className="flex-1">
            <div className="flex items-center gap-1.5 text-[11px] uppercase tracking-[0.16em] text-text-muted mb-1">
              <span className="inline-flex items-center gap-1 px-2 py-0.5 border border-border bg-bg-secondary/60">
                {PROVIDER_LOGOS[market.provider?.toLowerCase()] ? (
                  <Image
                    src={PROVIDER_LOGOS[market.provider.toLowerCase()]}
                    alt={market.provider}
                    width={14}
                    height={14}
                    className="inline-block"
                  />
                ) : null}
                {market.provider}
              </span>
              <span className="px-2 py-0.5 border border-border bg-bg-secondary/60">
                {market.chainId === 137
                  ? "polygon"
                  : market.chainId === 8453
                    ? "base"
                    : `chain-${market.chainId}`}
              </span>
              {market.liquidityMode === "bootstrap_hybrid" ? (
                <span className="px-2 py-0.5 border border-accent/30 bg-accent/10 text-accent">
                  {bootstrapLabel}
                </span>
              ) : null}
            </div>
            <h3 className="font-semibold text-text-primary text-sm leading-snug line-clamp-2 group-hover:text-accent transition-colors">
              {market.question}
            </h3>
          </div>
        </div>

        {/* Outcome rows */}
        <div className="relative space-y-2 mb-4 flex-1">
          {displayOutcomes.map((outcome, idx) => {
            const percent = Math.round(outcome.probability * 100);
            const isYes = outcome.label.toLowerCase().includes("yes");
            return (
              <div
                key={idx}
                className="flex items-center gap-2 bg-bg-secondary/60 border border-border px-3 py-2"
              >
                <span className="text-xs uppercase tracking-[0.12em] text-text-muted flex items-center gap-2">
                  <span
                    className="w-2 h-2 rounded-full"
                    style={{
                      backgroundColor: isYes
                        ? "var(--color-yes)"
                        : "var(--color-no)",
                    }}
                  />
                  {outcome.label}
                </span>
                <span className="ml-auto text-base font-semibold text-text-primary">
                  {percent}%
                </span>
              </div>
            );
          })}
        </div>

        {/* Footer */}
        <div className="relative flex items-center justify-between pt-3 border-t border-border">
          <div className="flex items-center gap-2 text-xs text-text-muted">
            <span className="font-semibold text-text-primary">
              {formatVolume(market.totalVolume)}
            </span>
            {market.liquidityMode === "bootstrap_hybrid" &&
            market.bootstrapSeedUsdc ? (
              <span className="text-text-secondary">
                {bootstrapLabel} ${Math.round(market.bootstrapSeedUsdc)}
              </span>
            ) : null}
            {typeof market.tradabilityScore === "number" ? (
              <span className="text-text-secondary">
                tradability {Math.round(market.tradabilityScore)}
              </span>
            ) : null}
            {market.frequency && (
              <>
                <RefreshCw className="w-3 h-3" />
                <span>{formatFrequency(market.frequency)}</span>
              </>
            )}
          </div>
          <span className="inline-flex items-center gap-1.5 text-xs font-medium uppercase tracking-[0.12em] text-text-muted">
            Open market
            <ArrowUpRight className="w-3.5 h-3.5" />
          </span>
        </div>
      </div>
    </Link>
  );
}
