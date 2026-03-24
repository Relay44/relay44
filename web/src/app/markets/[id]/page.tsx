"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useBaseWallet } from "@/hooks/useBaseWallet";
import { PageShell } from "@/components/layout";
import { ReadOnlyNotice } from "@/components/runtime/ReadOnlyNotice";
import { Button, LoadingScreen, useToast } from "@/components/ui";
import { MarketHeader, MarketStats, MarketInfo } from "@/components/market";
import {
  ExternalOrderForm,
  OrderForm,
  OrderBookDisplay,
  OrderList,
} from "@/components/order";
import { ShareCastButton } from "@/components/farcaster";
import {
  useClaimWinnings,
  useDecisionCells,
  useMarket,
  usePositions,
  useRuntimeMode,
  useSessionState,
} from "@/hooks";
import { SITE_URL } from "@/lib/seo";

export default function MarketDetailPage() {
  const params = useParams();
  const marketId = decodeURIComponent(params.id as string);
  const baseWallet = useBaseWallet();
  const walletConnected = baseWallet.isConnected;
  const { addToast } = useToast();
  const { readOnly } = useRuntimeMode();
  const { hasSession, sessionRestored } = useSessionState();
  const claimWinnings = useClaimWinnings();

  const { data: market, isLoading, error } = useMarket(marketId);
  const { data: positionsData } = usePositions();
  const { data: decisionCellsData } = useDecisionCells({
    limit: 50,
    enabled: walletConnected && hasSession && sessionRestored,
  });

  const claimable =
    positionsData?.data.find((entry) => entry.marketId === marketId)
      ?.claimable || 0;
  const relatedDecisionCells = (decisionCellsData?.data || []).filter((cell) =>
    cell.linkedMarketRefs.includes(marketId)
  );

  const handleClaim = async () => {
    try {
      const result = await claimWinnings.mutateAsync(marketId);
      addToast(`Claim submitted onchain: ${result.txSignature}`, "success");
    } catch (claimError) {
      const message =
        claimError instanceof Error ? claimError.message : "Claim failed";
      addToast(message, "error");
    }
  };

  if (isLoading) {
    return (
      <PageShell>
        <LoadingScreen />
      </PageShell>
    );
  }

  if (error || !market) {
    return (
      <PageShell>
        <div className="text-center py-12">
          <h2 className="text-xl font-semibold mb-2">Market not found</h2>
          <Link href="/markets" className="text-accent hover:text-accent-hover">
            Back to Markets
          </Link>
        </div>
      </PageShell>
    );
  }

  return (
    <PageShell>
      <Link
        href="/markets"
        className="inline-flex items-center gap-2 text-text-secondary hover:text-text-primary mb-4"
      >
        <ChevronLeftIcon className="w-5 h-5" />
        Back to Markets
      </Link>

      <div className="flex items-center justify-between mb-2">
        <MarketHeader market={market} />
        <ShareCastButton
          text={`${market.question}\n\nYES ${Math.round(market.yesPrice * 100)}% | NO ${Math.round(market.noPrice * 100)}%`}
          embedUrl={`${SITE_URL}/markets/${encodeURIComponent(market.id)}`}
        />
      </div>
      <MarketStats market={market} />

      {walletConnected && !market.isExternal && claimable > 0 ? (
        <div className="mb-6 border border-border p-4">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <div className="text-sm font-medium text-text-primary">
                Claimable winnings
              </div>
              <div className="mt-1 text-sm text-text-secondary">
                ${claimable.toFixed(2)} is currently claimable for this market.
              </div>
            </div>
            <Button
              type="button"
              onClick={() => void handleClaim()}
              loading={claimWinnings.isPending}
              disabled={readOnly || claimWinnings.isPending}
            >
              Claim on Base
            </Button>
          </div>
        </div>
      ) : null}

      {relatedDecisionCells.length > 0 ? (
        <div className="mb-6 border border-border p-4">
          <div className="flex flex-col gap-4">
            <div>
              <div className="text-sm font-medium text-text-primary">
                Used in decision cells
              </div>
              <div className="mt-1 text-sm text-text-secondary">
                This market is linked into private decision systems that use its
                probability as a live node input.
              </div>
            </div>
            <div className="grid gap-3">
              {relatedDecisionCells.map((cell) => (
                <Link
                  key={cell.id}
                  href={`/decisions/${encodeURIComponent(cell.id)}`}
                  className="border border-border bg-bg-secondary px-4 py-3 transition-colors hover:border-border-hover hover:bg-bg-hover"
                >
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div>
                      <div className="text-[11px] uppercase tracking-[0.18em] text-text-secondary">
                        {cell.decisionType}
                      </div>
                      <div className="mt-1 font-medium text-text-primary">
                        {cell.title}
                      </div>
                    </div>
                    <div className="text-right text-sm">
                      <div className="text-text-secondary">Recommendation</div>
                      <div className="font-medium text-text-primary">
                        {cell.recommendation.state.replace(/_/g, " ")}
                      </div>
                    </div>
                  </div>
                </Link>
              ))}
            </div>
          </div>
        </div>
      ) : null}

      <div className="grid lg:grid-cols-2 gap-6 mb-6">
        {market.status === "active" ? (
          readOnly ? (
            <ReadOnlyNotice
              title="Trading is disabled in this preview"
              body="Market data stays live, but order placement and execution are turned off in this environment."
              actionHref="/markets"
              actionLabel="Browse more markets"
              className="h-full"
            />
          ) : !market.executionUsers ? (
            <div className="card flex items-center justify-center py-12">
              <p className="text-text-secondary">
                Trading is unavailable for this market under the current
                provider policy.
              </p>
            </div>
          ) : walletConnected ? (
            market.isExternal ? (
              <ExternalOrderForm market={market} />
            ) : (
              <OrderForm market={market} />
            )
          ) : (
            <div className="card flex flex-col items-center justify-center gap-3 py-12 text-center">
              <p className="text-lg font-medium text-text-primary">
                Connect wallet to trade
              </p>
              <p className="max-w-md text-sm text-text-secondary">
                Connect your Base wallet from the header, approve the sign-in
                prompt, and return here to place orders against the live book.
              </p>
              <div className="flex flex-wrap justify-center gap-3">
                <Link
                  href="/how-it-works"
                  className="inline-flex h-10 items-center border border-accent px-4 text-sm uppercase tracking-[0.12em] text-accent transition-colors hover:bg-accent/10"
                >
                  How it works
                </Link>
                <Link
                  href="/wallet"
                  className="inline-flex h-10 items-center border border-border px-4 text-sm uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
                >
                  Wallet setup
                </Link>
              </div>
            </div>
          )
        ) : (
          <div className="card flex items-center justify-center py-12">
            <p className="text-text-secondary">Trading is closed</p>
          </div>
        )}

        <OrderBookDisplay marketId={marketId} />
      </div>

      {walletConnected && !market.isExternal && (
        <div className="mb-6">
          <h3 className="font-semibold mb-4">Your Orders</h3>
          <OrderList marketId={marketId} />
        </div>
      )}

      <MarketInfo market={market} />
    </PageShell>
  );
}

function ChevronLeftIcon({ className }: { className?: string }) {
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
        d="M15 19l-7-7 7-7"
      />
    </svg>
  );
}
