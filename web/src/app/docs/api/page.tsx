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
  title: 'API',
  description:
    'Reference for Relay44 capability, market, health, and integration endpoints exposed by the public web stack.',
  path: '/docs/api',
  keywords: ['api', 'developer docs', 'market endpoints', 'web4 capabilities'],
});

const endpointGroups = [
  {
    title: 'Health and runtime',
    description: 'Use these routes to confirm the deployment is live and inspect the current runtime mode.',
    endpoints: [
      { method: 'GET', path: '/health', note: 'basic web-service health check' },
      { method: 'GET', path: '/health/detailed', note: 'runtime checks and provider status' },
      { method: 'GET', path: '/v1/web4/capabilities', note: 'wallet, read/write, and launch flags' },
    ],
  },
  {
    title: 'Markets',
    description: 'Read market listings, market detail, order books, and recent trades from the unified market feed.',
    endpoints: [
      { method: 'GET', path: '/v1/evm/markets?limit=50&offset=0&source=all', note: 'market directory with internal and external sources' },
      { method: 'GET', path: '/v1/evm/markets/{marketId}', note: 'single market snapshot' },
      { method: 'GET', path: '/v1/evm/markets/{marketId}/orderbook?outcome=yes&depth=20', note: 'best bid and ask levels for one outcome' },
      { method: 'GET', path: '/v1/evm/markets/{marketId}/trades?limit=50', note: 'recent fills and trades' },
    ],
  },
  {
    title: 'Agents',
    description: 'Agent endpoints expose the current runtime set when a backend is present and otherwise return a factual empty state.',
    endpoints: [
      { method: 'GET', path: '/v1/evm/agents?limit=20&offset=0', note: 'agent directory and activity summary' },
      { method: 'GET', path: '/v1/evm/agents/{agentId}', note: 'single agent detail when available' },
    ],
  },
  {
    title: 'Auth and integration',
    description: 'These routes support wallet sign-in, Farcaster actions, and machine-facing integrations.',
    endpoints: [
      { method: 'GET', path: '/v1/auth/siwe/nonce', note: 'nonce for SIWE login' },
      { method: 'GET', path: '/v1/web4/mcp', note: 'machine-facing capability entrypoint' },
      { method: 'GET', path: '/v1/web4/agent-card', note: 'agent card payload for embeds' },
    ],
  },
];

export default function ApiDocsPage() {
  return (
    <PageShell>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/docs/api',
            name: 'Relay44 API',
            description:
              'Reference for Relay44 capability, market, health, and integration endpoints exposed by the public web stack.',
          }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'API', url: '/docs/api' },
          ]),
        ]}
      />

      <div className="mx-auto max-w-5xl py-2 sm:py-4">
        <div className="max-w-3xl">
          <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">API</h1>
          <p className="mt-4 text-base leading-7 text-text-secondary">
            The public web deployment exposes a small set of read routes directly. When a full
            backend is configured, the same pages can also use the upstream API through the proxy.
            In web-only mode, unsupported write routes stay disabled instead of returning fake data.
          </p>
        </div>

        <div className="mt-8 grid gap-4">
          {endpointGroups.map((group) => (
            <Card key={group.title} className="p-6">
              <h2 className="text-lg font-semibold text-text-primary">{group.title}</h2>
              <p className="mt-2 text-sm leading-6 text-text-secondary">{group.description}</p>
              <div className="mt-4 overflow-hidden border border-border">
                {group.endpoints.map((endpoint) => (
                  <div
                    key={endpoint.path}
                    className="grid gap-2 border-b border-border px-4 py-3 last:border-b-0 md:grid-cols-[5.5rem_minmax(0,1fr)]"
                  >
                    <span className="inline-flex h-8 w-fit items-center border border-border px-3 text-[0.75rem] font-medium uppercase tracking-[0.14em] text-text-primary">
                      {endpoint.method}
                    </span>
                    <div className="min-w-0">
                      <code className="block overflow-x-auto text-sm text-text-primary">
                        {endpoint.path}
                      </code>
                      <p className="mt-1 text-xs uppercase tracking-[0.12em] text-text-muted">
                        {endpoint.note}
                      </p>
                    </div>
                  </div>
                ))}
              </div>
            </Card>
          ))}
        </div>

        <div className="mt-8 flex flex-wrap gap-3">
          <Link
            href="/v1/web4/capabilities"
            className="inline-flex h-10 items-center border border-border px-4 text-sm text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
          >
            Open capabilities
          </Link>
          <Link
            href="/v1/evm/markets?limit=5&offset=0&source=all"
            className="inline-flex h-10 items-center border border-border px-4 text-sm text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
          >
            Open market feed
          </Link>
          <Link
            href="/health/detailed"
            className="inline-flex h-10 items-center border border-border px-4 text-sm text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
          >
            Open health report
          </Link>
        </div>
      </div>
    </PageShell>
  );
}
