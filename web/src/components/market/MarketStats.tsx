import { Card } from "@/components/ui";
import {
  getBootstrapReasonLabel,
  getBootstrapStatusLabel,
} from "@/lib/bootstrap";
import {
  formatCurrency,
  formatDate,
  formatPercent,
  truncateAddress,
} from "@/lib/utils";
import type { Market } from "@/types";

export interface MarketStatsProps {
  market: Market;
}

export function MarketStats({ market }: MarketStatsProps) {
  const bootstrapLabel = getBootstrapStatusLabel(market);
  const bootstrapReason =
    getBootstrapReasonLabel(market.bootstrapPauseReason) ||
    market.bootstrapLastError ||
    null;
  const lastReconciled = market.bootstrapLastReconciledAt
    ? formatDate(market.bootstrapLastReconciledAt)
    : "Not yet";

  return (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-3 mb-6">
      <Card>
        <div className="text-text-muted text-xs mb-1">Yes Price</div>
        <div className="text-xl font-semibold text-accent">
          {formatPercent(market.yesPrice)}
        </div>
      </Card>
      <Card>
        <div className="text-text-muted text-xs mb-1">No Price</div>
        <div className="text-xl font-semibold text-text-primary">
          {formatPercent(market.noPrice)}
        </div>
      </Card>
      <Card>
        <div className="text-text-secondary text-xs mb-1">24h Volume</div>
        <div className="text-lg font-semibold">
          {formatCurrency(market.volume24h)}
        </div>
      </Card>
      <Card>
        <div className="text-text-secondary text-xs mb-1">Total Volume</div>
        <div className="text-lg font-semibold">
          {formatCurrency(market.totalVolume)}
        </div>
      </Card>
      {market.liquidityMode === "bootstrap_hybrid" ? (
        <>
          <Card>
            <div className="text-text-secondary text-xs mb-1">
              Bootstrap Status
            </div>
            <div className="text-lg font-semibold capitalize">{bootstrapLabel}</div>
          </Card>
          <Card>
            <div className="text-text-secondary text-xs mb-1">
              Bootstrap Seed
            </div>
            <div className="text-lg font-semibold">
              {formatCurrency(market.bootstrapSeedUsdc || 0)}
            </div>
          </Card>
          <Card>
            <div className="text-text-secondary text-xs mb-1">Active Depth</div>
            <div className="text-lg font-semibold">
              {formatCurrency(market.bootstrapReservedUsdc || 0)}
            </div>
          </Card>
          <Card>
            <div className="text-text-secondary text-xs mb-1">Last Reconcile</div>
            <div className="text-sm font-medium text-text-primary">
              {lastReconciled}
            </div>
          </Card>
          {bootstrapReason ? (
            <Card>
              <div className="text-text-secondary text-xs mb-1">Health</div>
              <div className="text-sm font-medium text-text-primary">
                {bootstrapReason}
              </div>
            </Card>
          ) : null}
        </>
      ) : null}
    </div>
  );
}

export function MarketInfo({ market }: MarketStatsProps) {
  const bootstrapLabel = getBootstrapStatusLabel(market);
  const bootstrapReason =
    getBootstrapReasonLabel(market.bootstrapPauseReason) ||
    market.bootstrapLastError ||
    null;

  return (
    <Card>
      <h3 className="font-semibold mb-4">Market Info</h3>
      <div className="space-y-3 text-sm">
        <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
          <span className="text-text-secondary">Resolution Source</span>
          <span className="break-all font-mono text-text-muted sm:text-right">
            {truncateAddress(market.oracle, 6)}
          </span>
        </div>
        <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
          <span className="text-text-secondary">Trading Ends</span>
          <span>{formatDate(market.tradingEnd)}</span>
        </div>
        <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
          <span className="text-text-secondary">Resolution Deadline</span>
          <span>{formatDate(market.resolutionDeadline)}</span>
        </div>
        <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
          <span className="text-text-secondary">Created</span>
          <span>{formatDate(market.createdAt)}</span>
        </div>
        <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
          <span className="text-text-secondary">Fee</span>
          <span>{(market.feeBps / 100).toFixed(2)}%</span>
        </div>
        {market.liquidityMode === "bootstrap_hybrid" ? (
          <>
            <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
              <span className="text-text-secondary">Liquidity Mode</span>
              <span>Bootstrap hybrid</span>
            </div>
            <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
              <span className="text-text-secondary">Bootstrap Seed</span>
              <span>{formatCurrency(market.bootstrapSeedUsdc || 0)}</span>
            </div>
            <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
              <span className="text-text-secondary">Bootstrap Status</span>
              <span className="capitalize">{bootstrapLabel}</span>
            </div>
            <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
              <span className="text-text-secondary">Bootstrap Preset</span>
              <span>{market.bootstrapPreset || "balanced"}</span>
            </div>
            <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
              <span className="text-text-secondary">Active Depth</span>
              <span>{formatCurrency(market.bootstrapReservedUsdc || 0)}</span>
            </div>
            <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
              <span className="text-text-secondary">Vault Available</span>
              <span>{formatCurrency(market.bootstrapAvailableUsdc || 0)}</span>
            </div>
            <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
              <span className="text-text-secondary">Active Slots</span>
              <span>{market.bootstrapActiveSlots || 0}</span>
            </div>
            <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
              <span className="text-text-secondary">Organic Depth Ratio</span>
              <span>
                {market.bootstrapOrganicDepthRatio != null
                  ? `${market.bootstrapOrganicDepthRatio.toFixed(2)}x`
                  : "0.00x"}
              </span>
            </div>
            <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
              <span className="text-text-secondary">Last Reconcile</span>
              <span>
                {market.bootstrapLastReconciledAt
                  ? formatDate(market.bootstrapLastReconciledAt)
                  : "Not yet"}
              </span>
            </div>
            {bootstrapReason ? (
              <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
                <span className="text-text-secondary">Bootstrap Note</span>
                <span className="max-w-[24rem] text-text-primary sm:text-right">
                  {bootstrapReason}
                </span>
              </div>
            ) : null}
          </>
        ) : null}
      </div>
    </Card>
  );
}
