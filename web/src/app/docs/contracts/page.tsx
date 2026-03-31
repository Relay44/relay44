import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Smart Contracts',
  description: 'Relay44 smart contracts on Base — MarketCore, OrderBook, AgentManager, ERC-8004 Identity, and Reputation contracts.',
  path: '/docs/contracts',
  keywords: ['smart contracts', 'base', 'solidity', 'erc-8004', 'on-chain'],
});

const contracts = [
  {
    name: 'MarketCore',
    description: 'Core prediction market contract. Handles market creation, outcome share minting, and resolution. Markets are created with a question, outcomes, and trading window.',
  },
  {
    name: 'OrderBook',
    description: 'On-chain order book for matching buy and sell orders. Supports limit orders with price-time priority. The matcher service calls the match function to execute trades.',
  },
  {
    name: 'AgentManager',
    description: 'Manages on-chain agent registrations. Agents are created with a strategy config and can be authorized to execute trades on behalf of the owner.',
  },
  {
    name: 'ERC-8004 Identity',
    description: 'Identity registry following the ERC-8004 standard. Wallets can register an on-chain identity with tiers and metadata. Used for reputation scoring and access control.',
  },
  {
    name: 'Reputation',
    description: 'Tracks trading reputation scores on-chain. Outcomes from resolved markets update the trader\'s reputation. Higher reputation unlocks access to premium features.',
  },
  {
    name: 'PayoutManager',
    description: 'Handles automated payout distribution for resolved markets. Winners can claim their payouts through this contract or the API wrapper.',
  },
];

export default function ContractsPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/contracts', name: 'Smart Contracts', description: 'Relay44 smart contracts on Base.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Contracts', url: '/docs/contracts' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Smart Contracts</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Relay44&apos;s smart contracts are deployed on Base (Ethereum L2). The contracts handle
        market settlement, order matching, identity, reputation, and agent management on-chain.
      </p>

      <div className="mt-8 grid gap-6">
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Network</h2>
          <div className="mt-3 overflow-hidden border border-border">
            {[
              { label: 'Chain', value: 'Base (Chain ID: 8453)' },
              { label: 'RPC', value: 'https://mainnet.base.org' },
              { label: 'Explorer', value: 'https://basescan.org' },
            ].map((row) => (
              <div key={row.label} className="flex items-center gap-4 border-b border-border px-4 py-3 last:border-b-0">
                <span className="w-24 text-xs uppercase tracking-widest text-text-muted">{row.label}</span>
                <code className="text-sm text-text-primary">{row.value}</code>
              </div>
            ))}
          </div>
        </Card>

        {contracts.map((contract) => (
          <Card key={contract.name} className="p-6">
            <h2 className="text-lg font-semibold text-text-primary">{contract.name}</h2>
            <p className="mt-2 text-sm leading-6 text-text-secondary">{contract.description}</p>
          </Card>
        ))}

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Non-custodial design</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            All write operations use a prepare-sign-submit pattern. The API returns an unsigned
            transaction that the user signs with their wallet. Relay44 never holds private keys
            or custodies funds. The relay endpoint can submit pre-signed meta-transactions for
            gasless execution.
          </p>
        </Card>
      </div>
    </>
  );
}
