import Link from 'next/link';
import { StructuredData } from '@/components/seo/StructuredData';
import { EndpointGroup, type Endpoint } from '@/components/docs';
import {
  buildBreadcrumbStructuredData,
  buildPageMetadata,
  buildWebPageStructuredData,
} from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'API Reference',
  description:
    'Complete API reference for Relay44 — 147 endpoints across markets, orders, agents, decisions, authentication, and on-chain operations.',
  path: '/docs/api',
  keywords: ['api', 'developer docs', 'market endpoints', 'web4 capabilities', 'rest api'],
});

const groups: Array<{ title: string; description: string; endpoints: Endpoint[] }> = [
  {
    title: 'Health and runtime',
    description: 'Confirm the deployment is live and inspect current runtime mode.',
    endpoints: [
      { method: 'GET', path: '/health', description: 'Basic web-service health check' },
      { method: 'GET', path: '/health/detailed', description: 'Runtime checks and provider status' },
      { method: 'GET', path: '/metrics', description: 'Internal metrics' },
      { method: 'GET', path: '/metrics/prometheus', description: 'Prometheus-format metrics' },
    ],
  },
  {
    title: 'Markets (read)',
    description: 'Read market listings, order books, and recent trades from the unified feed.',
    endpoints: [
      { method: 'GET', path: '/v1/markets', description: 'List markets with pagination and filters' },
      { method: 'GET', path: '/v1/markets/{market_id}', description: 'Single market snapshot' },
      { method: 'GET', path: '/v1/markets/{market_id}/orderbook', description: 'Best bid/ask levels for one outcome' },
      { method: 'GET', path: '/v1/markets/{market_id}/trades', description: 'Recent fills and trades' },
      { method: 'GET', path: '/v1/evm/markets', description: 'Unified feed including external sources' },
      { method: 'GET', path: '/v1/evm/markets/{market_id}', description: 'Single market with EVM data' },
      { method: 'GET', path: '/v1/evm/markets/{market_id}/orderbook', description: 'Order book with on-chain depth' },
      { method: 'GET', path: '/v1/evm/markets/{market_id}/trades', description: 'Trade history' },
    ],
  },
  {
    title: 'Orders',
    description: 'Place, list, and cancel orders.',
    endpoints: [
      { method: 'GET', path: '/v1/orders', description: 'List your open orders', auth: true },
      { method: 'POST', path: '/v1/orders', description: 'Place a new order', auth: true },
      { method: 'GET', path: '/v1/orders/{order_id}', description: 'Get order details', auth: true },
      { method: 'DELETE', path: '/v1/orders/{order_id}', description: 'Cancel an order', auth: true },
    ],
  },
  {
    title: 'Positions',
    description: 'Track and claim outcome positions.',
    endpoints: [
      { method: 'GET', path: '/v1/positions', description: 'List your positions', auth: true },
      { method: 'GET', path: '/v1/positions/{market_id}', description: 'Position in a specific market', auth: true },
      { method: 'POST', path: '/v1/positions/{market_id}/claim', description: 'Claim winnings from a resolved market', auth: true },
    ],
  },
];

export default function ApiDocsPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/docs/api',
            name: 'Relay44 API Reference',
            description:
              'Complete API reference for Relay44 — 147 endpoints across markets, orders, agents, decisions, authentication, and on-chain operations.',
          }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'API Reference', url: '/docs/api' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">API Reference</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Relay44 exposes 147 endpoints across 16 API scopes. This overview covers the core read
        and write routes. See the sub-pages for detailed endpoint documentation per domain.
      </p>

      <div className="mt-8 grid gap-4">
        {groups.map((g) => (
          <EndpointGroup key={g.title} title={g.title} description={g.description} endpoints={g.endpoints} />
        ))}
      </div>

      <div className="mt-8 grid gap-3 sm:grid-cols-2">
        {[
          { label: 'Auth endpoints', href: '/docs/api/auth' },
          { label: 'Agent endpoints', href: '/docs/api/agents' },
          { label: 'Decision endpoints', href: '/docs/api/decisions' },
          { label: 'EVM / On-chain', href: '/docs/api/evm' },
          { label: 'WebSocket events', href: '/docs/api/websocket' },
        ].map((link) => (
          <Link
            key={link.href}
            href={link.href}
            className="inline-flex h-10 items-center border border-border px-4 text-sm text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
          >
            {link.label} &rarr;
          </Link>
        ))}
      </div>
    </>
  );
}
