import { Badge } from '@/components/ui';
import { MARKET_STATUS_LABELS } from '@/lib/constants';
import type { Market } from '@/types';

export interface MarketHeaderProps {
  market: Market;
}

export function MarketHeader({ market }: MarketHeaderProps) {
  // Use accent for active, muted for others - no harsh green/red
  const statusVariant =
    market.status === 'active'
      ? 'accent'
      : market.status === 'resolved'
        ? 'default'
        : 'muted';

  return (
    <div className="mb-6">
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <Badge variant="muted">{market.category}</Badge>
        <Badge variant={market.isExternal ? 'accent' : 'muted'}>
          {market.provider}
        </Badge>
        <Badge variant="muted">
          {market.chainId === 137 ? 'polygon' : market.chainId === 8453 ? 'base' : `chain-${market.chainId}`}
        </Badge>
        <Badge variant={statusVariant}>
          {MARKET_STATUS_LABELS[market.status]}
        </Badge>
      </div>
      <h1 className="mb-2 text-xl font-bold sm:text-2xl">{market.question}</h1>
      <p className="text-sm leading-6 text-text-secondary">{market.description}</p>
    </div>
  );
}
