import { StructuredData } from '@/components/seo/StructuredData';
import { EndpointGroup, type Endpoint } from '@/components/docs';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'EVM / On-chain API',
  description: 'EVM and on-chain endpoints for Relay44 — transaction preparation, identity, reputation, validation, bootstrap, and matcher operations.',
  path: '/docs/api/evm',
  keywords: ['evm', 'on-chain', 'base', 'smart contracts', 'transactions'],
});

const groups: Array<{ title: string; description: string; endpoints: Endpoint[] }> = [
  {
    title: 'Identity (ERC-8004)',
    description: 'On-chain identity registration and tier management.',
    endpoints: [
      { method: 'GET', path: '/v1/evm/identity/{wallet}', description: 'Get identity profile for a wallet' },
      { method: 'POST', path: '/v1/evm/write/identity/register', description: 'Prepare a RegisterIdentity transaction', auth: true },
      { method: 'POST', path: '/v1/evm/write/identity/tier', description: 'Prepare a SetTier transaction', auth: true },
      { method: 'POST', path: '/v1/evm/write/identity/active', description: 'Prepare a SetActive transaction', auth: true },
    ],
  },
  {
    title: 'Reputation',
    description: 'On-chain reputation tracking.',
    endpoints: [
      { method: 'GET', path: '/v1/evm/reputation/{wallet}', description: 'Get reputation score for a wallet' },
      { method: 'POST', path: '/v1/evm/write/reputation/outcome', description: 'Record a reputation outcome', auth: true },
    ],
  },
  {
    title: 'Validation',
    description: 'Market resolution validation requests.',
    endpoints: [
      { method: 'GET', path: '/v1/evm/validation/{request_hash}', description: 'Get validation request status' },
      { method: 'POST', path: '/v1/evm/write/validation/request', description: 'Submit a validation request', auth: true },
      { method: 'POST', path: '/v1/evm/write/validation/response', description: 'Submit a validation response', auth: true },
    ],
  },
  {
    title: 'Matcher service',
    description: 'Order matching engine management.',
    endpoints: [
      { method: 'GET', path: '/v1/evm/matcher/health', description: 'Matcher service health' },
      { method: 'GET', path: '/v1/evm/matcher/stats', description: 'Matching statistics' },
      { method: 'POST', path: '/v1/evm/matcher/pause', description: 'Pause the matcher', auth: true },
      { method: 'POST', path: '/v1/evm/matcher/resume', description: 'Resume the matcher', auth: true },
      { method: 'POST', path: '/v1/evm/matcher/report', description: 'Generate matcher report', auth: true },
    ],
  },
  {
    title: 'Payouts',
    description: 'Automated payout processing.',
    endpoints: [
      { method: 'GET', path: '/v1/evm/payouts/health', description: 'Payout service health' },
      { method: 'GET', path: '/v1/evm/payouts/candidates', description: 'Markets eligible for payout' },
      { method: 'GET', path: '/v1/evm/payouts/backlog', description: 'Pending payout queue' },
      { method: 'GET', path: '/v1/evm/payouts/jobs', description: 'Active payout jobs' },
      { method: 'POST', path: '/v1/evm/payouts/report', description: 'Generate payout report', auth: true },
    ],
  },
  {
    title: 'Indexer',
    description: 'Blockchain event indexing.',
    endpoints: [
      { method: 'GET', path: '/v1/evm/indexer/health', description: 'Indexer health and sync status' },
      { method: 'GET', path: '/v1/evm/indexer/lag', description: 'Block lag behind chain tip' },
      { method: 'POST', path: '/v1/evm/indexer/backfill', description: 'Trigger block range backfill', auth: true },
    ],
  },
  {
    title: 'Bootstrap',
    description: 'Market bootstrap and liquidity seeding.',
    endpoints: [
      { method: 'POST', path: '/v1/evm/internal/markets/{market_id}/bootstrap', description: 'Initialize market bootstrap', auth: true },
      { method: 'PATCH', path: '/v1/evm/internal/markets/{market_id}/bootstrap/runtime', description: 'Update bootstrap runtime config', auth: true },
      { method: 'POST', path: '/v1/evm/internal/markets/{market_id}/bootstrap/pause', description: 'Pause bootstrap', auth: true },
      { method: 'POST', path: '/v1/evm/internal/markets/{market_id}/bootstrap/resume', description: 'Resume bootstrap', auth: true },
      { method: 'POST', path: '/v1/evm/internal/markets/{market_id}/bootstrap/refresh', description: 'Refresh bootstrap state', auth: true },
      { method: 'POST', path: '/v1/evm/internal/markets/{market_id}/bootstrap/graduate', description: 'Graduate market from bootstrap', auth: true },
      { method: 'GET', path: '/v1/evm/bootstrap/operator', description: 'Bootstrap operator status' },
      { method: 'POST', path: '/v1/evm/bootstrap/admin/backfill', description: 'Backfill bootstrap data', auth: true },
      { method: 'POST', path: '/v1/evm/bootstrap/runner/tick', description: 'Trigger bootstrap tick', auth: true },
      { method: 'POST', path: '/v1/evm/bootstrap/runner/report', description: 'Generate bootstrap report', auth: true },
    ],
  },
  {
    title: 'Token and relay',
    description: 'Token state and meta-transaction relay.',
    endpoints: [
      { method: 'GET', path: '/v1/evm/token/state', description: 'Token contract state (supply, paused, etc.)' },
      { method: 'POST', path: '/v1/evm/write/relay', description: 'Relay a signed meta-transaction', auth: true },
    ],
  },
];

export default function EvmApiPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/api/evm', name: 'EVM / On-chain API', description: 'EVM and on-chain endpoints for Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'API', url: '/docs/api' },
            { name: 'EVM', url: '/docs/api/evm' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">EVM / On-chain API</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Relay44 runs on Base (Ethereum L2). The EVM API covers transaction preparation, identity
        management (ERC-8004), reputation, validation, order matching, payouts, indexing, and
        market bootstrap operations. Write endpoints return unsigned transactions for client-side signing.
      </p>

      <div className="mt-8 grid gap-4">
        {groups.map((g) => (
          <EndpointGroup key={g.title} {...g} />
        ))}
      </div>
    </>
  );
}
