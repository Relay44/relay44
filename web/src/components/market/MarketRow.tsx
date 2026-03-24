"use client";

import Link from "next/link";
import { useRuntimeMode } from "@/hooks";
import { cn } from "@/lib/utils";
import type { Market } from "@/types";

export interface MarketRowProps {
  market: Market;
  index: number;
}

function formatVolume(volume: number): string {
  if (volume >= 1_000_000) {
    return `$${(volume / 1_000_000).toFixed(1)}M`;
  }
  if (volume >= 1_000) {
    return `$${Math.round(volume / 1_000)}K`;
  }
  return `$${volume.toLocaleString()}`;
}

export function MarketRow({ market, index }: MarketRowProps) {
  const { readOnly } = useRuntimeMode();
  const yesPercent = Math.round(market.yesPrice * 100);
  const hexIndex = (index + 1).toString(16).toUpperCase().padStart(1, "0");

  return (
    <Link
      href={`/markets/${encodeURIComponent(market.id)}`}
      className="group block"
    >
      <div
        className={cn(
          "flex items-center gap-4 sm:gap-6 py-5 border-b border-border",
          "hover:bg-bg-hover transition-colors duration-fast"
        )}
      >
        <span className="text-text-muted text-sm font-mono hidden sm:block w-12 flex-shrink-0">
          [0X{hexIndex}]
        </span>
        <span className="font-bold uppercase text-base sm:text-lg flex-1 text-text-primary group-hover:text-accent transition-colors leading-snug">
          {market.question}
        </span>
        <span className="text-text-muted text-sm font-mono hidden md:block">
          ODDS: {yesPercent}%
        </span>
        <span className="text-text-muted text-sm font-mono hidden md:block">
          VOL: {formatVolume(market.totalVolume)}
        </span>
        <span className="text-text-primary text-sm font-bold sm:hidden">
          {yesPercent}%
        </span>
        <span
          className={cn(
            "border border-border px-4 py-1.5 text-sm font-bold uppercase",
            "hover:bg-accent hover:text-white hover:border-accent",
            "transition-all duration-fast flex-shrink-0"
          )}
        >
          {readOnly ? "View" : "Trade"}
        </span>
      </div>
    </Link>
  );
}
