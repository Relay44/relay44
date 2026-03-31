import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Decision Cells Guide',
  description: 'Build decision workflows on Relay44 — create cells, add nodes, attach markets and agents, set automation rules.',
  path: '/docs/guides/decisions',
  keywords: ['decisions', 'decision cells', 'workflows', 'automation'],
});

export default function DecisionsGuidePage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/guides/decisions', name: 'Decision Cells Guide', description: 'Build decision workflows on Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Guides', url: '/docs/guides' },
            { name: 'Decisions', url: '/docs/guides/decisions' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Decision Cells</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Decision cells are graph-based workflows that help you structure complex decisions using
        market signals and automated agent execution.
      </p>

      <div className="mt-8 grid gap-6">
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">What is a decision cell?</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            A decision cell is a container that holds a graph of connected nodes. Each node
            represents a factor in your decision. Nodes can be linked to prediction markets
            (for probability signals) and agents (for automated execution when conditions are met).
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Creating cells and nodes</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Create a cell from the Decisions page, then add nodes for each factor you want to
            track. Nodes can be weighted to reflect their importance. The cell automatically
            computes an aggregate score from its nodes.
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Attaching markets and agents</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Attach a market to a node to feed its probability signal into the decision graph.
            Attach an agent to automate execution when the cell&apos;s aggregate score crosses
            a threshold. This creates a closed loop: market data informs the decision, and the
            decision triggers agent action.
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Automation rules</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Set automation rules that define when agents should execute based on the cell state.
            Configure alerts to get notified when specific conditions are met. The automation
            runner evaluates cells periodically and triggers configured actions.
          </p>
        </Card>
      </div>
    </>
  );
}
