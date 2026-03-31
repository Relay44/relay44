import { StructuredData } from '@/components/seo/StructuredData';
import { EndpointGroup, type Endpoint } from '@/components/docs';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Markets API',
  description: 'Market endpoints for Relay44 — list, create, and query prediction markets, order books, and trade history.',
  path: '/docs/api/markets',
  keywords: ['markets', 'prediction markets', 'orderbook', 'trades'],
});

const groups: Array<{ title: string; description: string; endpoints: Endpoint[] }> = [
  {
    title: 'Market listing',
    description: 'Browse and search markets from internal and external sources.',
    endpoints: [
      { method: 'GET', path: '/v1/markets', description: 'List markets with limit, offset, and source filters' },
      { method: 'GET', path: '/v1/markets/{market_id}', description: 'Get a single market snapshot with pricing' },
      { method: 'GET', path: '/v1/evm/markets', description: 'Unified feed including Polymarket, Limitless, and internal markets' },
      { method: 'GET', path: '/v1/evm/markets/{market_id}', description: 'Market detail with on-chain state' },
    ],
  },
  {
    title: 'Market creation',
    description: 'Create new prediction markets on-chain.',
    endpoints: [
      { method: 'POST', path: '/v1/markets', description: 'Create a new market (prepares unsigned tx)', auth: true },
      { method: 'POST', path: '/v1/evm/write/markets/create', description: 'Prepare a CreateMarket transaction for signing', auth: true },
      { method: 'POST', path: '/v1/evm/write/markets/resolve', description: 'Prepare a ResolveMarket transaction', auth: true },
    ],
  },
  {
    title: 'Order book and trades',
    description: 'Query order book depth and recent trade history for any market.',
    endpoints: [
      { method: 'GET', path: '/v1/markets/{market_id}/orderbook', description: 'Order book with bid/ask levels' },
      { method: 'GET', path: '/v1/markets/{market_id}/trades', description: 'Recent trades with price and quantity' },
      { method: 'GET', path: '/v1/evm/markets/{market_id}/orderbook', description: 'On-chain order book with depth parameter' },
      { method: 'GET', path: '/v1/evm/markets/{market_id}/trades', description: 'On-chain trade history' },
    ],
  },
];

export default function MarketsApiPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/api/markets', name: 'Markets API', description: 'Market endpoints for Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'API', url: '/docs/api' },
            { name: 'Markets', url: '/docs/api/markets' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Markets API</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Markets are the core entity in Relay44. Each market has a question, outcome shares (yes/no),
        an order book, and a lifecycle (open, trading, resolved). Markets can be internal (on-chain)
        or aggregated from external venues like Polymarket and Limitless.
      </p>

      <div className="mt-8 grid gap-4">
        {groups.map((g) => (
          <EndpointGroup key={g.title} {...g} />
        ))}
      </div>
    </>
  );
}
