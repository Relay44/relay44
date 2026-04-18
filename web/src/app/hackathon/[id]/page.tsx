'use client';

import { useParams } from 'next/navigation';
import { PageShell } from '@/components/layout';
import { Badge } from '@/components/ui/Badge';
import {
  HackathonCountdown,
  HackathonRegistration,
  HackathonLeaderboard,
  HackathonPnlChart,
} from '@/components/hackathon';
import { useHackathon } from '@/hooks/useHackathons';
import { STATUS_VARIANT } from '@/lib/hackathon';

export default function HackathonDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { data: hackathon, isLoading } = useHackathon(id);

  if (isLoading) {
    return (
      <PageShell>
        <div className="py-8">
          <div className="mx-auto max-w-3xl">
            <div className="h-64 bg-bg-secondary animate-pulse border border-border" />
          </div>
        </div>
      </PageShell>
    );
  }

  if (!hackathon) {
    return (
      <PageShell>
        <div className="py-8">
          <div className="mx-auto max-w-3xl text-center text-text-secondary">
            Hackathon not found.
          </div>
        </div>
      </PageShell>
    );
  }

  const start = new Date(hackathon.startTime);
  const end = new Date(hackathon.endTime);
  const now = Date.now();
  const isUpcoming = now < start.getTime();
  const isActive = now >= start.getTime() && now < end.getTime();
  const rules = hackathon.rulesJson as Record<string, unknown>;

  return (
    <PageShell>
      <div className="py-8">
        <div className="mx-auto max-w-3xl space-y-6">
        {/* Header */}
        <div className="space-y-3">
          <div className="flex items-center gap-3 flex-wrap">
            <h1 className="text-2xl font-bold">{hackathon.name}</h1>
            <Badge variant={STATUS_VARIANT[hackathon.status] || 'default'}>
              {hackathon.status}
            </Badge>
            <Badge variant="accent">
              ${hackathon.prizePoolUsdc.toLocaleString()} USDC
            </Badge>
          </div>

          {hackathon.description && (
            <p className="text-text-secondary">{hackathon.description}</p>
          )}

          <div className="flex items-center gap-4 text-sm text-text-muted">
            <span>
              {start.toLocaleDateString()} — {end.toLocaleDateString()}
            </span>
            <span>{hackathon.participantCount} participants</span>
            <span>{hackathon.agentCount} agents</span>
          </div>

          {isUpcoming && (
            <HackathonCountdown targetTime={hackathon.startTime} label="Starts in" />
          )}
          {isActive && (
            <HackathonCountdown targetTime={hackathon.endTime} label="Ends in" />
          )}
        </div>

        {/* Rules */}
        {rules && Object.keys(rules).length > 0 && (
          <div className="p-4 bg-bg-secondary border border-border space-y-2">
            <h2 className="font-medium">Rules</h2>
            <ul className="text-sm text-text-secondary space-y-1 list-disc list-inside">
              {typeof rules.rules === 'string' ? (
                <li>{rules.rules as string}</li>
              ) : Array.isArray(rules.rules) ? (
                (rules.rules as string[]).map((rule: string, i: number) => (
                  <li key={i}>{rule}</li>
                ))
              ) : (
                Object.entries(rules).map(([key, val]) => (
                  <li key={key}>
                    <strong>{key}:</strong> {String(val)}
                  </li>
                ))
              )}
            </ul>
          </div>
        )}

        {/* Registration */}
        <HackathonRegistration hackathon={hackathon} />

        {/* Leaderboard */}
        <HackathonLeaderboard hackathonId={hackathon.id} />

        {/* PnL Chart */}
        <HackathonPnlChart hackathonId={hackathon.id} />
        </div>
      </div>
    </PageShell>
  );
}
