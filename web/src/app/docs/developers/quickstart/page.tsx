import { StructuredData } from '@/components/seo/StructuredData';
import { CodeBlock } from '@/components/docs';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Quickstart',
  description: 'Relay44 API quickstart — authenticate, fetch markets, and place your first order with curl.',
  path: '/docs/developers/quickstart',
  keywords: ['quickstart', 'curl', 'api examples', 'first request'],
});

export default function QuickstartPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/developers/quickstart', name: 'Quickstart', description: 'Relay44 API quickstart.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Developers', url: '/docs/developers' },
            { name: 'Quickstart', url: '/docs/developers/quickstart' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Quickstart</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Get up and running with the Relay44 API in under 5 minutes using curl.
      </p>

      <div className="mt-8 grid gap-6">
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">1. Check health</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Verify the API is reachable.
          </p>
          <div className="mt-4">
            <CodeBlock language="bash" code="curl https://relay44.com/health" />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">2. List markets</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Fetch the first 10 markets from all sources.
          </p>
          <div className="mt-4">
            <CodeBlock
              language="bash"
              code="curl 'https://relay44.com/v1/evm/markets?limit=10&offset=0&source=all'"
            />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">3. Get a nonce</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Start the SIWE authentication flow by requesting a nonce.
          </p>
          <div className="mt-4">
            <CodeBlock language="bash" code="curl https://relay44.com/v1/auth/siwe/nonce" />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">4. Authenticate</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Sign the SIWE message with your wallet and submit the signature.
          </p>
          <div className="mt-4">
            <CodeBlock
              language="bash"
              code={`curl -X POST https://relay44.com/v1/auth/siwe/login \\
  -H 'Content-Type: application/json' \\
  -d '{"message": "<siwe-message>", "signature": "<hex-signature>"}'`}
            />
          </div>
          <p className="mt-3 text-xs text-text-muted">
            The response includes <code>access_token</code> and <code>refresh_token</code>.
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">5. Fetch order book</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Get the order book for a specific market.
          </p>
          <div className="mt-4">
            <CodeBlock
              language="bash"
              code={`curl 'https://relay44.com/v1/evm/markets/<market_id>/orderbook?outcome=yes&depth=20'`}
            />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">6. Place an order</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Place a limit order using your JWT.
          </p>
          <div className="mt-4">
            <CodeBlock
              language="bash"
              code={`curl -X POST https://relay44.com/v1/orders \\
  -H 'Authorization: Bearer <access_token>' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "market_id": "<market_id>",
    "outcome": "yes",
    "side": "buy",
    "price": 0.55,
    "quantity": 100
  }'`}
            />
          </div>
        </Card>
      </div>
    </>
  );
}
