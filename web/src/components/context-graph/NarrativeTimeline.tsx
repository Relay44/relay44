'use client';

import { Card } from '@/components/ui/Card';
import type { NarrativeSnapshot } from './types';
import { RISK_COLORS } from './types';

interface Props {
  timeline: NarrativeSnapshot[];
}

export function NarrativeTimeline({ timeline }: Props) {
  if (timeline.length === 0) return null;

  const maxScore = Math.max(...timeline.map((s) => s.misinfo_score), 1);

  return (
    <Card className="p-4">
      <h3 className="text-sm font-medium text-text-secondary mb-3">
        Misinfo Score Timeline
      </h3>

      <div className="flex items-end gap-1 h-20">
        {timeline.map((snapshot, i) => {
          const height = (snapshot.misinfo_score / maxScore) * 100;
          const riskLevel =
            snapshot.misinfo_score >= 80 ? 'critical' :
            snapshot.misinfo_score >= 60 ? 'high' :
            snapshot.misinfo_score >= 35 ? 'medium' : 'low';
          const color = RISK_COLORS[riskLevel];
          const date = new Date(snapshot.snapshot_at * 1000);

          return (
            <div
              key={snapshot.id || i}
              className="flex-1 relative group cursor-pointer"
              title={`${date.toLocaleString()}: Score ${snapshot.misinfo_score}`}
            >
              <div
                className="w-full rounded-t transition-all"
                style={{
                  height: `${Math.max(height, 4)}%`,
                  backgroundColor: color,
                  opacity: 0.7,
                }}
              />
              <div className="hidden group-hover:block absolute bottom-full left-1/2 -translate-x-1/2 mb-1 px-2 py-1 rounded bg-bg-secondary border border-border text-[10px] text-text-primary whitespace-nowrap z-10">
                {date.toLocaleDateString()} {date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                <br />
                Score: {snapshot.misinfo_score} | Claims: {snapshot.claim_count}
              </div>
            </div>
          );
        })}
      </div>

      <div className="flex justify-between mt-1 text-[10px] text-text-muted">
        <span>
          {timeline.length > 0
            ? new Date(timeline[timeline.length - 1].snapshot_at * 1000).toLocaleDateString()
            : ''}
        </span>
        <span>
          {timeline.length > 0
            ? new Date(timeline[0].snapshot_at * 1000).toLocaleDateString()
            : ''}
        </span>
      </div>
    </Card>
  );
}
