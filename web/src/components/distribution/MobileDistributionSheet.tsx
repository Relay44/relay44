'use client';

import { useState } from 'react';
import { TrendingUp } from 'lucide-react';
import { Button } from '@/components/ui';
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { DistributionTradePanel } from './DistributionTradePanel';
import type { DistributionTradePanelProps } from './DistributionTradePanel';

interface MobileDistributionSheetProps extends DistributionTradePanelProps {}

export function MobileDistributionSheet(props: MobileDistributionSheetProps) {
  const [open, setOpen] = useState(false);

  const mu = props.market.marketMu ?? (props.market.outcomeMin + props.market.outcomeMax) / 2;
  const sigma = props.market.marketSigma ?? (props.market.outcomeMax - props.market.outcomeMin) / 6;

  return (
    <>
      {/* Sticky bottom bar — mobile only */}
      <div className="fixed bottom-0 inset-x-0 z-40 border-t border-border bg-bg-primary/95 backdrop-blur-sm p-3 lg:hidden">
        <div className="flex items-center justify-between gap-3 max-w-[1400px] mx-auto">
          <div className="flex items-center gap-3 min-w-0 text-xs">
            <span className="text-text-secondary font-mono tabular-nums">
              {'\u03BC'} {mu.toFixed(2)}
            </span>
            <span className="text-text-muted">|</span>
            <span className="text-text-secondary font-mono tabular-nums">
              {'\u03C3'} {sigma.toFixed(2)}
            </span>
            {props.market.outcomeUnit && (
              <span className="text-text-muted">{props.market.outcomeUnit}</span>
            )}
          </div>
          <Button
            variant="bid"
            size="sm"
            className="flex items-center gap-1.5 shrink-0"
            onClick={() => setOpen(true)}
          >
            <TrendingUp className="w-3.5 h-3.5" />
            Trade
          </Button>
        </div>
      </div>

      {/* Sheet */}
      <Sheet open={open} onOpenChange={setOpen}>
        <SheetContent side="bottom" className="max-h-[85vh] overflow-y-auto p-0">
          <SheetHeader className="px-4 pt-4 pb-2">
            <SheetTitle className="text-sm">Trade</SheetTitle>
          </SheetHeader>
          <DistributionTradePanel {...props} />
        </SheetContent>
      </Sheet>
    </>
  );
}
