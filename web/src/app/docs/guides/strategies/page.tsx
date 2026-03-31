import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Strategies Guide',
  description: 'Trading strategy types on Relay44 — momentum, mean-revert, and market-maker strategies explained.',
  path: '/docs/guides/strategies',
  keywords: ['strategies', 'momentum', 'mean-revert', 'market-maker', 'trading strategies'],
});

const strategies = [
  {
    name: 'Default',
    tag: 'default',
    description: 'Always executes at the configured price and quantity. No signal evaluation — the agent trades every tick if conditions allow. Good for simple scheduled buys.',
  },
  {
    name: 'Momentum',
    tag: 'momentum',
    description: 'Executes only when the price is trending favorably. For buys, the agent waits until the mid price is below the target price (spread is positive). For sells, it waits until mid price exceeds the target. Quantity scales from 0.5x to 1.5x based on the size of the favorable spread — bigger gaps mean more conviction.',
  },
  {
    name: 'Mean Revert',
    tag: 'mean-revert',
    description: 'Buys when the price drops below the target (expecting reversion up) and sells when it rises above (expecting reversion down). Skips execution when price is within a 2% dead zone around the target to avoid noise. Quantity scales from 0.5x to 2x based on deviation — larger deviations trigger larger positions.',
  },
  {
    name: 'Market Maker',
    tag: 'market-maker',
    description: 'Always executes, but adjusts the limit price to sit at the top of the order book. For buys, it places just above the best bid (capped at the agent\'s target price). For sells, just below the best ask (floored at the target). Quantity remains fixed. Designed for providing liquidity and earning the spread.',
  },
];

export default function StrategiesGuidePage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/guides/strategies', name: 'Strategies Guide', description: 'Trading strategies on Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Guides', url: '/docs/guides' },
            { name: 'Strategies', url: '/docs/guides/strategies' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Strategies</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Each agent runs a strategy that determines when to trade and how to size positions.
        Choose a strategy that matches your market thesis.
      </p>

      <div className="mt-8 grid gap-6">
        {strategies.map((s) => (
          <Card key={s.tag} className="p-6">
            <div className="flex items-center gap-3">
              <h2 className="text-lg font-semibold text-text-primary">{s.name}</h2>
              <code className="border border-border px-2 py-0.5 text-xs text-text-muted">{s.tag}</code>
            </div>
            <p className="mt-3 text-sm leading-6 text-text-secondary">{s.description}</p>
          </Card>
        ))}
      </div>

      <div className="mt-8">
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Combining with guardrails</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Strategies control <em>when</em> and <em>how much</em> to trade. Guardrails add hard
            limits on top: max notional per execution, daily spend cap, and slippage tolerance.
            Even an aggressive strategy respects guardrail limits. Configure both for optimal
            risk management.
          </p>
        </Card>
      </div>
    </>
  );
}
