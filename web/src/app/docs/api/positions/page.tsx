import { StructuredData } from '@/components/seo/StructuredData';
import { EndpointGroup, type Endpoint } from '@/components/docs';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Positions API',
  description: 'Position endpoints for Relay44 — view holdings, track P&L, and claim winnings from resolved markets.',
  path: '/docs/api/positions',
  keywords: ['positions', 'holdings', 'portfolio', 'claim winnings'],
});

const groups: Array<{ title: string; description: string; endpoints: Endpoint[] }> = [
  {
    title: 'Position queries',
    description: 'View your outcome share holdings and unrealized P&L.',
    endpoints: [
      { method: 'GET', path: '/v1/positions', description: 'List all your positions across markets', auth: true },
      { method: 'GET', path: '/v1/positions/{market_id}', description: 'Position details for a specific market', auth: true },
    ],
  },
  {
    title: 'Claim winnings',
    description: 'Claim your payout from resolved markets.',
    endpoints: [
      { method: 'POST', path: '/v1/positions/{market_id}/claim', description: 'Claim winnings for a resolved market', auth: true },
      { method: 'POST', path: '/v1/evm/write/positions/claim', description: 'Prepare a Claim transaction for signing', auth: true },
      { method: 'POST', path: '/v1/evm/write/positions/claim-for', description: 'Prepare a ClaimFor transaction (claim on behalf of another wallet)', auth: true },
    ],
  },
];

export default function PositionsApiPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/api/positions', name: 'Positions API', description: 'Position endpoints for Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'API', url: '/docs/api' },
            { name: 'Positions', url: '/docs/api/positions' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Positions API</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Positions represent your outcome share holdings in a market. When a market resolves,
        holders of the winning outcome can claim their payout. Positions are tracked both on-chain
        and via the API for convenience.
      </p>

      <div className="mt-8 grid gap-4">
        {groups.map((g) => (
          <EndpointGroup key={g.title} {...g} />
        ))}
      </div>
    </>
  );
}
