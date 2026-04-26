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

const RELAY_UTILITY = `curl https://relay44-api.onrender.com/v1/protocol/relay-utility | jq`;

const CHECK_STAKING_TIER = `import { createPublicClient, http } from 'viem';
import { base } from 'viem/chains';
import {
  qualifyX402OnChain,
  priceForX402Tier,
} from '@relay44/agent-sdk';

const client = createPublicClient({
  chain: base,
  transport: http('https://mainnet.base.org'),
});

const qualification = await qualifyX402OnChain({
  client,
  network: 'production',
  wallet: '0xYourWallet',
});

console.log(qualification.tier.name);          // Bronze | Silver | Gold | Diamond
console.log(qualification.bypassesX402);        // true at Gold and above
console.log(qualification.x402DiscountBps);     // 0, 2500, or 10000

// Compute what an x402-priced endpoint would charge this wallet:
const breakdown = priceForX402Tier(2_500n, qualification);
console.log(breakdown.effectiveMicroUsdc);      // 0n at Gold+, 1875n at Silver`;

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
            5. Read RELAY utility metadata
          </h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Returns chain id, RELAY token state, total staked, the four-tier
            table with fee-discount bps and x402 bypass flags, and the reward
            distributor address. Use this to render a tier badge or to size
            x402 payments without hard-coding constants.
          </p>
          <div className="mt-4">
            <CodeBlock language="bash" code={RELAY_UTILITY} />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">
            6. Check a wallet&apos;s staking tier and x402 access
          </h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            The agent SDK reads <code>RelayStaking.getTier</code> on Base and
            returns the tier metadata, x402 bypass flag, and effective price
            for an x402-quoted endpoint. Equivalent to the server-side staking
            check the API performs before charging an agent.
          </p>
          <div className="mt-4">
            <CodeBlock language="typescript" code={CHECK_STAKING_TIER} />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">
            7. Authenticate for trading
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
            8. Read an order book
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
            9. Place an order
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
