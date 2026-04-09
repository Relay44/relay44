'use client';

import { useState, useCallback } from 'react';
import { useRouter } from 'next/navigation';
import { ChevronDown } from 'lucide-react';
import { PageShell } from '@/components/layout';
import { Button, Input, Select, useToast } from '@/components/ui';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { useCreateDistributionMarket } from '@/hooks/useDistribution';
import { cn } from '@/lib/utils';

const CATEGORIES = [
  { value: 'crypto', label: 'Crypto' },
  { value: 'politics', label: 'Politics' },
  { value: 'sports', label: 'Sports' },
  { value: 'technology', label: 'Technology' },
  { value: 'entertainment', label: 'Entertainment' },
  { value: 'science', label: 'Science' },
  { value: 'finance', label: 'Finance' },
  { value: 'other', label: 'Other' },
];

const DEFAULT_COLLATERAL = '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913'; // USDC
const COLLATERAL_TOKENS = [
  { value: DEFAULT_COLLATERAL, label: 'USDC' },
  { value: '0x0000000000000000000000000000000000000000', label: 'RELAY' },
];

function generateMarketId(question: string): string {
  const slug = question
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, '')
    .replace(/\s+/g, '-')
    .slice(0, 48);
  const suffix = Math.random().toString(36).slice(2, 8);
  return `dist-${slug}-${suffix}`;
}

export default function CreateDistributionMarketPage() {
  const router = useRouter();
  const { address } = useBaseWallet();
  const { addToast } = useToast();
  const createMutation = useCreateDistributionMarket();

  // Core fields
  const [question, setQuestion] = useState('');
  const [description, setDescription] = useState('');
  const [category, setCategory] = useState('');
  const [outcomeMin, setOutcomeMin] = useState('');
  const [outcomeMax, setOutcomeMax] = useState('');
  const [outcomeUnit, setOutcomeUnit] = useState('');
  const [tradingEnd, setTradingEnd] = useState('');

  // Advanced (hidden by default)
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [liquidityParam, setLiquidityParam] = useState('300');
  const [collateralToken, setCollateralToken] = useState(DEFAULT_COLLATERAL);
  const [feeBps, setFeeBps] = useState('100');
  const [resolver, setResolver] = useState('');
  const [useOracle, setUseOracle] = useState(false);
  const [oracleFeedId, setOracleFeedId] = useState('');
  const [resolutionDeadline, setResolutionDeadline] = useState('');

  const [errors, setErrors] = useState<Record<string, string>>({});

  const validate = useCallback((): boolean => {
    const e: Record<string, string> = {};

    if (!question.trim() || question.length < 10) {
      e.question = 'Question must be at least 10 characters';
    }
    if (!question.endsWith('?')) {
      e.question = 'Question must end with ?';
    }

    const min = parseFloat(outcomeMin);
    const max = parseFloat(outcomeMax);
    if (isNaN(min)) e.outcomeMin = 'Required';
    if (isNaN(max)) e.outcomeMax = 'Required';
    if (!isNaN(min) && !isNaN(max) && min >= max) {
      e.outcomeMax = 'Must be greater than min';
    }

    if (!tradingEnd) {
      e.tradingEnd = 'Required';
    } else {
      const end = new Date(tradingEnd);
      if (end <= new Date()) {
        e.tradingEnd = 'Must be in the future';
      }
    }

    const lp = parseFloat(liquidityParam);
    if (isNaN(lp) || lp <= 0) {
      e.liquidityParam = 'Must be a positive number';
    }

    const fee = parseInt(feeBps, 10);
    if (isNaN(fee) || fee < 0 || fee > 1000) {
      e.feeBps = 'Must be 0-1000 (basis points)';
    }

    if (useOracle && !oracleFeedId.trim()) {
      e.oracleFeedId = 'Feed ID required when oracle is enabled';
    }

    setErrors(e);
    return Object.keys(e).length === 0;
  }, [question, outcomeMin, outcomeMax, liquidityParam, feeBps, tradingEnd, useOracle, oracleFeedId]);

  const handleSubmit = useCallback(async () => {
    if (!validate()) return;

    const marketId = generateMarketId(question);

    try {
      await createMutation.mutateAsync({
        marketId,
        question: question.trim(),
        description: description.trim() || undefined,
        category: category || undefined,
        outcomeMin: parseFloat(outcomeMin),
        outcomeMax: parseFloat(outcomeMax),
        outcomeUnit: outcomeUnit.trim() || undefined,
        liquidityParam: parseFloat(liquidityParam),
        collateralToken,
        feeBps: parseInt(feeBps, 10),
        resolver: resolver.trim() || undefined,
        useOracle,
        oracleFeedId: useOracle ? oracleFeedId.trim() : undefined,
        tradingEnd: new Date(tradingEnd).toISOString(),
        resolutionDeadline: resolutionDeadline
          ? new Date(resolutionDeadline).toISOString()
          : undefined,
      });
      addToast('Distribution market created', 'success');
      router.push(`/distribution/${encodeURIComponent(marketId)}`);
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to create market';
      addToast(msg, 'error');
    }
  }, [
    validate, question, description, category, outcomeMin, outcomeMax,
    outcomeUnit, liquidityParam, collateralToken, feeBps, resolver,
    useOracle, oracleFeedId, tradingEnd, resolutionDeadline,
    createMutation, addToast, router,
  ]);

  if (!address) {
    return (
      <PageShell>
        <div className="flex flex-col items-center justify-center py-20 gap-4">
          <p className="text-text-secondary text-sm">
            Connect your wallet to create a market.
          </p>
        </div>
      </PageShell>
    );
  }

  return (
    <PageShell>
      <div className="container mx-auto max-w-2xl px-4 py-8">
        <h1 className="text-lg font-medium text-text-primary mb-2">
          Create Distribution Market
        </h1>
        <p className="text-xs text-text-muted mb-6">
          Create a continuous outcome market where traders express beliefs as probability distributions.
        </p>

        <div className="space-y-6">
          {/* Question */}
          <Input
            label="Question"
            placeholder="What will the price of BTC be on Dec 31?"
            value={question}
            onChange={(e) => setQuestion(e.target.value)}
            error={errors.question}
          />

          {/* Description */}
          <div className="space-y-1.5">
            <label className="block text-sm font-medium text-text-primary">
              Description
            </label>
            <textarea
              className="flex w-full min-h-[80px] px-3 py-2 bg-bg-secondary border border-border text-base text-text-primary placeholder:text-text-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-1 focus-visible:ring-offset-bg-base focus-visible:border-accent"
              placeholder="How will this market be resolved? Add resolution criteria..."
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>

          {/* Category */}
          <div className="space-y-1.5">
            <label className="block text-sm font-medium text-text-primary">
              Category
            </label>
            <Select
              options={CATEGORIES}
              value={category}
              onChange={(e) => setCategory(e.target.value)}
              placeholder="Select category..."
            />
          </div>

          {/* Outcome Range */}
          <div className="grid grid-cols-3 gap-4">
            <Input
              label="Outcome Min"
              type="number"
              step="any"
              placeholder="0"
              value={outcomeMin}
              onChange={(e) => setOutcomeMin(e.target.value)}
              error={errors.outcomeMin}
            />
            <Input
              label="Outcome Max"
              type="number"
              step="any"
              placeholder="100000"
              value={outcomeMax}
              onChange={(e) => setOutcomeMax(e.target.value)}
              error={errors.outcomeMax}
            />
            <Input
              label="Unit"
              placeholder="USD, %, etc."
              value={outcomeUnit}
              onChange={(e) => setOutcomeUnit(e.target.value)}
            />
          </div>

          {/* Trading End */}
          <Input
            label="Trading End"
            type="datetime-local"
            value={tradingEnd}
            onChange={(e) => setTradingEnd(e.target.value)}
            error={errors.tradingEnd}
            hint="When trading closes. Resolution can happen after this."
          />

          {/* Advanced Settings */}
          <button
            type="button"
            onClick={() => setShowAdvanced((v) => !v)}
            className="flex items-center gap-2 text-xs text-text-secondary hover:text-text-primary transition-colors"
          >
            <ChevronDown
              className={cn(
                'h-3.5 w-3.5 transition-transform',
                showAdvanced && 'rotate-180',
              )}
            />
            Advanced Settings
          </button>

          {showAdvanced && (
            <div className="space-y-4 border border-border p-4">
              <div className="grid grid-cols-2 gap-4">
                <Input
                  label="Liquidity Parameter (b)"
                  type="number"
                  step="any"
                  placeholder="300"
                  value={liquidityParam}
                  onChange={(e) => setLiquidityParam(e.target.value)}
                  error={errors.liquidityParam}
                  hint="Higher = more liquidity, less price impact"
                />
                <Input
                  label="Fee (basis points)"
                  type="number"
                  step="1"
                  placeholder="100"
                  value={feeBps}
                  onChange={(e) => setFeeBps(e.target.value)}
                  error={errors.feeBps}
                  hint="100 bps = 1%"
                />
              </div>

              <div className="space-y-1.5">
                <label className="block text-sm font-medium text-text-primary">
                  Collateral Token
                </label>
                <Select
                  options={COLLATERAL_TOKENS}
                  value={collateralToken}
                  onChange={(e) => setCollateralToken(e.target.value)}
                />
              </div>

              <Input
                label="Resolution Deadline"
                type="datetime-local"
                value={resolutionDeadline}
                onChange={(e) => setResolutionDeadline(e.target.value)}
                hint="Optional. Defaults to trading end + 7 days"
              />

              <Input
                label="Resolver Address"
                placeholder="0x..."
                value={resolver}
                onChange={(e) => setResolver(e.target.value)}
                hint="Leave empty to resolve it yourself"
              />

              <label className="flex items-center gap-2 text-sm text-text-primary cursor-pointer">
                <input
                  type="checkbox"
                  checked={useOracle}
                  onChange={(e) => setUseOracle(e.target.checked)}
                  className="accent-accent"
                />
                Use Pyth Oracle for auto-resolution
              </label>

              {useOracle && (
                <Input
                  label="Pyth Feed ID"
                  placeholder="0x..."
                  value={oracleFeedId}
                  onChange={(e) => setOracleFeedId(e.target.value)}
                  error={errors.oracleFeedId}
                  hint="Pyth price feed ID for automatic resolution"
                />
              )}
            </div>
          )}

          {/* Submit */}
          <div className="flex items-center gap-4 pt-4">
            <Button
              variant="primary"
              size="lg"
              onClick={handleSubmit}
              loading={createMutation.isPending}
              disabled={createMutation.isPending}
              className="flex-1"
            >
              Create Market
            </Button>
            <Button
              variant="ghost"
              size="lg"
              onClick={() => router.push('/distribution')}
            >
              Cancel
            </Button>
          </div>
        </div>
      </div>
    </PageShell>
  );
}
