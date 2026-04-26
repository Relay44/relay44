import { StructuredData } from '@/components/seo/StructuredData';
import { CodeBlock } from '@/components/docs';
import { Card } from '@/components/ui';
import {
  buildBreadcrumbStructuredData,
  buildPageMetadata,
  buildWebPageStructuredData,
} from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Quickstart',
  description:
    'Relay44 Protocol quickstart: install @relay44/protocol, read contracts on Base, fetch markets, and authenticate for trading.',
  path: '/docs/developers/quickstart',
  keywords: ['quickstart', 'protocol package', 'viem', 'api examples', 'base'],
});

const INSTALL = `npm install @relay44/protocol viem`;

const READ_MARKET_CORE = `import { createPublicClient, http } from 'viem';
import { base } from 'viem/chains';
import { getContractAddress, marketCoreAbi } from '@relay44/protocol';

const client = createPublicClient({
  chain: base,
  transport: http('https://mainnet.base.org'),
});

const marketCount = await client.readContract({
  address: getContractAddress('production', 'marketCore'),
  abi: marketCoreAbi,
  functionName: 'marketCount',
});

console.log({ marketCount });`;

const FETCH_MARKETS = `curl 'https://relay44-api.onrender.com/v1/evm/markets?limit=10&offset=0&source=all'`;

const PROTOCOL_METRICS = `curl https://relay44-api.onrender.com/v1/protocol/metrics | jq`;

const AUTHENTICATE = `curl https://relay44-api.onrender.com/v1/auth/siwe/nonce

curl -X POST https://relay44-api.onrender.com/v1/auth/siwe/login \\
  -H 'Content-Type: application/json' \\
  -d '{
    "wallet": "0xYourWallet",
    "message": "<siwe-message>",
    "signature": "<hex-signature>"
  }'`;

const ORDERBOOK = `curl 'https://relay44-api.onrender.com/v1/evm/markets/<market_id>/orderbook?outcome=yes&depth=20'`;

const PLACE_ORDER = `curl -X POST https://relay44-api.onrender.com/v1/orders \\
  -H 'Authorization: Bearer <access_token>' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "market_id": "<market_id>",
    "outcome": "yes",
    "side": "buy",
    "price": 0.55,
    "quantity": 100
  }'`;

export default function QuickstartPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/docs/developers/quickstart',
            name: 'Quickstart',
            description: 'Relay44 Protocol quickstart.',
          }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Developers', url: '/docs/developers' },
            { name: 'Quickstart', url: '/docs/developers/quickstart' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">
        Quickstart
      </h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Start with importable protocol artifacts, then use the public API for
        market data, metrics, authentication, and order flow.
      </p>

      <div className="mt-8 grid gap-6">
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">
            1. Install protocol artifacts
          </h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            `@relay44/protocol` exports production and staging addresses,
            generated ABIs from `evm/out`, and typed helpers.
          </p>
          <div className="mt-4">
            <CodeBlock language="bash" code={INSTALL} />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">
            2. Read MarketCore on Base
          </h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            This verifies the contract package, manifest, ABI, and Base RPC path.
          </p>
          <div className="mt-4">
            <CodeBlock language="typescript" code={READ_MARKET_CORE} />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">
            3. Fetch public markets
          </h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Read unified markets from the production API.
          </p>
          <div className="mt-4">
            <CodeBlock language="bash" code={FETCH_MARKETS} />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">
            4. Read protocol metrics
          </h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            The public protocol dashboard is backed by the same endpoint.
          </p>
          <div className="mt-4">
            <CodeBlock language="bash" code={PROTOCOL_METRICS} />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">
            5. Authenticate for trading
          </h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Trading requires a SIWE JWT. The API never takes custody of keys.
          </p>
          <div className="mt-4">
            <CodeBlock language="bash" code={AUTHENTICATE} />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">
            6. Read an order book
          </h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Market data is public and works without wallet auth.
          </p>
          <div className="mt-4">
            <CodeBlock language="bash" code={ORDERBOOK} />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">
            7. Place an order
          </h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Use the JWT from SIWE auth. Wallet-signed EVM write flows are exposed
            separately under the EVM write API.
          </p>
          <div className="mt-4">
            <CodeBlock language="bash" code={PLACE_ORDER} />
          </div>
        </Card>
      </div>
    </>
  );
}
