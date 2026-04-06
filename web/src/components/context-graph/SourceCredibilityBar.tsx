'use client';

import { Badge } from '@/components/ui/Badge';
import type { GraphNode } from './types';

interface Props {
  source: GraphNode;
}

export function SourceCredibilityBar({ source }: Props) {
  const credibility = (source.data.credibilityScore as number) || 50;
  const platform = (source.data.platform as string) || 'unknown';
  const url = source.data.url as string;

  const barColor =
    credibility >= 80 ? '#10b981' :
    credibility >= 50 ? '#f59e0b' :
    credibility >= 30 ? '#f97316' : '#ef4444';

  return (
    <div className="flex items-center gap-2 py-1">
      <Badge variant="muted" className="w-14 justify-center shrink-0 text-[10px] uppercase">
        {platform}
      </Badge>
      <div className="flex-1 min-w-0">
        {url ? (
          <a
            href={url}
            target="_blank"
            rel="noopener noreferrer"
            className="text-xs text-text-primary hover:text-accent truncate block"
          >
            {source.label}
          </a>
        ) : (
          <span className="text-xs text-text-primary truncate block">
            {source.label}
          </span>
        )}
      </div>
      <div className="w-16 h-1.5 bg-bg-tertiary rounded-full overflow-hidden shrink-0">
        <div
          className="h-full rounded-full"
          style={{ width: `${credibility}%`, backgroundColor: barColor }}
        />
      </div>
      <span className="text-xs text-text-muted w-6 text-right shrink-0">
        {credibility}
      </span>
    </div>
  );
}
