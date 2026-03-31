import { StructuredData } from '@/components/seo/StructuredData';
import { EndpointGroup, type Endpoint } from '@/components/docs';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Decisions API',
  description: 'Decision cell endpoints for Relay44 — create decision workflows, attach markets and agents, set automation rules and alerts.',
  path: '/docs/api/decisions',
  keywords: ['decisions', 'decision cells', 'automation', 'workflows'],
});

const groups: Array<{ title: string; description: string; endpoints: Endpoint[] }> = [
  {
    title: 'Decision cells',
    description: 'Create and manage decision cell graphs.',
    endpoints: [
      { method: 'GET', path: '/v1/decisions', description: 'List your decision cells', auth: true },
      { method: 'POST', path: '/v1/decisions', description: 'Create a new decision cell', auth: true },
      { method: 'GET', path: '/v1/decisions/{cell_id}', description: 'Get cell details with nodes', auth: true },
      { method: 'PATCH', path: '/v1/decisions/{cell_id}', description: 'Update cell metadata', auth: true },
    ],
  },
  {
    title: 'Nodes',
    description: 'Add and configure nodes within a decision cell.',
    endpoints: [
      { method: 'POST', path: '/v1/decisions/{cell_id}/nodes', description: 'Add a node to the cell', auth: true },
      { method: 'PATCH', path: '/v1/decisions/{cell_id}/nodes/{node_id}', description: 'Update node configuration', auth: true },
      { method: 'POST', path: '/v1/decisions/{cell_id}/nodes/{node_id}/attach-market', description: 'Attach a market to a node', auth: true },
      { method: 'POST', path: '/v1/decisions/{cell_id}/nodes/{node_id}/attach-agent', description: 'Attach an agent to a node', auth: true },
    ],
  },
  {
    title: 'Automation and alerts',
    description: 'Configure automated actions and notification triggers.',
    endpoints: [
      { method: 'POST', path: '/v1/decisions/{cell_id}/automation', description: 'Set automation rules for the cell', auth: true },
      { method: 'POST', path: '/v1/decisions/{cell_id}/alerts', description: 'Configure alert triggers', auth: true },
      { method: 'POST', path: '/v1/decisions/{cell_id}/recalculate', description: 'Force recalculation of cell state', auth: true },
      { method: 'POST', path: '/v1/decisions/{cell_id}/actions', description: 'Execute an action on the cell', auth: true },
    ],
  },
  {
    title: 'Events and runner',
    description: 'View cell events and trigger automation ticks.',
    endpoints: [
      { method: 'GET', path: '/v1/decisions/{cell_id}/events', description: 'List events for a decision cell', auth: true },
      { method: 'POST', path: '/v1/decisions/runner/tick', description: 'Trigger automation evaluation for all cells', auth: true },
    ],
  },
];

export default function DecisionsApiPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/api/decisions', name: 'Decisions API', description: 'Decision cell endpoints for Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'API', url: '/docs/api' },
            { name: 'Decisions', url: '/docs/api/decisions' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Decisions API</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Decision cells are graph-based workflows that combine market signals, agent execution,
        and automation rules into structured decision-making processes. Each cell contains nodes
        that can be linked to markets and agents with configurable triggers.
      </p>

      <div className="mt-8 grid gap-4">
        {groups.map((g) => (
          <EndpointGroup key={g.title} {...g} />
        ))}
      </div>
    </>
  );
}
