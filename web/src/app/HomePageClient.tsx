"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { Header, BottomNav } from "@/components/layout";
import { HeroTicket } from "@/components/home/HeroTicket";
import { MarketRow, FeaturedSlider } from "@/components/market";
import { useMarkets } from "@/hooks";
import type { HomeLiveFeed } from "@/lib/server/homeLive";
import type { Market, PaginatedResponse } from "@/types";

interface HomePageClientProps {
  initialMarkets?: PaginatedResponse<Market> | null;
  initialLiveFeed: HomeLiveFeed;
}

const AGENT_LOGS = [
  { time: "14:02:11", text: 'Agent <span class="text-text-primary">Osprey-7</span> executing arb strategy.' },
  { time: "14:02:08", text: '<span class="text-text-primary">BOUGHT 4,500 YES</span> @ 68\u00A2 [GPT-5]' },
  { time: "14:01:45", text: '<span class="text-text-primary">Mantis-V</span> detected sentiment shift.' },
  { time: "14:01:22", text: '<span class="text-text-primary">Kestrel-3</span> liquidated short position.' },
  { time: "14:00:58", text: 'Signal confidence above threshold. <span class="text-text-primary">AUTO-BID</span> triggered.' },
  { time: "14:00:31", text: '<span class="text-text-primary">Osprey-7</span> scanning new feeds.' },
];

function AgentPanel() {
  return (
    <aside className="hidden lg:flex w-[300px] shrink-0 flex-col border-r border-border">
      <div className="flex items-center justify-between px-4 py-3 border-b border-border font-mono text-[0.75rem]">
        <span className="text-text-muted uppercase tracking-wider">Swarm Telemetry</span>
        <span className="text-text-primary">LIVE</span>
      </div>
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {AGENT_LOGS.map((log, i) => (
          <div key={i} className="font-mono text-[0.7rem] text-text-muted border-l-2 border-border pl-2">
            <span className="text-text-muted/50">{log.time}</span>
            <br />
            <span dangerouslySetInnerHTML={{ __html: log.text }} />
          </div>
        ))}
      </div>
    </aside>
  );
}


function MarketTable({ markets, isLoading }: { markets: Market[]; isLoading: boolean }) {
  if (isLoading) {
    return (
      <div className="p-6 sm:p-8 space-y-0">
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
    );
  }

  if (markets.length === 0) {
    return (
      <div className="py-16 text-center text-text-muted text-sm font-mono uppercase tracking-wider">
        No active markets
      </div>
    );
  }

  return (
    <div className="p-4 sm:p-6 md:p-8">
      <table className="w-full font-mono border-collapse">
        <thead>
          <tr>
            <th className="text-left py-4 px-4 border-b border-border text-[0.7rem] text-text-muted uppercase">Active Markets</th>
            <th className="text-left py-4 px-4 border-b border-border text-[0.7rem] text-text-muted uppercase hidden md:table-cell">Volume</th>
            <th className="text-left py-4 px-4 border-b border-border text-[0.7rem] text-text-muted uppercase hidden md:table-cell">Ends</th>
            <th className="text-left py-4 px-4 border-b border-border text-[0.7rem] text-text-muted uppercase">Pricing</th>
          </tr>
        </thead>
        <tbody>
          {markets.map((market) => {
            const yesPrice = market.yesPrice != null ? `${Math.round(market.yesPrice * 100)}\u00A2` : '—';
            const noPrice = market.noPrice != null ? `${Math.round(market.noPrice * 100)}\u00A2` : '—';
            const endDate = market.tradingEnd
              ? new Date(market.tradingEnd).toISOString().slice(0, 10).replace(/-/g, '.')
              : '—';

            return (
              <tr
                key={market.id}
                className="border-b border-border transition-colors hover:bg-bg-hover group"
              >
                <td className="py-5 px-4" style={{ fontFamily: 'var(--font-display)', fontWeight: 700, fontSize: '1.1rem' }}>
                  <Link href={`/markets/${market.id}`} className="flex items-center gap-2">
                    <span className="inline-block w-1.5 h-1.5 rounded-full bg-green-500 shrink-0" />
                    <span className="group-hover:underline">{market.question}</span>
                  </Link>
                </td>
                <td className="py-5 px-4 text-[0.85rem] hidden md:table-cell">
                  ${market.volume != null ? (market.volume / 1_000_000 >= 1 ? `${(market.volume / 1_000_000).toFixed(1)}M` : `${(market.volume / 1_000).toFixed(0)}K`) : '—'}
                </td>
                <td className="py-5 px-4 text-[0.85rem] hidden md:table-cell">{endDate}</td>
                <td className="py-5 px-4">
                  <div className="flex gap-2">
                    <Link
                      href={`/markets/${market.id}`}
                      className="flex items-center justify-between gap-2 border border-border px-3 py-2 w-20 text-[0.8rem] font-bold transition-colors hover:bg-text-primary hover:text-text-inverse"
                    >
                      <span>YES</span>
                      <span>{yesPrice}</span>
                    </Link>
                    <Link
                      href={`/markets/${market.id}`}
                      className="flex items-center justify-between gap-2 border border-border px-3 py-2 w-20 text-[0.8rem] font-bold transition-colors hover:bg-text-primary hover:text-text-inverse"
                    >
                      <span>NO</span>
                      <span>{noPrice}</span>
                    </Link>
                  </div>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
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
        if (!response.ok) return;
        const payload = (await response.json()) as HomeLiveFeed;
        setLiveFeed(payload);
      } catch {}
    };

    const interval = window.setInterval(() => {
      void refresh();
    }, 5 * 60_000);

    return () => {
      window.clearInterval(interval);
    };
  }, []);

  return (
    <div className="h-screen flex flex-col overflow-hidden">
      <Header />

      <div className="flex flex-1 overflow-hidden pt-[73px] sm:pt-[81px]">
        <AgentPanel />

        <main className="flex-1 overflow-y-auto">
          <section className="border-b border-border h-[280px] sm:h-[320px]">
            <HeroTicket />
          </section>

          <section className="py-5 border-b border-border">
            <FeaturedSlider markets={markets.slice(0, 8)} title="Signal Relay" />
          </section>

          <MarketTable markets={markets} isLoading={isLoading} />
        </main>
      </div>

      <BottomNav />
    </div>
  );
}
