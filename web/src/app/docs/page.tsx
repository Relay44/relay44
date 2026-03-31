import Link from 'next/link';
import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import {
  buildBreadcrumbStructuredData,
  buildPageMetadata,
  buildWebPageStructuredData,
} from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Documentation',
  description:
    'Relay44 documentation — guides, API reference, developer resources, and smart contract details for the prediction market platform on Base.',
  path: '/docs',
  keywords: ['documentation', 'guides', 'api reference', 'developer docs'],
});

const sections = [
  {
    title: 'User Guides',
    description: 'Learn how markets work, manage agents, build decision workflows, and connect external venues.',
    href: '/docs/guides/getting-started',
    items: ['Getting started', 'Markets', 'Agents', 'Decision cells', 'Credentials', 'Strategies'],
  },
  {
    title: 'Developers',
    description: 'Integrate with Relay44 — authentication flows, REST API quickstart, and WebSocket real-time feeds.',
    href: '/docs/developers',
    items: ['Quickstart', 'Authentication', 'WebSocket integration'],
  },
  {
    title: 'API Reference',
    description: '147 endpoints across 16 scopes — markets, orders, agents, decisions, EVM operations, and more.',
    href: '/docs/api',
    items: ['Auth', 'Markets', 'Orders', 'Positions', 'Agents', 'Decisions', 'EVM', 'WebSocket'],
  },
  {
    title: 'Smart Contracts',
    description: 'On-chain contracts deployed on Base — MarketCore, OrderBook, ERC-8004, and agent management.',
    href: '/docs/contracts',
    items: ['Contract addresses', 'ABI overview'],
  },
];

export default function DocsLandingPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/docs',
            name: 'Relay44 Documentation',
            description:
              'Guides, API reference, developer resources, and smart contract details for Relay44.',
          }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Documentation</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Relay44 is a prediction market platform on Base with live markets, automated agent execution,
        decision workflows, and multi-venue data aggregation. These docs cover everything you need
        to trade, build, and integrate.
      </p>

      <div className="mt-8 grid gap-4 sm:grid-cols-2">
        {sections.map((section) => (
          <Link key={section.href} href={section.href} className="block">
            <Card className="h-full p-6 transition-colors hover:border-border-hover">
              <h2 className="text-lg font-semibold text-text-primary">{section.title}</h2>
              <p className="mt-2 text-sm leading-6 text-text-secondary">{section.description}</p>
              <div className="mt-3 flex flex-wrap gap-2">
                {section.items.map((item) => (
                  <span
                    key={item}
                    className="inline-block border border-border px-2 py-0.5 text-[0.65rem] uppercase tracking-widest text-text-muted"
                  >
                    {item}
                  </span>
                ))}
              </div>
            </Card>
          </Link>
        ))}
      </div>

      <div className="mt-8">
        <Link
          href="/docs/guides/getting-started"
          className="inline-flex h-10 items-center border border-border px-6 text-sm font-medium text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
        >
          Get started &rarr;
        </Link>
      </div>
    </>
  );
}
