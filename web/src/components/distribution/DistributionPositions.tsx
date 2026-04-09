'use client';

import { useState, useCallback } from 'react';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui';
import type { DistributionPosition } from '@/types/distribution';

export interface DistributionPositionsProps {
  positions: DistributionPosition[];
  marketResolved: boolean;
  onClose: (positionId: number) => void;
  onClaim: (positionId: number) => void;
}

function formatVal(value: number | undefined, decimals = 4): string {
  if (value === undefined || value === null) return '--';
  return value.toFixed(decimals);
}

export function DistributionPositions({
  positions,
  marketResolved,
  onClose,
  onClaim,
}: DistributionPositionsProps) {
  const [pendingAction, setPendingAction] = useState<number | null>(null);

  const handleClose = useCallback(
    async (positionId: number) => {
      if (pendingAction !== null) return;
      setPendingAction(positionId);
      try {
        await onClose(positionId);
      } finally {
        setPendingAction(null);
      }
    },
    [onClose, pendingAction],
  );

  const handleClaim = useCallback(
    async (positionId: number) => {
      if (pendingAction !== null) return;
      setPendingAction(positionId);
      try {
        await onClaim(positionId);
      } finally {
        setPendingAction(null);
      }
    },
    [onClaim, pendingAction],
  );

  if (positions.length === 0) {
    return (
      <div className="text-center text-text-secondary text-xs py-8">
        No positions yet
      </div>
    );
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-xs">
        <thead>
          <tr className="border-b border-border">
            <th className="text-left text-text-secondary uppercase tracking-wide font-medium py-2 px-2">
              {'\u03BC'}
            </th>
            <th className="text-left text-text-secondary uppercase tracking-wide font-medium py-2 px-2">
              {'\u03C3'}
            </th>
            <th className="text-right text-text-secondary uppercase tracking-wide font-medium py-2 px-2">
              Size
            </th>
            <th className="text-right text-text-secondary uppercase tracking-wide font-medium py-2 px-2">
              Cost
            </th>
            <th className="text-right text-text-secondary uppercase tracking-wide font-medium py-2 px-2">
              Value
            </th>
            <th className="text-right text-text-secondary uppercase tracking-wide font-medium py-2 px-2">
              PnL
            </th>
            <th className="text-right text-text-secondary uppercase tracking-wide font-medium py-2 px-2">
              Action
            </th>
          </tr>
        </thead>
        <tbody>
          {positions.map((pos) => {
            const pnl = pos.pnl ?? 0;
            const isPnlPositive = pnl >= 0;
            const isActionPending = pendingAction === pos.positionId;

            return (
              <tr
                key={pos.id}
                className="border-b border-border/50 hover:bg-bg-secondary transition-colors duration-fast"
              >
                <td className="py-2.5 px-2 font-mono tabular-nums text-text-primary">
                  {formatVal(pos.mu, 3)}
                </td>
                <td className="py-2.5 px-2 font-mono tabular-nums text-text-primary">
                  {formatVal(pos.sigma, 3)}
                </td>
                <td className="py-2.5 px-2 font-mono tabular-nums text-text-primary text-right">
                  {formatVal(pos.size, 2)}
                </td>
                <td className="py-2.5 px-2 font-mono tabular-nums text-text-primary text-right">
                  {formatVal(pos.costBasis, 4)}
                </td>
                <td className="py-2.5 px-2 font-mono tabular-nums text-text-primary text-right">
                  {formatVal(pos.collateral, 4)}
                </td>
                <td
                  className={cn(
                    'py-2.5 px-2 font-mono tabular-nums text-right',
                    isPnlPositive ? 'text-bid' : 'text-ask'
                  )}
                >
                  {pnl >= 0 ? '+' : ''}{formatVal(pnl, 4)}
                </td>
                <td className="py-2.5 px-2 text-right">
                  {pos.status === 'open' && !marketResolved && (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => handleClose(pos.positionId)}
                      disabled={pendingAction !== null}
                      loading={isActionPending}
                      className="text-[0.65rem] px-2 py-1 h-auto"
                    >
                      Close
                    </Button>
                  )}
                  {marketResolved && pos.status !== 'claimed' && (
                    <Button
                      variant="bid"
                      size="sm"
                      onClick={() => handleClaim(pos.positionId)}
                      disabled={pendingAction !== null}
                      loading={isActionPending}
                      className="text-[0.65rem] px-2 py-1 h-auto"
                    >
                      Claim
                    </Button>
                  )}
                  {pos.status === 'claimed' && (
                    <span className="text-text-muted text-[0.65rem]">Claimed</span>
                  )}
                  {pos.status === 'closed' && (
                    <span className="text-text-muted text-[0.65rem]">Closed</span>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
