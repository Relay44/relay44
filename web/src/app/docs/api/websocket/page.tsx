import { StructuredData } from '@/components/seo/StructuredData';
import { CodeBlock } from '@/components/docs';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'WebSocket API',
  description: 'WebSocket real-time events for Relay44 — order book updates, trades, position changes, market status, and platform events.',
  path: '/docs/api/websocket',
  keywords: ['websocket', 'real-time', 'streaming', 'events'],
});

const messageTypes = [
  { type: 'orderbook', description: 'Order book level update for a market', fields: 'market_id, outcome, side, price, quantity, timestamp' },
  { type: 'trade', description: 'New trade executed', fields: 'market_id, outcome, price, quantity, buyer, seller, timestamp' },
  { type: 'position', description: 'Position balance change for a user', fields: 'market_id, owner, yes_balance, no_balance, timestamp' },
  { type: 'market', description: 'Market price or status change', fields: 'market_id, yes_price, no_price, status, timestamp' },
  { type: 'event', description: 'Platform event (agent execution, lifecycle)', fields: 'Raw JSON from the event bus' },
  { type: 'ping', description: 'Keep-alive ping', fields: 'None' },
];

export default function WebSocketApiPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/api/websocket', name: 'WebSocket API', description: 'WebSocket events for Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'API', url: '/docs/api' },
            { name: 'WebSocket', url: '/docs/api/websocket' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">WebSocket API</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Connect to the WebSocket endpoint for real-time market data, trade notifications, and
        platform events. The connection supports both market-specific subscriptions and a global
        feed of all updates.
      </p>

      <div className="mt-8 grid gap-4">
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Connection</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Connect to the WebSocket endpoint at <code className="text-text-primary">/ws</code>.
            The connection accepts JSON messages for subscription management.
          </p>
          <div className="mt-4">
            <CodeBlock language="bash" code="wscat -c wss://relay44.com/ws" />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Subscribe to a market</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Send a subscribe request to receive updates for a specific market.
          </p>
          <div className="mt-4">
            <CodeBlock
              language="json"
              code={`{
  "channel": "market",
  "market_id": "0x1234...abcd"
}`}
            />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Message types</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            All messages are JSON objects with a <code className="text-text-primary">type</code> field
            and a <code className="text-text-primary">data</code> payload.
          </p>
          <div className="mt-4 overflow-hidden border border-border">
            {messageTypes.map((msg) => (
              <div
                key={msg.type}
                className="grid gap-2 border-b border-border px-4 py-3 last:border-b-0 md:grid-cols-[8rem_minmax(0,1fr)]"
              >
                <code className="text-sm font-medium text-text-primary">{msg.type}</code>
                <div className="min-w-0">
                  <p className="text-sm text-text-secondary">{msg.description}</p>
                  <p className="mt-1 text-xs text-text-muted">{msg.fields}</p>
                </div>
              </div>
            ))}
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Example message</h2>
          <div className="mt-4">
            <CodeBlock
              language="json"
              code={`{
  "type": "trade",
  "data": {
    "market_id": "0x1234...abcd",
    "outcome": "yes",
    "price": 0.65,
    "quantity": 100,
    "buyer": "0xBuyer...",
    "seller": "0xSeller...",
    "timestamp": 1711900000
  }
}`}
            />
          </div>
        </Card>
      </div>
    </>
  );
}
