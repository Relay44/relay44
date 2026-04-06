'use client';

import { Card } from '@/components/ui/Card';
import { Badge } from '@/components/ui/Badge';
import type { MisinformationScore } from './types';
import { RISK_COLORS } from './types';

interface Props {
  score: MisinformationScore;
}

const RISK_BADGE_VARIANT: Record<string, 'success' | 'warning' | 'danger'> = {
  low: 'success',
  medium: 'warning',
  high: 'danger',
  critical: 'danger',
};

export function MisinfoScoreCard({ score }: Props) {
  const color = RISK_COLORS[score.riskLevel];

  return (
    <Card className="p-4">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-sm font-medium text-text-secondary">Misinformation Risk</h3>
        <Badge variant={RISK_BADGE_VARIANT[score.riskLevel] ?? 'muted'} className="uppercase">
          {score.riskLevel}
        </Badge>
      </div>

      <div className="flex items-center gap-4 mb-4">
        <div className="relative w-16 h-16 shrink-0">
          <svg viewBox="0 0 36 36" className="w-full h-full">
            <path
              d="M18 2.0845
                a 15.9155 15.9155 0 0 1 0 31.831
                a 15.9155 15.9155 0 0 1 0 -31.831"
              fill="none"
              className="stroke-border"
              strokeWidth="3"
            />
            <path
              d="M18 2.0845
                a 15.9155 15.9155 0 0 1 0 31.831
                a 15.9155 15.9155 0 0 1 0 -31.831"
              fill="none"
              stroke={color}
              strokeWidth="3"
              strokeDasharray={`${score.overall}, 100`}
              strokeLinecap="round"
            />
          </svg>
          <span
            className="absolute inset-0 flex items-center justify-center text-lg font-bold"
            style={{ color }}
          >
            {score.overall}
          </span>
        </div>
        <div className="flex-1">
          <p className="text-sm text-text-primary">{score.summary}</p>
          <p className="text-xs text-text-muted mt-1">
            Confidence: {score.confidence}%
          </p>
        </div>
      </div>

      <div className="space-y-2">
        {score.components.map((comp) => (
          <div key={comp.name} className="flex items-center gap-2">
            <span className="text-xs text-text-muted w-28 shrink-0">
              {comp.name.replace(/([A-Z])/g, ' $1').trim()}
            </span>
            <div className="flex-1 h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
              <div
                className="h-full rounded-full transition-all"
                style={{
                  width: `${comp.value}%`,
                  backgroundColor:
                    comp.value > 70 ? '#ef4444' :
                    comp.value > 40 ? '#f59e0b' : '#10b981',
                }}
              />
            </div>
            <span className="text-xs text-text-secondary w-8 text-right">
              {Math.round(comp.value)}
            </span>
          </div>
        ))}
      </div>
    </Card>
  );
}
