import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Agents Guide',
  description: 'Create and manage automated trading agents on Relay44 — strategies, paper trading, live execution, and performance monitoring.',
  path: '/docs/guides/agents',
  keywords: ['agents', 'trading bots', 'automated trading', 'strategies', 'paper trading'],
});

export default function AgentsGuidePage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/guides/agents', name: 'Agents Guide', description: 'Create and manage agents on Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Guides', url: '/docs/guides' },
            { name: 'Agents', url: '/docs/guides/agents' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Agents</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Agents are automated programs that trade on your behalf. Configure a strategy, set risk
        limits, and let the agent execute on a schedule.
      </p>

      <div className="mt-8 grid gap-6">
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Creating an agent</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Navigate to the Agents page and click &quot;Create Agent&quot;. Select a market, choose your
            outcome (Yes/No), set a price target and quantity, and pick a strategy. The agent
            starts in paper mode by default — no real funds are used until you switch to live.
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Paper vs Live mode</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            In <strong>paper mode</strong>, the agent simulates trades against the real order book
            without executing. This lets you test strategies risk-free. In <strong>live mode</strong>,
            the agent submits real orders to the venue using your connected credentials.
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Strategies</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Each agent runs a strategy that determines when and how to trade.
            See the <a href="/docs/guides/strategies" className="underline">Strategies guide</a> for
            details on momentum, mean-revert, and market-maker strategies.
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Execution guardrails</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Set safety limits on each agent: <strong>max notional per execution</strong> caps the
            dollar value of any single trade, <strong>max daily spend</strong> limits total USDC
            spent in a rolling 24-hour window, and <strong>max slippage</strong> rejects trades
            that would move the price too far from the target.
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Monitoring performance</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            The agent detail page shows execution history, P&L, win rate, and strategy signals.
            Failed executions are logged with reasons. Agents auto-deactivate after too many
            consecutive failures to protect your capital.
          </p>
        </Card>
      </div>
    </>
  );
}
