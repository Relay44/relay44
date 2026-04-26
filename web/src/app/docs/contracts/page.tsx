import { StructuredData } from '@/components/seo/StructuredData';
import { CodeBlock } from '@/components/docs';
import { Card } from '@/components/ui';
import { CrossHostLink } from '@/components/layout/CrossHostLink';
import {
  buildBreadcrumbStructuredData,
  buildPageMetadata,
  buildWebPageStructuredData,
} from '@/lib/seo';
import {
  CONTRACT_METADATA,
  CONTRACT_ORDER,
  PROTOCOL_NETWORKS,
  basescanAddressUrl,
  type ContractName,
  type NetworkName,
} from '@/lib/protocol';

export const metadata = buildPageMetadata({
  title: 'Protocol Reference',
  description:
    'Live contract addresses, ABIs, and a viem integration example for the Relay44 Protocol on Base.',
  path: '/docs/contracts',
  image: '/docs/contracts/opengraph-image',
  keywords: [
    'smart contracts',
    'base',
    'solidity',
    'abi',
    'viem',
    'protocol reference',
    'relay44 protocol',
  ],
});

const CATEGORY_LABEL: Record<'core' | 'token' | 'agent' | 'identity', string> = {
  core: 'Core Protocol',
  token: 'RELAY Token',
  agent: 'Agent Runtime',
  identity: 'ERC-8004 Identity',
};

function AddressRow({
  name,
  network,
}: {
  name: ContractName;
  network: NetworkName;
}) {
  const address = PROTOCOL_NETWORKS[network].contracts[name];
  if (!address) {
    return (
      <div className="flex items-center gap-3 border-b border-border px-4 py-3 text-xs last:border-b-0">
        <span className="w-20 uppercase tracking-widest text-text-muted">
          {PROTOCOL_NETWORKS[network].label}
        </span>
        <span className="text-text-muted">Not deployed</span>
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-2 border-b border-border px-4 py-3 last:border-b-0 sm:flex-row sm:items-center">
      <span className="w-20 shrink-0 text-[0.7rem] uppercase tracking-widest text-text-muted">
        {PROTOCOL_NETWORKS[network].label}
      </span>
      <code className="flex-1 overflow-x-auto font-mono text-xs text-text-primary sm:text-sm">
        {address}
      </code>
      <a
        href={basescanAddressUrl(address, network)}
        target="_blank"
        rel="noreferrer"
        className="text-[0.7rem] uppercase tracking-widest text-text-muted transition-colors hover:text-text-primary"
      >
        Basescan →
      </a>
    </div>
  );
}

function ContractCard({ name }: { name: ContractName }) {
  const meta = CONTRACT_METADATA[name];
  return (
    <Card className="p-6" id={name}>
      <div className="flex flex-col gap-1 sm:flex-row sm:items-baseline sm:justify-between">
        <div>
          <p className="text-[0.65rem] uppercase tracking-[0.2em] text-text-muted">
            {CATEGORY_LABEL[meta.category]}
          </p>
          <h3 className="mt-1 text-lg font-semibold text-text-primary">{meta.label}</h3>
        </div>
        {meta.abiKey ? (
          <div className="flex items-center gap-3 text-[0.7rem] uppercase tracking-widest text-text-muted">
            <a
              href={`/api/contracts/${meta.abiKey}/abi`}
              target="_blank"
              rel="noreferrer"
              className="transition-colors hover:text-text-primary"
            >
              ABI ↗
            </a>
          </div>
        ) : null}
      </div>
      <p className="mt-3 text-sm leading-6 text-text-secondary">{meta.description}</p>
      <div className="mt-4 overflow-hidden border border-border">
        <AddressRow name={name} network="production" />
        <AddressRow name={name} network="staging" />
      </div>
    </Card>
  );
}

const INTEGRATION_EXAMPLE = `import { createPublicClient, http, parseAbi } from 'viem';
import { base } from 'viem/chains';

// Addresses are published on this page. Pull the ABI directly from the
// protocol reference endpoint, or paste the minimal one below.
//   curl https://relay44.com/api/contracts/market-core/abi
const MARKET_CORE = '0xc9259a18696Ecbf7636C1a01F40Bc9d47e249AE8';

const marketCoreAbi = parseAbi([
  'function marketCount() view returns (uint256)',
  'function markets(uint256 marketId) view returns (bytes32 questionHash, uint64 closeTime, uint64 resolveTime, address resolver, bool resolved, bool outcome)',
  'function getMarketMetadata(uint256 marketId) view returns (string question, string description, string category, string resolutionSource)',
]);

const client = createPublicClient({
  chain: base,
  transport: http(),
});

async function readLatestMarket() {
  const count = await client.readContract({
    address: MARKET_CORE,
    abi: marketCoreAbi,
    functionName: 'marketCount',
  });

  if (count === 0n) return null;

  const latestId = count - 1n;

  const [market, metadata] = await Promise.all([
    client.readContract({
      address: MARKET_CORE,
      abi: marketCoreAbi,
      functionName: 'markets',
      args: [latestId],
    }),
    client.readContract({
      address: MARKET_CORE,
      abi: marketCoreAbi,
      functionName: 'getMarketMetadata',
      args: [latestId],
    }),
  ]);

  return { id: latestId, market, metadata };
}

readLatestMarket().then(console.log);
`;

const CURL_EXAMPLE = `# Fetch the OrderBook ABI as JSON
curl https://relay44.com/api/contracts/order-book/abi | jq

# Available ABIs:
#   market-core    MarketCore
#   order-book     OrderBook
#   relay-staking  RelayStaking
#   erc20          Standard ERC20 (for RELAY, USDC)
`;

export default function ContractsPage() {
  const prod = PROTOCOL_NETWORKS.production;
  const staging = PROTOCOL_NETWORKS.staging;

  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/docs/contracts',
            name: 'Protocol Reference',
            description:
              'Live contract addresses, ABIs, and integration example for Relay44 Protocol on Base.',
          }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Protocol Reference', url: '/docs/contracts' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">
        Protocol Reference
      </h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Relay44 is open infrastructure for prediction markets on Base. This page is the
        canonical reference for the deployed protocol — mainnet and sepolia addresses,
        ABIs, and a minimal integration example you can drop into any viem, ethers, or
        wagmi project.
      </p>

      <div className="mt-8 grid gap-6">
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Network</h2>
          <div className="mt-3 grid gap-4 sm:grid-cols-2">
            {[prod, staging].map((net) => (
              <div key={net.label} className="border border-border">
                <div className="border-b border-border bg-bg-secondary px-4 py-2">
                  <p className="text-[0.65rem] uppercase tracking-[0.2em] text-text-muted">
                    {net.label}
                  </p>
                  <p className="text-sm font-semibold text-text-primary">{net.chain}</p>
                </div>
                <div className="px-4 py-3 text-xs text-text-secondary">
                  <div className="flex justify-between gap-2">
                    <span className="text-text-muted">Chain ID</span>
                    <code className="text-text-primary">{net.chainId}</code>
                  </div>
                  <div className="mt-1 flex justify-between gap-2">
                    <span className="text-text-muted">RPC</span>
                    <code className="truncate text-text-primary">{net.rpc}</code>
                  </div>
                  <div className="mt-1 flex justify-between gap-2">
                    <span className="text-text-muted">Explorer</span>
                    <a
                      href={net.explorer}
                      target="_blank"
                      rel="noreferrer"
                      className="text-text-primary underline-offset-2 hover:underline"
                    >
                      basescan.org
                    </a>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Quick start — viem</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Read the latest market from MarketCore on Base mainnet. Addresses are
            published below; full ABIs are served over HTTP for any language.
          </p>
          <div className="mt-4">
            <CodeBlock language="typescript" code={INTEGRATION_EXAMPLE} />
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">ABIs</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Core protocol ABIs are served as JSON so you can pull them into any
            toolchain without cloning the monorepo.
          </p>
          <div className="mt-4">
            <CodeBlock language="bash" code={CURL_EXAMPLE} />
          </div>
        </Card>

        <div className="pt-2">
          <h2 className="text-lg font-semibold text-text-primary">Contracts</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            All contracts are deployed from the `evm/src` workspace in the open-source monorepo.
          </p>
        </div>

        {CONTRACT_ORDER.map((name) => (
          <ContractCard key={name} name={name} />
        ))}

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Non-custodial design</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            All write operations use a prepare-sign-submit pattern. The Relay44 API
            returns an unsigned transaction that the user signs with their own wallet.
            Relay44 never holds private keys or custodies collateral. An optional relay
            endpoint can forward pre-signed meta-transactions for gasless execution.
          </p>
          <p className="mt-3 text-sm leading-6 text-text-secondary">
            Want to build on the protocol directly? See{' '}
            <CrossHostLink
              href="/tokenomics"
              className="text-text-primary underline-offset-2 hover:underline"
            >
              /tokenomics
            </CrossHostLink>{' '}
            for how fees and rewards flow back to stakers, agents, and creators, or
            jump straight to the{' '}
            <a
              href="/docs/developers/quickstart"
              className="text-text-primary underline-offset-2 hover:underline"
            >
              API quickstart
            </a>
            .
          </p>
        </Card>
      </div>
    </>
  );
}
