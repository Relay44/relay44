import Link from 'next/link';
import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Developers',
  description: 'Developer overview for Relay44 — architecture, API base URL, rate limits, and integration patterns.',
  path: '/docs/developers',
  keywords: ['developers', 'api', 'integration', 'architecture'],
});

export default function DevelopersPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({ path: '/docs/developers', name: 'Developers', description: 'Developer overview for Relay44.' }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Developers', url: '/docs/developers' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">Developers</h1>
      <p className="mt-4 text-base leading-7 text-text-secondary">
        Build on Relay44 using the REST API, WebSocket feeds, and smart contracts.
      </p>

      <div className="mt-8 grid gap-6">
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Architecture</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Relay44 is a hybrid on-chain/off-chain prediction market platform deployed on Base.
            The backend (Rust/Actix-web) manages the order book, agent execution, and venue
            integrations. The frontend (Next.js) provides the trading interface. Smart contracts
            handle settlement, identity, and reputation on-chain.
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Base URL</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            All API endpoints are served from <code className="text-text-primary">https://relay44.com</code>.
            The API is versioned under <code className="text-text-primary">/v1/</code>.
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Authentication</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Most read endpoints are public. Write endpoints require a JWT obtained through one
            of the supported auth flows (SIWE, Solana, Farcaster). Pass the token as
            a <code className="text-text-primary">Bearer</code> token in the
            <code className="text-text-primary"> Authorization</code> header.
          </p>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Response format</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            All responses are JSON. Successful responses return the data directly. Errors return
            an object with <code className="text-text-primary">error</code> and
            <code className="text-text-primary"> message</code> fields with an appropriate HTTP
            status code.
          </p>
        </Card>

        <div className="grid gap-3 sm:grid-cols-3">
          {[
            { label: 'Quickstart', href: '/docs/developers/quickstart' },
            { label: 'Authentication', href: '/docs/developers/authentication' },
            { label: 'WebSocket', href: '/docs/developers/websocket' },
          ].map((link) => (
            <Link
              key={link.href}
              href={link.href}
              className="inline-flex h-10 items-center justify-center border border-border px-4 text-sm text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
            >
              {link.label} &rarr;
            </Link>
          ))}
        </div>
      </div>
    </>
  );
}
