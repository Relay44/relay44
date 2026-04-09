'use client';

import { useState, useCallback } from 'react';
import { HelpCircle } from 'lucide-react';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/Tabs';
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
  TooltipProvider,
} from '@/components/ui/tooltip';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import type { DistributionMarket, DistributionQuote } from '@/types/distribution';
import type { DistributionPosition } from '@/types/distribution';
import { DistributionPositions } from './DistributionPositions';

export interface DistributionTradePanelProps {
  market: DistributionMarket;
  quote: DistributionQuote | null;
  isLoadingQuote: boolean;
  quoteError?: Error | null;
  proposalMu: number;
  proposalSigma: number;
  size: number;
  onMuChange: (mu: number) => void;
  onSigmaChange: (sigma: number) => void;
  onSizeChange: (size: number) => void;
  onExecuteTrade: () => void;
  isTrading: boolean;
  positions?: DistributionPosition[];
  marketResolved?: boolean;
  onClosePosition?: (positionId: number) => void;
  onClaimPayout?: (positionId: number) => void;
}

function formatNum(value: number, decimals = 4): string {
  return value.toFixed(decimals);
}

function InfoTip({ text }: { text: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button type="button" className="inline-flex ml-1 text-text-muted hover:text-text-secondary transition-colors">
          <HelpCircle className="w-3 h-3" />
        </button>
      </TooltipTrigger>
      <TooltipContent side="top" className="max-w-[220px] text-[11px] leading-relaxed">
        {text}
      </TooltipContent>
    </Tooltip>
  );
}

export function DistributionTradePanel({
  market,
  quote,
  isLoadingQuote,
  quoteError,
  proposalMu,
  proposalSigma,
  size,
  onMuChange,
  onSigmaChange,
  onSizeChange,
  onExecuteTrade,
  isTrading,
  positions = [],
  marketResolved = false,
  onClosePosition,
  onClaimPayout,
}: DistributionTradePanelProps) {
  const [activeTab, setActiveTab] = useState('trade');
  const [showConfirm, setShowConfirm] = useState(false);

  const deltaMu = quote?.deltaMu ?? (proposalMu - (market.marketMu ?? 0));
  const deltaSigma = quote?.deltaSigma ?? (proposalSigma - (market.marketSigma ?? 1));
  const minSigma = 0.15;

  const handleExecuteClick = useCallback(() => {
    setShowConfirm(true);
  }, []);

  const handleConfirm = useCallback(() => {
    setShowConfirm(false);
    onExecuteTrade();
  }, [onExecuteTrade]);

  return (
    <TooltipProvider delayDuration={300}>
      <div className="flex flex-col h-full">
        {/* Tab bar */}
        <Tabs value={activeTab} onValueChange={setActiveTab}>
          <TabsList className="w-full border-b border-border bg-transparent p-0 h-auto">
            <TabsTrigger
              value="trade"
              className={cn(
                'flex-1 py-2 text-[0.7rem] uppercase tracking-wide font-medium transition-colors duration-fast',
                'data-[state=active]:bg-accent data-[state=active]:text-text-inverse',
                'data-[state=inactive]:text-text-secondary data-[state=inactive]:hover:text-text-primary'
              )}
            >
              Trade
            </TabsTrigger>
            <TabsTrigger
              value="positions"
              className={cn(
                'flex-1 py-2 text-[0.7rem] uppercase tracking-wide font-medium transition-colors duration-fast',
                'data-[state=active]:bg-accent data-[state=active]:text-text-inverse',
                'data-[state=inactive]:text-text-secondary data-[state=inactive]:hover:text-text-primary'
              )}
            >
              Positions
            </TabsTrigger>
          </TabsList>

          <TabsContent value="trade" className="mt-0 flex-1 overflow-y-auto">
            <div className="p-4 space-y-5">
              {/* How it works */}
              <details className="group">
                <summary className="text-[0.65rem] uppercase tracking-[0.14em] text-text-muted cursor-pointer hover:text-text-secondary transition-colors select-none flex items-center gap-1.5">
                  <span className="transition-transform group-open:rotate-90">&#9654;</span>
                  How distribution markets work
                </summary>
                <div className="mt-2 text-xs text-text-secondary space-y-1.5 pl-4 border-l border-border">
                  <p>You trade by proposing a bell curve (normal distribution).</p>
                  <p>Set the <strong>mean</strong> where you think the outcome will center.</p>
                  <p>Set <strong>std dev</strong> to express confidence — lower = more confident.</p>
                  <p>When resolved, positions are scored by how well their curve predicted reality.</p>
                  <p>Tighter curves earn more if correct, but risk more if wrong.</p>
                </div>
              </details>

              {/* Proposed delta header */}
              <div className="flex items-center gap-3 text-xs text-bid font-mono">
                <span className="w-2 h-2 bg-bid inline-block" />
                <span>Proposed</span>
                <span className="tabular-nums">
                  {'\u0394'}{'\u03BC'}: {deltaMu >= 0 ? '+' : ''}{formatNum(deltaMu, 3)}
                </span>
                <span className="tabular-nums">
                  {'\u0394'}{'\u03C3'}: {deltaSigma >= 0 ? '+' : ''}{formatNum(deltaSigma, 3)}
                </span>
              </div>

              {/* MEAN (mu) control */}
              <div className="space-y-2">
                <label className="text-xs text-text-secondary uppercase tracking-wide flex items-center">
                  MEAN ({'\u03BC'})
                  <InfoTip text="Where you think the outcome will center. Drag right for a higher predicted value." />
                </label>
                <div className="font-mono tabular-nums text-2xl text-text-primary">
                  {formatNum(proposalMu, 3)}
                </div>
                <input
                  type="range"
                  min={market.outcomeMin}
                  max={market.outcomeMax}
                  step={(market.outcomeMax - market.outcomeMin) / 1000}
                  value={proposalMu}
                  onChange={(e) => onMuChange(parseFloat(e.target.value))}
                  className="w-full h-1 appearance-none cursor-pointer bg-border accent-bid"
                  style={{ accentColor: 'var(--color-bid)' }}
                />
                <div className="flex justify-between text-xs text-text-secondary font-mono tabular-nums">
                  <span>{formatNum(market.outcomeMin, 1)}</span>
                  <span>{formatNum(market.outcomeMax, 1)}</span>
                </div>
              </div>

              {/* STD DEV (sigma) control */}
              <div className="space-y-2">
                <label className="text-xs text-text-secondary uppercase tracking-wide flex items-center">
                  STD DEV ({'\u03C3'})
                  <InfoTip text="Your confidence level. Lower = more confident, higher payout if right, bigger loss if wrong." />
                </label>
                <div className="font-mono tabular-nums text-2xl text-text-primary">
                  {formatNum(proposalSigma, 3)}
                </div>
                <input
                  type="range"
                  min={minSigma}
                  max={(market.outcomeMax - market.outcomeMin) / 2}
                  step={0.001}
                  value={proposalSigma}
                  onChange={(e) => onSigmaChange(parseFloat(e.target.value))}
                  className="w-full h-1 appearance-none cursor-pointer bg-border"
                  style={{ accentColor: '#f59e0b' }}
                />
                <div className="flex justify-between text-xs text-text-secondary font-mono tabular-nums">
                  <span>{formatNum(minSigma, 3)}</span>
                  <span>{formatNum((market.outcomeMax - market.outcomeMin) / 2, 1)}</span>
                </div>
                <div className="text-xs text-text-muted">
                  Min {'\u03C3'}: {formatNum(minSigma, 3)} (contract enforced)
                </div>
              </div>

              {/* SIZE control */}
              <div className="space-y-2">
                <label className="text-xs text-text-secondary uppercase tracking-wide flex items-center">
                  Size
                  <InfoTip text="Units to trade. Multiplies your collateral cost and potential payout proportionally." />
                </label>
                <input
                  type="number"
                  min={1}
                  step={1}
                  value={size}
                  onChange={(e) => {
                    const v = parseInt(e.target.value, 10);
                    if (!isNaN(v) && v > 0) onSizeChange(v);
                  }}
                  className="w-full h-10 px-3 py-2 bg-bg-secondary border border-border text-text-primary font-mono tabular-nums text-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:border-accent"
                />
                <div className="text-xs text-text-muted">
                  Number of units (multiplies collateral cost)
                </div>
              </div>

              {/* Collateral Required card */}
              <div className="border border-border bg-bg-secondary p-4 space-y-3">
                <div className="text-xs text-text-secondary uppercase tracking-wide">
                  Collateral Required
                </div>
                <div className="font-mono tabular-nums text-2xl text-text-primary">
                  {isLoadingQuote ? (
                    <span className="text-text-muted animate-pulse">---</span>
                  ) : quote ? (
                    <>
                      {formatNum(quote.cost, 4)}{' '}
                      <span className="text-sm text-text-secondary">
                        {quote.collateralToken}
                      </span>
                    </>
                  ) : quoteError ? (
                    <span className="text-ask text-sm">Quote unavailable</span>
                  ) : (
                    <span className="text-text-muted text-sm">Adjust sliders to get a quote</span>
                  )}
                </div>

                {quoteError && !isLoadingQuote && (
                  <p className="text-xs text-text-muted">
                    {quoteError.message?.includes('insufficient')
                      ? 'Insufficient liquidity for this trade size'
                      : quoteError.message?.includes('sigma')
                        ? 'Standard deviation is outside the allowed range'
                        : 'Unable to calculate cost. Try adjusting your parameters.'}
                  </p>
                )}

                {quote && (
                  <div className="space-y-1.5 pt-2 border-t border-border">
                    <div className="flex justify-between text-xs">
                      <span className="text-text-secondary uppercase tracking-wide">
                        Fees (est)
                      </span>
                      <span className="font-mono tabular-nums text-text-primary">
                        {formatNum(quote.fees, 4)}
                      </span>
                    </div>
                    <div className="flex justify-between text-xs">
                      <span className="text-text-secondary uppercase tracking-wide">
                        Price Impact
                      </span>
                      <span
                        className={cn(
                          'font-mono tabular-nums',
                          Math.abs(quote.deltaMu) > 1 || Math.abs(quote.deltaSigma) > 1
                            ? 'text-ask'
                            : 'text-text-primary',
                        )}
                      >
                        {'\u0394\u03BC'} {quote.deltaMu >= 0 ? '+' : ''}{formatNum(quote.deltaMu, 3)}
                        {' / '}
                        {'\u0394\u03C3'} {quote.deltaSigma >= 0 ? '+' : ''}{formatNum(quote.deltaSigma, 3)}
                      </span>
                    </div>
                    <div className="flex justify-between text-xs">
                      <span className="text-text-secondary uppercase tracking-wide">
                        Min f(x)
                      </span>
                      <span className="font-mono tabular-nums text-text-primary">
                        {formatNum(quote.minFx, 6)}
                      </span>
                    </div>
                    <div className="flex justify-between text-xs">
                      <span className="text-text-secondary uppercase tracking-wide">
                        Arg min_x
                      </span>
                      <span className="font-mono tabular-nums text-text-primary">
                        {formatNum(quote.argMinX, 3)}
                      </span>
                    </div>
                  </div>
                )}

                <div className="text-xs text-text-muted pt-1">
                  Collateral secures against maximum potential loss
                </div>
              </div>

              {/* Payout Preview */}
              {quote && (
                <div className="border border-border bg-bg-secondary p-4 space-y-3">
                  <div className="text-xs text-text-secondary uppercase tracking-wide">
                    Payout Preview
                  </div>
                  <div className="text-xs text-text-muted">
                    Estimated return if the market resolves at these values
                  </div>
                  {[
                    { label: 'At your mean', value: proposalMu },
                    { label: 'At market mean', value: market.marketMu ?? proposalMu },
                    { label: 'At range center', value: (market.outcomeMin + market.outcomeMax) / 2 },
                  ].map(({ label, value }) => {
                    const z = (value - proposalMu) / proposalSigma;
                    const density = Math.exp(-0.5 * z * z);
                    const mktMu = market.marketMu ?? (market.outcomeMin + market.outcomeMax) / 2;
                    const mktSigma = market.marketSigma ?? (market.outcomeMax - market.outcomeMin) / 6;
                    const mktZ = (value - mktMu) / mktSigma;
                    const mktDensity = Math.exp(-0.5 * mktZ * mktZ);
                    const ratio = mktDensity > 0 ? Math.min(density / mktDensity, 10) : 1;
                    const estPayout = quote.cost * ratio;
                    return (
                      <div key={label} className="flex justify-between text-xs">
                        <span className="text-text-secondary">
                          {label} ({formatNum(value, 1)})
                        </span>
                        <span
                          className={cn(
                            'font-mono tabular-nums',
                            estPayout > quote.cost ? 'text-bid' : 'text-text-primary',
                          )}
                        >
                          ~{formatNum(estPayout, 4)}
                        </span>
                      </div>
                    );
                  })}
                  <div className="text-xs text-text-muted pt-1">
                    Estimates based on Gaussian density ratio (capped at 10x)
                  </div>
                </div>
              )}

              {/* Execute Trade button */}
              <Button
                variant="bid"
                size="lg"
                className="w-full py-3"
                onClick={handleExecuteClick}
                disabled={isTrading || isLoadingQuote || !quote || !!quoteError}
                loading={isTrading}
              >
                {isTrading ? 'Executing...' : 'Execute Trade'}
              </Button>
            </div>
          </TabsContent>

          <TabsContent value="positions" className="mt-0 flex-1 overflow-y-auto">
            <div className="p-4">
              {positions.length === 0 ? (
                <div className="text-center text-text-secondary text-xs py-8">
                  No positions in this market
                </div>
              ) : (
                <DistributionPositions
                  positions={positions}
                  marketResolved={marketResolved}
                  onClose={onClosePosition ?? (() => {})}
                  onClaim={onClaimPayout ?? (() => {})}
                />
              )}
            </div>
          </TabsContent>
        </Tabs>

        {/* Trade Confirmation Dialog */}
        <Dialog open={showConfirm} onOpenChange={setShowConfirm}>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Confirm Trade</DialogTitle>
              <DialogDescription>
                Review your position before executing.
              </DialogDescription>
            </DialogHeader>
            <div className="space-y-2 py-2">
              <div className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
                <span className="text-text-secondary uppercase tracking-wide">Mean ({'\u03BC'})</span>
                <span className="font-mono tabular-nums text-text-primary text-right">{formatNum(proposalMu, 3)}</span>
                <span className="text-text-secondary uppercase tracking-wide">Std Dev ({'\u03C3'})</span>
                <span className="font-mono tabular-nums text-text-primary text-right">{formatNum(proposalSigma, 3)}</span>
                <span className="text-text-secondary uppercase tracking-wide">Size</span>
                <span className="font-mono tabular-nums text-text-primary text-right">{size}</span>
                {quote && (
                  <>
                    <span className="text-text-secondary uppercase tracking-wide">Cost</span>
                    <span className="font-mono tabular-nums text-text-primary text-right">
                      {formatNum(quote.cost, 4)} {quote.collateralToken}
                    </span>
                    <span className="text-text-secondary uppercase tracking-wide">Fees</span>
                    <span className="font-mono tabular-nums text-text-primary text-right">
                      {formatNum(quote.fees, 4)}
                    </span>
                    <span className="text-text-secondary uppercase tracking-wide">Price Impact</span>
                    <span
                      className={cn(
                        'font-mono tabular-nums text-right',
                        Math.abs(quote.deltaMu) > 1 ? 'text-ask' : 'text-text-primary',
                      )}
                    >
                      {'\u0394\u03BC'} {quote.deltaMu >= 0 ? '+' : ''}{formatNum(quote.deltaMu, 3)}
                    </span>
                  </>
                )}
              </div>
            </div>
            <DialogFooter>
              <Button variant="ghost" onClick={() => setShowConfirm(false)}>
                Cancel
              </Button>
              <Button variant="bid" onClick={handleConfirm} loading={isTrading}>
                Confirm Trade
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </div>
    </TooltipProvider>
  );
}
