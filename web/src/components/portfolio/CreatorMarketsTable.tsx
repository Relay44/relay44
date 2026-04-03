'use client';

import { Badge, Card } from '@/components/ui';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import type { CreatorEconomicsMarketSummary } from '@/types';
import { formatCurrency } from '@/lib/utils';

function formatBps(bps: number): string {
  return `${bps >= 0 ? '+' : ''}${(bps / 100).toFixed(1)}%`;
}

function statusVariant(status?: string) {
  if (!status) {
    return 'secondary' as const;
  }

  switch (status) {
    case 'active':
    case 'bootstrapping':
      return 'success' as const;
    case 'graduated':
    case 'resolved':
      return 'accent' as const;
    case 'paused':
    case 'underfunded':
      return 'warning' as const;
    case 'error':
      return 'danger' as const;
    default:
      return 'secondary' as const;
  }
}

interface CreatorMarketsTableProps {
  markets: CreatorEconomicsMarketSummary[];
  selectedMarketId?: string | null;
  onSelect: (marketId: string) => void;
}

export function CreatorMarketsTable({
  markets,
  selectedMarketId,
  onSelect,
}: CreatorMarketsTableProps) {
  return (
    <Card className="overflow-hidden p-0">
      <div className="border-b border-border px-4 py-3 sm:px-5">
        <h2 className="text-lg font-semibold text-text-primary">Creator markets</h2>
        <p className="mt-1 text-sm text-text-secondary">
          Private liquidity economics for markets you launched.
        </p>
      </div>
      <Table>
        <TableHeader>
          <TableRow className="hover:bg-transparent">
            <TableHead className="px-4 sm:px-5">Market</TableHead>
            <TableHead>Status</TableHead>
            <TableHead className="text-right">Seed</TableHead>
            <TableHead className="text-right">Burn</TableHead>
            <TableHead className="text-right">Net P&amp;L</TableHead>
            <TableHead className="text-right">ROI</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {markets.map((market) => {
            const selected = market.marketId === selectedMarketId;

            return (
              <TableRow
                key={market.marketId}
                className={selected ? 'bg-bg-secondary/70' : undefined}
              >
                <TableCell className="px-4 py-3 sm:px-5">
                  <button
                    type="button"
                    onClick={() => onSelect(market.marketId)}
                    className="block w-full text-left"
                  >
                    <div className="font-medium text-text-primary">
                      {market.marketQuestion}
                    </div>
                    <div className="mt-1 text-xs text-text-secondary">
                      Market #{market.marketId}
                    </div>
                  </button>
                </TableCell>
                <TableCell>
                  <Badge variant={statusVariant(market.bootstrapStatus ?? market.status)}>
                    {(market.bootstrapStatus ?? market.status).replace(/_/g, ' ')}
                  </Badge>
                </TableCell>
                <TableCell className="text-right">
                  {formatCurrency(market.seedUsdc)}
                </TableCell>
                <TableCell className="text-right">
                  {formatCurrency(market.subsidyBurnUsdc)}
                </TableCell>
                <TableCell
                  className={`text-right ${
                    market.netLiquidityPnlUsdc >= 0 ? 'text-accent' : 'text-text-primary'
                  }`}
                >
                  {formatCurrency(market.netLiquidityPnlUsdc)}
                </TableCell>
                <TableCell
                  className={`text-right ${
                    market.roiBps >= 0 ? 'text-accent' : 'text-text-primary'
                  }`}
                >
                  {formatBps(market.roiBps)}
                </TableCell>
              </TableRow>
            );
          })}
        </TableBody>
      </Table>
    </Card>
  );
}
