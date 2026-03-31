import { StructuredData } from '@/components/seo/StructuredData';
import { CodeBlock } from '@/components/docs';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'WebSocket Integration',
  description: 'Relay44 WebSocket integration guide — connect, subscribe, handle messages, and implement reconnection logic.',
  path: '/docs/developers/websocket',
  keywords: ['websocket', 'real-time', 'integration', 'streaming'],
});

export default function WebSocketGuidePage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/developers/websocket', name: 'WebSocket Integration', description: 'WebSocket integration guide for Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Developers', url: '/docs/developers' },
            { name: 'WebSocket', url: '/docs/developers/websocket' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">WebSocket Integration</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Use the WebSocket connection for real-time market data, trade notifications, and platform
        events. This guide covers connection setup, subscription management, and reconnection.
      </p>

      <div className="mt-8 grid gap-6">
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Connection</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Connect to <code className="text-text-primary">wss://relay44.com/ws</code>. The server
            sends periodic ping messages to keep the connection alive.
          </p>
          <div className="mt-4">
            <CodeBlock
              language="javascript"
              code={`const ws = new WebSocket('wss://relay44.com/ws');

ws.onopen = () => {
  console.log('Connected');
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log(msg.type, msg.data);
};`}
            />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Subscribing to markets</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            After connecting, send a subscribe message to receive updates for specific markets.
            You can subscribe to multiple markets on the same connection.
          </p>
          <div className="mt-4">
            <CodeBlock
              language="javascript"
              code={`// Subscribe to a specific market
ws.send(JSON.stringify({
  channel: 'market',
  market_id: '0x1234...abcd'
}));`}
            />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Message handling</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            All messages have a <code className="text-text-primary">type</code> field. Handle each
            type to update your application state.
          </p>
          <div className="mt-4">
            <CodeBlock
              language="javascript"
              code={`ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);

  switch (msg.type) {
    case 'orderbook':
      updateOrderBook(msg.data);
      break;
    case 'trade':
      addTrade(msg.data);
      break;
    case 'position':
      updatePosition(msg.data);
      break;
    case 'market':
      updateMarketPrice(msg.data);
      break;
    case 'event':
      handlePlatformEvent(msg.data);
      break;
    case 'ping':
      // Keep-alive, no action needed
      break;
  }
};`}
            />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Reconnection</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Implement exponential backoff for reconnection to handle network interruptions gracefully.
          </p>
          <div className="mt-4">
            <CodeBlock
              language="javascript"
              code={`let retryDelay = 1000;

function connect() {
  const ws = new WebSocket('wss://relay44.com/ws');

  ws.onopen = () => {
    retryDelay = 1000; // Reset on success
    // Re-subscribe to channels
  };

  ws.onclose = () => {
    setTimeout(connect, retryDelay);
    retryDelay = Math.min(retryDelay * 2, 30000);
  };
}`}
            />
          </div>
        </Card>
      </div>
    </>
  );
}
