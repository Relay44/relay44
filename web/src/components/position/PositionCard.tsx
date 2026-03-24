import Link from 'next/link';
import { Card, Badge } from '@/components/ui';
import { formatPnl, formatPnlPercent, formatPrice } from '@/lib/utils';
import type { Position } from '@/types';

export interface PositionCardProps {
  position: Position;
}

export function PositionCard({ position }: PositionCardProps) {
  const hasYes = position.yesBalance > 0;
  const hasNo = position.noBalance > 0;

  const totalValue =
    position.yesBalance * position.currentYesPrice +
    position.noBalance * position.currentNoPrice;

  const pnlPercent =
    position.totalDeposited > 0
      ? ((totalValue - position.totalDeposited) / position.totalDeposited) * 100
      : 0;

  return (
    <Link href={`/markets/${encodeURIComponent(position.marketId)}`} className="block">
      <Card hover>
        <div className="mb-3 flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <h3 className="font-medium text-text-primary line-clamp-2 flex-1">
            {position.marketQuestion}
          </h3>
          <div className="flex gap-1">
            {hasYes && <Badge variant="bid">YES</Badge>}
            {hasNo && <Badge variant="ask">NO</Badge>}
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3 text-sm lg:grid-cols-5">
          <div>
            <div className="text-text-secondary text-xs">Shares</div>
            <div>
              {hasYes && <span className="text-bid">{position.yesBalance} Y</span>}
              {hasYes && hasNo && ' / '}
              {hasNo && <span className="text-ask">{position.noBalance} N</span>}
            </div>
          </div>
          <div>
            <div className="text-text-secondary text-xs">Avg Cost</div>
            <div>
              {hasYes && <span>${formatPrice(position.avgYesCost)}</span>}
              {hasYes && hasNo && ' / '}
              {hasNo && <span>${formatPrice(position.avgNoCost)}</span>}
            </div>
          </div>
          <div>
            <div className="text-text-secondary text-xs">Value</div>
            <div>${formatPrice(totalValue)}</div>
          </div>
          <div>
            <div className="text-text-secondary text-xs">P&L</div>
            <div
              className={
                position.unrealizedPnl >= 0 ? 'text-accent' : 'text-text-secondary'
              }
            >
              {formatPnlPercent(pnlPercent)}
            </div>
          </div>
          <div>
            <div className="text-text-secondary text-xs">Claimable</div>
            <div>${formatPrice(position.claimable)}</div>
          </div>
        </div>
      </Card>
    </Link>
  );
}
