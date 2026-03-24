"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { Header, BottomNav } from "@/components/layout";
import { WorldDeskHero } from "@/components/home/WorldDeskHero";
import { MarketRow } from "@/components/market";
import { SignalChart } from "@/components/market/FeaturedBanner";
import { useMarkets } from "@/hooks";
import type { HomeLiveFeed } from "@/lib/server/homeLive";
import type { Market, PaginatedResponse } from "@/types";

const TICKER_TEXT =
  "RELAY44 PROTOCOL LIVE ___ AGENT 004 PREDICTING ___ BTC $92,000 [72%] ___ AGI 2026 [14%] ___ MARS LANDING 2029 [61%] ___ SUPERCONDUCTOR LK-99 [08%] ___ NETWORK LATENCY 4MS ___ SETTLEMENT: BASE L2 ___ ";

interface HomePageClientProps {
  initialMarkets?: PaginatedResponse<Market> | null;
  initialLiveFeed: HomeLiveFeed;
}

export default function HomePageClient({
  initialMarkets,
  initialLiveFeed,
}: HomePageClientProps) {
  const [liveFeed, setLiveFeed] = useState(initialLiveFeed);

  const { data: marketsData, isLoading } = useMarkets(
    {
      sort: "volume",
      limit: 12,
    },
    {
      initialData: initialMarkets || undefined,
    },
  );

  const markets = marketsData?.data || [];

  useEffect(() => {
    setLiveFeed(initialLiveFeed);
  }, [initialLiveFeed]);

  useEffect(() => {
    const refresh = async () => {
      try {
        const response = await fetch("/api/home/live", { cache: "no-store" });
        if (!response.ok) {
          return;
        }

        const payload = (await response.json()) as HomeLiveFeed;
        setLiveFeed(payload);
      } catch {
        // Keep the last successful payload.
      }
    };

    const interval = window.setInterval(() => {
      void refresh();
    }, 5 * 60_000);

    return () => {
      window.clearInterval(interval);
    };
  }, []);

  return (
    <div className="min-h-screen relative overflow-x-hidden">
      <Header />

      <div
        className="fixed hero-glyph leading-none"
        style={{ right: "calc(-7rem + 40px)", top: "calc(3rem - 130px)" }}
        aria-hidden
      >
        <span className="block">&gt;&gt;&gt;</span>
        <span className="block">nm</span>
      </div>

      <div
        className="fixed left-5 top-1/2 -translate-y-1/2 writing-mode-vertical text-[11px] uppercase tracking-[0.3em] text-text-muted font-mono hidden lg:block"
        aria-hidden
      >
        STATUS: &gt; ACTIVE / SN: • OPERATION WEB-04.01 • FWD
      </div>

      <div
        className="fixed right-5 top-1/2 -translate-y-1/2 writing-mode-vertical text-[11px] uppercase tracking-[0.3em] text-text-muted font-mono hidden lg:block"
        aria-hidden
      >
        SYSTEM_STATUS_OK
      </div>

      <main className="container-app relative z-10 mb-12 pt-20">
        <WorldDeskHero slides={liveFeed.news} />

        <section className="border-b border-border py-6">
          <SignalChart initialSignal={liveFeed.signal} />
        </section>

        <section className="border-b border-border py-6">
          <div className="grid gap-4 lg:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)_minmax(0,1fr)]">
            <div className="border border-border bg-bg-primary p-5 brutal-shadow">
              <p className="text-[11px] uppercase tracking-[0.18em] text-accent">
                Launch primer
              </p>
              <h2 className="mt-3 text-2xl font-semibold uppercase tracking-[-0.03em] text-text-primary">
                Know the market rules before you trade.
              </h2>
              <p className="mt-3 max-w-2xl text-sm leading-6 text-text-secondary">
                relay44 is live market infrastructure on Base. Browse,
                inspect resolution logic, and check risk disclosures before you
                connect, trade, or publish a new market.
              </p>
              <div className="mt-5 flex flex-wrap gap-3">
                <Link
                  href="/how-it-works"
                  className="inline-flex h-10 items-center border border-accent px-4 text-sm uppercase tracking-[0.12em] text-accent transition-colors hover:bg-accent/10"
                >
                  How it works
                </Link>
                <Link
                  href="/legal/disclaimer"
                  className="inline-flex h-10 items-center border border-border px-4 text-sm uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
                >
                  Risk disclosure
                </Link>
              </div>
            </div>

            <div className="border border-border bg-bg-primary p-5 brutal-shadow">
              <p className="text-[11px] uppercase tracking-[0.18em] text-text-muted">
                Live now
              </p>
              <ul className="mt-4 space-y-3 text-sm leading-6 text-text-secondary">
                <li>Browse live markets, prices, and order books.</li>
                <li>Inspect portfolio and wallet state after sign-in.</li>
                <li>
                  Draft markets from live news and publish when write rails are
                  available.
                </li>
              </ul>
            </div>

            <div className="border border-border bg-bg-primary p-5 brutal-shadow">
              <p className="text-[11px] uppercase tracking-[0.18em] text-text-muted">
                Before you publish
              </p>
              <ul className="mt-4 space-y-3 text-sm leading-6 text-text-secondary">
                <li>Use one objective yes or no outcome.</li>
                <li>Name a source that can resolve the market cleanly.</li>
                <li>
                  Set a deadline that matches the question and settlement
                  window.
                </li>
              </ul>
            </div>
          </div>
        </section>

        <section className="py-8 pb-16">
          {isLoading ? (
            <div className="space-y-0">
              {Array.from({ length: 6 }).map((_, i) => (
                <div
                  key={i}
                  className="flex items-center gap-6 py-5 border-b border-border animate-pulse"
                >
                  <div className="w-12 h-4 bg-bg-secondary hidden sm:block" />
                  <div className="flex-1 h-5 bg-bg-secondary" />
                  <div className="w-16 h-4 bg-bg-secondary hidden md:block" />
                  <div className="w-20 h-4 bg-bg-secondary hidden md:block" />
                  <div className="w-16 h-8 bg-bg-secondary" />
                </div>
              ))}
            </div>
          ) : markets.length > 0 ? (
            markets.map((market, i) => (
              <MarketRow key={market.id} market={market} index={i} />
            ))
          ) : (
            <div className="py-12 text-center text-text-muted text-sm uppercase tracking-[0.16em]">
              No active markets
            </div>
          )}
        </section>
      </main>

      <div className="fixed bottom-0 left-0 right-0 bg-bg-primary border-t border-border overflow-hidden z-30 md:z-40">
        <div className="py-2.5 whitespace-nowrap overflow-hidden">
          <span className="animate-marquee text-[11px] text-accent uppercase tracking-[0.16em] font-mono">
            {TICKER_TEXT}
            {TICKER_TEXT}
          </span>
        </div>
      </div>

      <BottomNav />
    </div>
  );
}
