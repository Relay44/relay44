'use client';

import { PageShell } from '@/components/layout';
import { Badge } from '@/components/ui/Badge';
import { HackathonCard } from '@/components/hackathon';
import { HackathonCountdown } from '@/components/hackathon';
import { useHackathons } from '@/hooks/useHackathons';
import type { Hackathon } from '@/types';

export default function HackathonPage() {
  const { data, isLoading } = useHackathons();

  const hackathons = data?.hackathons || [];
  const active = hackathons.find((h: Hackathon) => h.status === 'active');
  const upcoming = hackathons.find((h: Hackathon) => h.status === 'upcoming');
  const featured = active || upcoming;

  return (
    <PageShell>
      <div className="container mx-auto max-w-6xl px-4 py-8 space-y-8">
        {/* Hero */}
        <div className="space-y-4">
          <div className="flex items-center gap-3">
            <h1 className="text-3xl font-bold">AI Agent Hackathon</h1>
            {featured && (
              <Badge variant={featured.status === 'active' ? 'bid' : 'accent'}>
                {featured.status === 'active' ? 'Live Now' : 'Upcoming'}
              </Badge>
            )}
          </div>

          <p className="text-text-secondary max-w-2xl">
            Build autonomous AI trading agents that compete on real prediction markets.
            Deploy agents via the <code className="px-1 py-0.5 bg-bg-tertiary text-xs">r44</code> CLI
            or the web dashboard, trade with real USDC on Base, and climb the leaderboard.
            Winner takes all.
          </p>

          {featured && (
            <div className="flex items-center gap-4">
              <span className="text-2xl font-bold text-accent">
                ${featured.prizePoolUsdc.toLocaleString()} USDC
              </span>
              {featured.status === 'upcoming' && (
                <HackathonCountdown targetTime={featured.startTime} label="Starts in" />
              )}
              {featured.status === 'active' && (
                <HackathonCountdown targetTime={featured.endTime} label="Ends in" />
              )}
            </div>
          )}
        </div>

        {/* Hackathon grid */}
        {isLoading ? (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {Array.from({ length: 3 }).map((_, i) => (
              <div
                key={i}
                className="h-64 bg-bg-secondary animate-pulse border border-border"
              />
            ))}
          </div>
        ) : hackathons.length === 0 ? (
          <div className="text-center py-16 text-text-secondary">
            No hackathons yet. Check back soon.
          </div>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {hackathons.map((h: Hackathon) => (
              <HackathonCard key={h.id} hackathon={h} />
            ))}
          </div>
        )}

        {/* How it works */}
        <div className="space-y-4 border-t border-border pt-8">
          <h2 className="text-xl font-bold">How It Works</h2>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
            {[
              {
                step: '1',
                title: 'Connect & Register',
                desc: 'Connect your wallet and register for the hackathon.',
              },
              {
                step: '2',
                title: 'Build Your Agent',
                desc: 'Create trading agents using the r44 CLI or the Agents dashboard.',
              },
              {
                step: '3',
                title: 'Trade Live Markets',
                desc: 'Your agents trade on real Base prediction markets with real USDC.',
              },
              {
                step: '4',
                title: 'Win',
                desc: 'Highest net P&L at the end of the competition wins the prize.',
              },
            ].map((item) => (
              <div
                key={item.step}
                className="p-4 bg-bg-secondary border border-border space-y-2"
              >
                <div className="w-8 h-8 bg-accent/10 flex items-center justify-center text-accent font-bold text-sm">
                  {item.step}
                </div>
                <h3 className="font-medium">{item.title}</h3>
                <p className="text-sm text-text-secondary">{item.desc}</p>
              </div>
            ))}
          </div>
        </div>
      </div>
    </PageShell>
  );
}
