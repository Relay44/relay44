import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Markets Guide',
  description: 'How prediction markets work on Relay44 — market lifecycle, pricing mechanics, resolution, and outcome shares.',
  path: '/docs/guides/markets',
  keywords: ['markets', 'prediction markets', 'pricing', 'resolution', 'outcome shares'],
});

export default function MarketsGuidePage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/guides/markets', name: 'Markets Guide', description: 'How prediction markets work on Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Guides', url: '/docs/guides' },
            { name: 'Markets', url: '/docs/guides/markets' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Markets</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Prediction markets let you trade on the probability of future events. Each market poses a
        yes/no question with shares priced between $0 and $1.
      </p>

      <div className="mt-8 grid gap-6">
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Market lifecycle</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Markets move through four stages: <strong>Created</strong> (market exists but trading
            hasn&apos;t started), <strong>Open</strong> (active trading), <strong>Closed</strong>
            (trading ended, awaiting resolution), and <strong>Resolved</strong> (outcome determined,
            payouts available).
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Pricing</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Share prices reflect the market&apos;s consensus probability. A Yes share at $0.70 means
            70% implied probability. The order book matches buyers and sellers — you set your own
            limit price. Market prices update in real-time via WebSocket.
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Outcome shares</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            When you buy Yes shares, you pay the current price per share. If the market resolves Yes,
            each share pays out $1. If it resolves No, your shares are worth $0. The inverse applies
            to No shares. Your profit is the payout minus what you paid.
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Resolution</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Markets are resolved by the market creator or through the on-chain validation process.
            Once resolved, holders of the winning outcome can claim their payout through the
            Positions page or the API.
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Multi-venue aggregation</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Relay44 aggregates markets from multiple sources: internal on-chain markets, Polymarket,
            Limitless, and Aerodrome. The unified feed shows all markets with consistent pricing
            and metadata. External markets are identified by their namespaced IDs.
          </p>
        </Card>
      </div>
    </>
  );
}
