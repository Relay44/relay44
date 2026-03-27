import Link from 'next/link';
import { PageShell } from '@/components/layout';
import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import {
  buildBreadcrumbStructuredData,
  buildPageMetadata,
  buildWebPageStructuredData,
} from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'How It Works',
  description: 'How Relay44 organizes market creation, trading, resolution, agent execution, and decision workflows.',
  path: '/how-it-works',
  keywords: ['how it works', 'market resolution', 'agent execution', 'prediction market platform'],
});

const sections = [
  {
    title: 'Market lifecycle',
    body:
      'Each market starts with a clear question, a close time, and a published resolution source. Trading stays tied to those terms so settlement and final outcomes remain auditable.',
  },
  {
    title: 'Trading and pricing',
    body:
      'Users can browse live markets, pricing, order books, and recent trades from Relay44 and connected venues. Market pages keep the question, close time, and activity in one place.',
  },
  {
    title: 'Agents',
    body:
      'Agents let users define execution instructions, attach venue credentials where needed, and monitor cadence, readiness, and run history from one directory.',
  },
  {
    title: 'Decision cells',
    body:
      'Decision cells are private workspaces for linking markets, alerts, and external agent actions around one decision. They help organize signals and track what should happen when conditions change.',
  },
  {
    title: 'Wallet and portfolio',
    body:
      'Wallet and portfolio views keep balances, positions, orders, and transfer history attached to one account surface.',
  },
  {
    title: 'API and integrations',
    body:
      'The API exposes market data, platform capabilities, and integration endpoints for developer tools, internal systems, and machine clients.',
  },
];

export default function HowItWorksPage() {
  return (
    <PageShell>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/how-it-works',
            name: 'How Relay44 works',
            description:
              'How Relay44 organizes market creation, trading, resolution, agent execution, and decision workflows.',
          }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'How It Works', url: '/how-it-works' },
          ]),
        ]}
      />
      <div className="mx-auto max-w-5xl py-2 sm:py-4">
        <div className="mb-8 max-w-3xl">
          <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">How It Works</h1>
          <p className="mt-4 text-base leading-7 text-text-secondary">
            Relay44 combines market creation, trading, wallet activity, decision workflows, and
            agent execution in one product. The platform is organized so market rules, execution
            state, and account activity stay legible from the same system.
          </p>
        </div>

        <div className="grid gap-4 md:grid-cols-2">
          {sections.map((section) => (
            <Card key={section.title} className="space-y-3 p-6">
              <h2 className="text-lg font-semibold text-text-primary">{section.title}</h2>
              <p className="text-sm leading-6 text-text-secondary">{section.body}</p>
            </Card>
          ))}
        </div>

        <div className="mt-8 flex flex-wrap gap-3">
          <Link
            href="/markets"
            className="inline-flex h-10 items-center border border-border px-4 text-sm text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
          >
            Browse markets
          </Link>
          <Link
            href="/agents"
            className="inline-flex h-10 items-center border border-border px-4 text-sm text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
          >
            View agents
          </Link>
          <Link
            href="/docs/api"
            className="inline-flex h-10 items-center border border-border px-4 text-sm text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
          >
            API documentation
          </Link>
          <Link
            href="/legal/disclaimer"
            className="inline-flex h-10 items-center border border-border px-4 text-sm text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
          >
            Risk disclaimer
          </Link>
        </div>
      </div>
    </PageShell>
  );
}
