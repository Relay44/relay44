import { StructuredData } from '@/components/seo/StructuredData';
import { EndpointGroup, type Endpoint } from '@/components/docs';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Agents API',
  description: 'Agent endpoints for Relay44 — create, configure, execute, and monitor automated trading agents across venues.',
  path: '/docs/api/agents',
  keywords: ['agents', 'trading agents', 'automated execution', 'paper trading'],
});

const groups: Array<{ title: string; description: string; endpoints: Endpoint[] }> = [
  {
    title: 'On-chain agents (EVM)',
    description: 'Agents registered on-chain via the AgentManager contract.',
    endpoints: [
      { method: 'GET', path: '/v1/evm/agents', description: 'List registered agents' },
      { method: 'GET', path: '/v1/evm/agents/{agent_id}', description: 'Agent details and metadata' },
      { method: 'POST', path: '/v1/evm/write/agents/create', description: 'Prepare a CreateAgent transaction', auth: true },
      { method: 'POST', path: '/v1/evm/write/agents/execute', description: 'Prepare an ExecuteAgent transaction', auth: true },
      { method: 'POST', path: '/v1/evm/write/agents/update', description: 'Prepare an UpdateAgent transaction', auth: true },
      { method: 'POST', path: '/v1/evm/write/agents/deactivate', description: 'Prepare a DeactivateAgent transaction', auth: true },
      { method: 'POST', path: '/v1/evm/write/agents/manager', description: 'Set agent manager address', auth: true },
      { method: 'POST', path: '/v1/evm/write/agents/manager-approval', description: 'Approve manager for agent operations', auth: true },
      { method: 'POST', path: '/v1/evm/write/agents/bootstrap-create', description: 'Create agent with bootstrap configuration', auth: true },
    ],
  },
  {
    title: 'External agents',
    description: 'Agents that execute on external venues (Polymarket, Limitless, Aerodrome).',
    endpoints: [
      { method: 'GET', path: '/v1/external/agents', description: 'List your external agents', auth: true },
      { method: 'POST', path: '/v1/external/agents', description: 'Create an external agent', auth: true },
      { method: 'PATCH', path: '/v1/external/agents/{agent_id}', description: 'Update agent config (price, quantity, strategy, guardrails)', auth: true },
      { method: 'POST', path: '/v1/external/agents/{agent_id}/execute', description: 'Manually trigger agent execution', auth: true },
    ],
  },
  {
    title: 'Public agent data',
    description: 'Public performance and listing data (no auth required).',
    endpoints: [
      { method: 'GET', path: '/v1/external/agents/public', description: 'Public agent directory' },
      { method: 'GET', path: '/v1/external/agents/public/performance', description: 'Public performance metrics' },
      { method: 'GET', path: '/v1/external/agents/performance', description: 'Your agents\' detailed performance', auth: true },
    ],
  },
  {
    title: 'Agent runner',
    description: 'Internal scheduler endpoints for agent tick execution.',
    endpoints: [
      { method: 'POST', path: '/v1/external/agents/runner/tick', description: 'Trigger one execution tick for all due agents', auth: true },
    ],
  },
];

export default function AgentsApiPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/api/agents', name: 'Agents API', description: 'Agent endpoints for Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'API', url: '/docs/api' },
            { name: 'Agents', url: '/docs/api/agents' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Agents API</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Agents are automated trading programs that execute strategies on prediction markets.
        Relay44 supports both on-chain agents (registered via smart contracts) and external agents
        that trade on connected venues. Agents can run in paper mode (simulated) or live mode.
      </p>

      <div className="mt-8 grid gap-4">
        {groups.map((g) => (
          <EndpointGroup key={g.title} {...g} />
        ))}
      </div>
    </>
  );
}
