"use client";

import Link from "next/link";
import { useBaseWallet } from "@/hooks/useBaseWallet";
import { Card } from "@/components/ui";
import { PositionList } from "@/components/position";
import { OrderList } from "@/components/order";
import { DistributionPositions } from "@/components/distribution";
import { useDecisionCells, usePositions, useSessionState, useDistributionPositions, useCloseDistPosition, useClaimDistPayout } from "@/hooks";
import { formatCurrency, formatPnl } from "@/lib/utils";

export default function PortfolioPage() {
  const { isConnected } = useBaseWallet();
  const { hasSession, sessionRestored } = useSessionState();
  const { data: positionsData } = usePositions();
  const { data: distPositions } = useDistributionPositions();
  const closeDistPosition = useCloseDistPosition();
  const claimDistPayout = useClaimDistPayout();
  const { data: decisionCellsData } = useDecisionCells({
    limit: 50,
    enabled: isConnected && hasSession && sessionRestored,
  });
  const positions = positionsData?.data || [];
  const positionMarketIds = new Set(positions.map((position) => position.marketId));
  const impactedDecisionCells = (decisionCellsData?.data || []).filter((cell) =>
    cell.linkedMarketRefs.some((marketId) => positionMarketIds.has(marketId))
  );

  if (!isConnected) {
    return (
      <div className="flex flex-col items-center justify-center min-h-[60vh] text-center">
        <div className="w-16 h-16 bg-bg-secondary  flex items-center justify-center mb-4">
          <WalletIcon className="w-8 h-8 text-text-secondary" />
        </div>
        <h2 className="text-xl font-semibold mb-2">Connect Your Wallet</h2>
        <p className="max-w-lg text-text-secondary">
          Connect your Base wallet from the header, approve the sign-in
          prompt, and then return here to inspect portfolio value, active
          positions, and open orders.
        </p>
        <div className="mt-5 flex flex-wrap justify-center gap-3">
          <Link
            href="/how-it-works"
            className="inline-flex h-10 items-center border border-accent px-4 text-sm uppercase tracking-[0.12em] text-accent transition-colors hover:bg-accent/10"
          >
            How it works
          </Link>
          <Link
            href="/markets"
            className="inline-flex h-10 items-center border border-border px-4 text-sm uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
          >
            Browse markets
          </Link>
        </div>
      </div>
    );
  }

  const totalValue = positions.reduce((sum, p) => {
    return (
      sum + p.yesBalance * p.currentYesPrice + p.noBalance * p.currentNoPrice
    );
  }, 0);

  const totalPnl = positions.reduce((sum, p) => sum + p.unrealizedPnl, 0);
  const realizedPnl = positions.reduce((sum, p) => sum + p.realizedPnl, 0);
  const totalClaimable = positions.reduce((sum, p) => sum + p.claimable, 0);

  const openDistPositions = (distPositions || []).filter((p) => p.status === 'open');
  const resolvedDistPositions = (distPositions || []).filter((p) => p.status !== 'open');
  const distCollateralValue = (distPositions || []).reduce((sum, p) => sum + p.collateral, 0);
  const distPnl = (distPositions || []).reduce((sum, p) => sum + (p.pnl ?? 0), 0);

  return (
    <>
      <h1 className="text-2xl font-bold mb-6">Portfolio</h1>

      <div className="grid grid-cols-2 gap-3 mb-6 sm:gap-4 md:grid-cols-3 xl:grid-cols-5">
        <Card>
          <div className="text-text-secondary text-sm mb-1">Total Value</div>
          <div className="text-xl sm:text-2xl font-semibold">
            {formatCurrency(totalValue)}
          </div>
        </Card>
        <Card>
          <div className="text-text-secondary text-sm mb-1">Unrealized P&L</div>
          <div
            className={`text-xl sm:text-2xl font-semibold ${
              totalPnl >= 0 ? "text-accent" : "text-text-secondary"
            }`}
          >
            {formatPnl(totalPnl)}
          </div>
        </Card>
        <Card>
          <div className="text-text-secondary text-sm mb-1">Realized P&L</div>
          <div
            className={`text-xl sm:text-2xl font-semibold ${
              realizedPnl >= 0 ? "text-accent" : "text-text-secondary"
            }`}
          >
            {formatPnl(realizedPnl)}
          </div>
        </Card>
        <Card>
          <div className="text-text-secondary text-sm mb-1">Positions</div>
          <div className="text-xl sm:text-2xl font-semibold">{positions.length}</div>
        </Card>
        <Card>
          <div className="text-text-secondary text-sm mb-1">Claimable</div>
          <div className="text-xl sm:text-2xl font-semibold">{formatCurrency(totalClaimable)}</div>
        </Card>
      </div>

      <section className="mb-8">
        {impactedDecisionCells.length > 0 ? (
          <div className="mb-8">
            <h2 className="text-lg font-semibold mb-4">Decision Impact</h2>
            <div className="grid gap-4 lg:grid-cols-2">
              {impactedDecisionCells.map((cell) => (
                <Card key={cell.id}>
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0 flex-1">
                      <div className="text-[11px] uppercase tracking-[0.18em] text-text-secondary">
                        {cell.decisionType}
                      </div>
                      <Link
                        href={`/decisions/${encodeURIComponent(cell.id)}`}
                        className="mt-2 block text-lg font-semibold text-text-primary hover:text-accent"
                      >
                        {cell.title}
                      </Link>
                      <p className="mt-2 text-sm text-text-secondary">
                        {cell.recommendation.whyChanged}
                      </p>
                    </div>
                    <div className="text-right text-sm">
                      <div className="text-text-secondary">Recommendation</div>
                      <div className="font-semibold text-text-primary">
                        {cell.recommendation.state.replace(/_/g, " ")}
                      </div>
                    </div>
                  </div>
                </Card>
              ))}
            </div>
          </div>
        ) : null}

        <h2 className="text-lg font-semibold mb-4">Active Positions</h2>
        <PositionList />
      </section>

      <section>
        <h2 className="text-lg font-semibold mb-4">Open Orders</h2>
        <OrderList />
      </section>

      {(distPositions || []).length > 0 && (
        <section className="mt-8">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-semibold">Distribution Positions</h2>
            <div className="flex items-center gap-4 text-sm text-text-secondary">
              <span>Collateral: {formatCurrency(distCollateralValue)}</span>
              <span className={distPnl >= 0 ? 'text-accent' : 'text-text-secondary'}>
                PnL: {formatPnl(distPnl)}
              </span>
            </div>
          </div>
          {openDistPositions.length > 0 && (
            <div className="mb-4">
              <h3 className="text-sm font-medium text-text-secondary mb-2 uppercase tracking-wide">Open</h3>
              <DistributionPositions
                positions={openDistPositions}
                marketResolved={false}
                onClose={(positionId) => closeDistPosition.mutate(positionId)}
                onClaim={() => {}}
              />
            </div>
          )}
          {resolvedDistPositions.length > 0 && (
            <div>
              <h3 className="text-sm font-medium text-text-secondary mb-2 uppercase tracking-wide">Resolved / Claimed</h3>
              <DistributionPositions
                positions={resolvedDistPositions}
                marketResolved={true}
                onClose={() => {}}
                onClaim={(positionId) => claimDistPayout.mutate(positionId)}
              />
            </div>
          )}
        </section>
      )}
    </>
  );
}

function WalletIcon({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
    >
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth={2}
        d="M3 10h18M7 15h1m4 0h1m-7 4h12a3 3 0 003-3V8a3 3 0 00-3-3H6a3 3 0 00-3 3v8a3 3 0 003 3z"
      />
    </svg>
  );
}
