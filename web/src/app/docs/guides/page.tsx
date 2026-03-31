import Link from 'next/link';
import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'User Guides',
  description: 'Relay44 user guides — learn how to trade on prediction markets, manage agents, build decision workflows, and connect external venues.',
  path: '/docs/guides',
  keywords: ['guides', 'tutorials', 'getting started', 'how to'],
});

const guides = [
  { title: 'Getting Started', description: 'Connect your wallet, browse markets, and place your first trade.', href: '/docs/guides/getting-started' },
  { title: 'Markets', description: 'How prediction markets work — lifecycle, pricing, resolution, and outcome shares.', href: '/docs/guides/markets' },
  { title: 'Agents', description: 'Create and configure automated trading agents with strategies and guardrails.', href: '/docs/guides/agents' },
  { title: 'Decision Cells', description: 'Build decision workflows that combine market signals with agent execution.', href: '/docs/guides/decisions' },
  { title: 'Credentials', description: 'Connect external venues — Polymarket, Limitless, and Aerodrome.', href: '/docs/guides/credentials' },
  { title: 'Strategies', description: 'Understand momentum, mean-revert, and market-maker strategy types.', href: '/docs/guides/strategies' },
];

export default function GuidesIndexPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/guides', name: 'User Guides', description: 'Relay44 user guides.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Guides', url: '/docs/guides' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">User Guides</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Step-by-step guides to help you get the most out of Relay44.
      </p>

      <div className="mt-8 grid gap-4">
        {guides.map((guide) => (
          <Link key={guide.href} href={guide.href} className="block">
            <Card className="p-6 transition-colors hover:border-border-hover">
              <h2 className="text-lg font-semibold text-text-primary">{guide.title}</h2>
              <p className="mt-2 text-sm leading-6 text-text-secondary">{guide.description}</p>
            </Card>
          </Link>
        ))}
      </div>
    </>
  );
}
