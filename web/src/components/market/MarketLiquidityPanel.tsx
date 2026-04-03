import { Card } from "@/components/ui";
import {
  getBootstrapReasonLabel,
  getBootstrapStatusLabel,
} from "@/lib/bootstrap";
import {
  formatCurrency,
  formatNumber,
  formatTimeAgo,
} from "@/lib/utils";
import type { Market } from "@/types";

export interface MarketLiquidityPanelProps {
  market: Market;
}

function Metric({
  label,
  value,
}: {
  label: string;
  value: string;
}) {
  return (
    <div className="border border-border bg-bg-secondary/60 p-3">
      <div className="text-[11px] uppercase tracking-[0.14em] text-text-muted">
        {label}
      </div>
      <div className="mt-1 text-sm font-medium text-text-primary">{value}</div>
    </div>
  );
}

function maybeCurrency(value?: number | null) {
  return value == null ? "—" : formatCurrency(value);
}

function maybeNumber(value?: number | null) {
  return value == null ? "—" : value.toLocaleString();
}

function maybeTimeAgo(value?: string | null) {
  return value ? formatTimeAgo(value) : "Not yet";
}

export function MarketLiquidityPanel({ market }: MarketLiquidityPanelProps) {
  if (market.liquidityMode !== "bootstrap_hybrid") {
    return null;
  }

  const status = getBootstrapStatusLabel(market);
  const reason =
    getBootstrapReasonLabel(market.bootstrapPauseReason) ||
    market.bootstrapLastError ||
    null;
  const score = Math.max(0, Math.round(market.tradabilityScore || 0));

  return (
    <Card className="mb-6">
      <div className="mb-4 flex items-start justify-between gap-3">
        <div>
          <h3 className="font-semibold">Liquidity Panel</h3>
          <p className="mt-1 text-xs text-text-secondary">
            Bootstrap liquidity, mirror freshness, and tradability signals in one
            place.
          </p>
        </div>
        <span className="text-xs uppercase tracking-[0.14em] text-text-muted">
          {status}
        </span>
      </div>

      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
        <Metric label="Bootstrap Seed" value={maybeCurrency(market.bootstrapSeedUsdc)} />
        <Metric label="Reserved Depth" value={maybeCurrency(market.bootstrapReservedUsdc)} />
        <Metric label="Available Vault" value={maybeCurrency(market.bootstrapAvailableUsdc)} />
        <Metric label="Inventory Yes" value={maybeCurrency(market.bootstrapInventoryYesUsdc)} />
        <Metric label="Inventory No" value={maybeCurrency(market.bootstrapInventoryNoUsdc)} />
        <Metric label="Inventory Net" value={maybeCurrency(market.bootstrapInventoryNetUsdc)} />
        <Metric
          label="Mirror Links"
          value={
            market.mirrorLinkCount != null &&
            market.mirrorActiveLinkCount != null
              ? `${maybeNumber(market.mirrorActiveLinkCount)} / ${maybeNumber(market.mirrorLinkCount)}`
              : "—"
          }
        />
        <Metric
          label="Organic Replacement Ratio"
          value={
            market.bootstrapOrganicDepthRatio != null
              ? `${market.bootstrapOrganicDepthRatio.toFixed(2)}x`
              : "—"
          }
        />
        <Metric
          label="Mirror Mirrored"
          value={maybeCurrency(market.mirrorTotalMirroredUsdc)}
        />
        <Metric
          label="Mirror Hedged"
          value={maybeCurrency(market.mirrorTotalHedgedUsdc)}
        />
        <Metric
          label="Mirror Freshness"
          value={maybeTimeAgo(market.mirrorLastMirrorAt)}
        />
        <Metric
          label="Last Hedge"
          value={maybeTimeAgo(market.mirrorLastHedgeAt)}
        />
        <Metric
          label="Mirror Hedge Backlog"
          value={maybeNumber(market.mirrorPendingHedges)}
        />
        <Metric
          label="Mirror Errors"
          value={maybeNumber(market.mirrorLinksWithErrors)}
        />
        <Metric
          label="Hedge Errors"
          value={maybeNumber(market.mirrorHedgeErrors)}
        />
        <Metric
          label="Mirror Exposure"
          value={maybeCurrency(market.mirrorNetExposureUsdc)}
        />
        <Metric label="Tradability Score" value={formatNumber(score)} />
      </div>

      {reason ? (
        <div className="mt-4 text-xs text-text-secondary">
          <span className="uppercase tracking-[0.14em] text-text-muted">
            Bootstrap note:
          </span>{" "}
          {reason}
        </div>
      ) : null}
    </Card>
  );
}
