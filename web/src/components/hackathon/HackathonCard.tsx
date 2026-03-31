'use client';

import Link from 'next/link';
import { Card, CardHeader, CardTitle, CardContent, CardFooter } from '@/components/ui/Card';
import { Badge } from '@/components/ui/Badge';
import { HackathonCountdown } from './HackathonCountdown';
import type { Hackathon } from '@/types';

const STATUS_VARIANT: Record<string, 'bid' | 'accent' | 'default' | 'ask'> = {
  upcoming: 'accent',
  active: 'bid',
  completed: 'default',
  cancelled: 'ask',
};

interface HackathonCardProps {
  hackathon: Hackathon;
}

export function HackathonCard({ hackathon }: HackathonCardProps) {
  const start = new Date(hackathon.startTime);
  const end = new Date(hackathon.endTime);
  const now = Date.now();
  const isUpcoming = now < start.getTime();
  const isActive = now >= start.getTime() && now < end.getTime();

  return (
    <Link href={`/hackathon/${hackathon.id}`}>
      <Card hover className="h-full transition-all duration-fast hover:border-accent hover:shadow-md hover:-translate-y-0.5">
        <CardHeader>
          <div className="flex items-center justify-between gap-2">
            <CardTitle className="text-lg truncate">{hackathon.name}</CardTitle>
            <Badge variant={STATUS_VARIANT[hackathon.status] || 'default'}>
              {hackathon.status}
            </Badge>
          </div>
        </CardHeader>

        <CardContent className="space-y-3">
          {hackathon.description && (
            <p className="text-sm text-text-secondary line-clamp-2">
              {hackathon.description}
            </p>
          )}

          <div className="flex items-center justify-between text-sm">
            <span className="text-text-secondary">Prize</span>
            <span className="font-medium text-accent">
              ${hackathon.prizePoolUsdc.toLocaleString()} USDC
            </span>
          </div>

          <div className="flex items-center justify-between text-sm">
            <span className="text-text-secondary">Participants</span>
            <span className="font-medium">{hackathon.participantCount}</span>
          </div>

          <div className="flex items-center justify-between text-sm">
            <span className="text-text-secondary">Agents</span>
            <span className="font-medium">{hackathon.agentCount}</span>
          </div>

          <div className="text-xs text-text-muted">
            {start.toLocaleDateString()} — {end.toLocaleDateString()}
          </div>
        </CardContent>

        <CardFooter>
          {isUpcoming && (
            <HackathonCountdown targetTime={hackathon.startTime} label="Starts in" />
          )}
          {isActive && (
            <HackathonCountdown targetTime={hackathon.endTime} label="Ends in" />
          )}
          {hackathon.status === 'completed' && (
            <span className="text-sm text-text-muted">Completed</span>
          )}
        </CardFooter>
      </Card>
    </Link>
  );
}
