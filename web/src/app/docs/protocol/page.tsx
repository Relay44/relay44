import Link from 'next/link';
import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Protocol',
  description: 'Relay44 protocol overview and technical roadmap — architecture, subsystems, design principles, and forward direction of the open-source multi-venue prediction market and agent execution stack.',
  path: '/docs/protocol',
  keywords: ['protocol', 'architecture', 'roadmap', 'overview', 'agent runtime', 'orderbook', 'erc-8004'],
});

const pages = [
  {
    title: 'Overview',
    description:
      'What Relay44 is, what has shipped, and the strategic direction. Written for partners, integrators, and ecosystem reviewers who want to understand the project at a glance.',
    href: '/docs/protocol/overview',
  },
  {
    title: 'Protocol Dashboard',
    description:
      'Public protocol metrics for markets, agents, settlement volume, and USDC collateral.',
    href: 'https://relay44.com/protocol',
  },
  {
    title: 'Contracts and Package',
    description:
      'Production/staging addresses, generated ABIs, @relay44/protocol install path, and viem examples.',
    href: '/docs/contracts',
  },
  {
    title: 'Builder Quickstart',
    description:
      'Install @relay44/protocol, read MarketCore on Base, fetch markets, authenticate, and place an order.',
    href: '/docs/developers/quickstart',
  },
  {
    title: '$RELAY Utility',
    description:
      'Canonical reference for the RELAY token surface — staking tiers, fee discounts, free x402 access at Gold and above, the public utility endpoint, and importable SDK constants.',
    href: '/docs/protocol/relay-utility',
  },
  {
    title: 'Technical Roadmap',
    description:
      'Architect-level deep dive — system layers, core subsystems, design principles, eight concrete technical workstreams, and the invariants that shape every decision.',
    href: '/docs/protocol/roadmap',
  },
];

export default function ProtocolIndexPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/docs/protocol',
            name: 'Protocol',
            description: 'Relay44 protocol overview and technical roadmap.',
          }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Protocol', url: '/docs/protocol' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Protocol</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Relay44 is open infrastructure for prediction markets and autonomous trading agents on
        Base. This section describes what the protocol is today and where it is headed. Everything
        here is grounded in the Apache-2.0 monorepo. Public updates land at{' '}
        <a
          href="https://x.com/Relay44BASE"
          className="text-accent hover:underline"
          target="_blank"
          rel="noopener noreferrer"
        >
          x.com/Relay44BASE
        </a>
        .
      </p>

      <div className="mt-8 grid gap-4">
        {pages.map((page) => (
          <Link
            key={page.href}
            href={page.href}
            className="block"
            target={page.href.startsWith('http') ? '_blank' : undefined}
            rel={page.href.startsWith('http') ? 'noreferrer' : undefined}
          >
            <Card className="p-6 transition-colors hover:border-border-hover">
              <h2 className="text-lg font-semibold text-text-primary">{page.title}</h2>
              <p className="mt-2 text-sm leading-6 text-text-secondary">{page.description}</p>
            </Card>
          </Link>
        ))}
      </div>
    </>
  );
}
