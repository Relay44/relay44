'use client';

import { useState, useCallback } from 'react';
import { useRouter } from 'next/navigation';
import { PageShell } from '@/components/layout';
import { Button, Input, Select, useToast } from '@/components/ui';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { useAdminGate } from '@/hooks/useAdminGate';
import { useCreateDistributionMarket } from '@/hooks/useDistribution';

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

const COLLATERAL_TOKENS = [
  { value: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913', label: 'USDC' },
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
  const isAdmin = useAdminGate(address);
  const { addToast } = useToast();
  const createMutation = useCreateDistributionMarket();

  const [question, setQuestion] = useState('');
  const [description, setDescription] = useState('');
  const [category, setCategory] = useState('');
  const [outcomeMin, setOutcomeMin] = useState('');
  const [outcomeMax, setOutcomeMax] = useState('');
  const [outcomeUnit, setOutcomeUnit] = useState('');
  const [liquidityParam, setLiquidityParam] = useState('100');
  const [collateralToken, setCollateralToken] = useState(COLLATERAL_TOKENS[0].value);
  const [feeBps, setFeeBps] = useState('100');
  const [resolver, setResolver] = useState('');
  const [useOracle, setUseOracle] = useState(false);
  const [oracleFeedId, setOracleFeedId] = useState('');
  const [tradingEnd, setTradingEnd] = useState('');
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

    const lp = parseFloat(liquidityParam);
    if (isNaN(lp) || lp <= 0) {
      e.liquidityParam = 'Must be a positive number';
    }

    const fee = parseInt(feeBps, 10);
    if (isNaN(fee) || fee < 0 || fee > 1000) {
      e.feeBps = 'Must be 0-1000 (basis points)';
    }

    if (!tradingEnd) {
      e.tradingEnd = 'Required';
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

  if (!isAdmin) {
    return (
      <PageShell>
        <div className="flex flex-col items-center justify-center py-20 gap-4">
          <p className="text-text-secondary text-sm">
            {address ? 'Access denied. Admin wallet required.' : 'Connect your wallet to continue.'}
          </p>
        </div>
      </PageShell>
    );
  }

  return (
    <PageShell>
      <div className="container mx-auto max-w-2xl px-4 py-8">
        <h1 className="text-lg font-medium text-text-primary mb-6">
          Create Distribution Market
        </h1>

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
              placeholder="Resolution criteria and additional context..."
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
              placeholder="100"
              value={outcomeMax}
              onChange={(e) => setOutcomeMax(e.target.value)}
              error={errors.outcomeMax}
            />
            <Input
              label="Unit"
              placeholder="USD, %, ECV..."
              value={outcomeUnit}
              onChange={(e) => setOutcomeUnit(e.target.value)}
            />
          </div>

          {/* LMSR Parameters */}
          <div className="grid grid-cols-2 gap-4">
            <Input
              label="Liquidity Parameter (b)"
              type="number"
              step="any"
              placeholder="100"
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

          {/* Collateral Token */}
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

          {/* Lifecycle */}
          <div className="grid grid-cols-2 gap-4">
            <Input
              label="Trading End"
              type="datetime-local"
              value={tradingEnd}
              onChange={(e) => setTradingEnd(e.target.value)}
              error={errors.tradingEnd}
            />
            <Input
              label="Resolution Deadline"
              type="datetime-local"
              value={resolutionDeadline}
              onChange={(e) => setResolutionDeadline(e.target.value)}
              hint="Optional. Defaults to trading end + 7 days"
            />
          </div>

          {/* Resolution */}
          <div className="border border-border p-4 space-y-4">
            <h2 className="text-sm font-medium text-text-primary">Resolution</h2>

            <Input
              label="Resolver Address"
              placeholder="0x..."
              value={resolver}
              onChange={(e) => setResolver(e.target.value)}
              hint="Leave empty for admin resolution"
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
              Create Distribution Market
            </Button>
            <Button
              variant="ghost"
              size="lg"
              onClick={() => router.push('/markets')}
            >
              Cancel
            </Button>
          </div>
        </div>
      </div>
    </PageShell>
  );
}
