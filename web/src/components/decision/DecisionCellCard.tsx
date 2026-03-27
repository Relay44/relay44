"use client";

import Link from "next/link";

import { Card } from "@/components/ui";
import {
  decisionTypeLabel,
  formatPercentFromBps,
  recommendationLabel,
} from "@/lib/decisionCells";
import type { DecisionCellListItem } from "@/types";

interface DecisionCellCardProps {
  cell: DecisionCellListItem;
}

export function DecisionCellCard({ cell }: DecisionCellCardProps) {
  return (
    <Link href={`/decisions/${encodeURIComponent(cell.id)}`}>
      <Card hover className="space-y-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0 flex-1">
            <p className="text-[11px] uppercase tracking-[0.18em] text-text-muted">
              {decisionTypeLabel(cell.decisionType)}
            </p>
            <h2 className="mt-2 text-lg font-semibold text-text-primary">
              {cell.title}
            </h2>
            <p className="mt-2 line-clamp-2 text-sm text-text-secondary">
              {cell.statement}
            </p>
          </div>
          <div className="flex flex-col items-end gap-2 text-right">
            <span className="border border-border bg-bg-secondary px-2 py-1 text-[11px] uppercase tracking-[0.14em] text-text-secondary">
              {recommendationLabel(cell.recommendation.state)}
            </span>
            {cell.automationEnabled ? (
              <span className="border border-accent/30 bg-accent/10 px-2 py-1 text-[11px] uppercase tracking-[0.14em] text-accent">
                automation on
              </span>
            ) : null}
          </div>
        </div>

        <div className="grid gap-3 sm:grid-cols-3">
          <div>
            <div className="text-[11px] uppercase tracking-[0.18em] text-text-muted">
              confidence
            </div>
            <div className="mt-1 text-lg font-semibold text-text-primary">
              {formatPercentFromBps(cell.recommendation.confidenceBps, 0)}
            </div>
          </div>
          <div>
            <div className="text-[11px] uppercase tracking-[0.18em] text-text-muted">
              live nodes
            </div>
            <div className="mt-1 text-lg font-semibold text-text-primary">
              {cell.recommendation.liveNodes}/{cell.recommendation.totalNodes}
            </div>
          </div>
          <div>
            <div className="text-[11px] uppercase tracking-[0.18em] text-text-muted">
              linked markets
            </div>
            <div className="mt-1 text-lg font-semibold text-text-primary">
              {cell.linkedMarketRefs.length}
            </div>
          </div>
        </div>

        <p className="text-sm text-text-secondary">
          {cell.recommendation.whyChanged}
        </p>
      </Card>
    </Link>
  );
}
