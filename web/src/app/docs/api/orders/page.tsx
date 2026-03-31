import { StructuredData } from '@/components/seo/StructuredData';
import { EndpointGroup, type Endpoint } from '@/components/docs';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Orders API',
  description: 'Order endpoints for Relay44 — place, list, cancel, and match orders on prediction markets.',
  path: '/docs/api/orders',
  keywords: ['orders', 'trading', 'limit orders', 'cancel orders'],
});

const groups: Array<{ title: string; description: string; endpoints: Endpoint[] }> = [
  {
    title: 'Order management',
    description: 'CRUD operations on orders.',
    endpoints: [
      { method: 'GET', path: '/v1/orders', description: 'List your open and filled orders', auth: true },
      { method: 'POST', path: '/v1/orders', description: 'Place a new limit order', auth: true },
      { method: 'GET', path: '/v1/orders/{order_id}', description: 'Get order details and fill status', auth: true },
      { method: 'DELETE', path: '/v1/orders/{order_id}', description: 'Cancel a pending order', auth: true },
    ],
  },
  {
    title: 'On-chain order preparation',
    description: 'Prepare unsigned transactions for on-chain order execution.',
    endpoints: [
      { method: 'POST', path: '/v1/evm/write/orders/place', description: 'Prepare a PlaceOrder transaction', auth: true },
      { method: 'POST', path: '/v1/evm/write/orders/cancel', description: 'Prepare a CancelOrder transaction', auth: true },
      { method: 'POST', path: '/v1/evm/write/orders/match', description: 'Prepare a MatchOrders transaction (matcher only)', auth: true },
    ],
  },
  {
    title: 'External venue orders',
    description: 'Place and manage orders on connected external venues (Polymarket, Limitless).',
    endpoints: [
      { method: 'POST', path: '/v1/external/orders/intent', description: 'Create an order intent for external execution', auth: true },
      { method: 'POST', path: '/v1/external/orders/submit', description: 'Submit an order to the external venue', auth: true },
      { method: 'POST', path: '/v1/external/orders/prepare-submit', description: 'Combined prepare + submit in one call', auth: true },
      { method: 'POST', path: '/v1/external/orders/cancel', description: 'Cancel an external order', auth: true },
      { method: 'POST', path: '/v1/external/orders/prepare-cancel', description: 'Prepare + cancel in one call', auth: true },
      { method: 'GET', path: '/v1/external/orders', description: 'List external orders', auth: true },
    ],
  },
];

export default function OrdersApiPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/api/orders', name: 'Orders API', description: 'Order endpoints for Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'API', url: '/docs/api' },
            { name: 'Orders', url: '/docs/api/orders' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Orders API</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Orders represent intent to buy or sell outcome shares at a given price. Relay44 supports
        both on-chain orders (matched by the matcher service) and external venue orders
        routed through connected credentials.
      </p>

      <div className="mt-8 grid gap-4">
        {groups.map((g) => (
          <EndpointGroup key={g.title} {...g} />
        ))}
      </div>
    </>
  );
}
