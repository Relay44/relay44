import Link from 'next/link';
import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import {
  buildBreadcrumbStructuredData,
  buildPageMetadata,
  buildWebPageStructuredData,
} from '@/lib/seo';

export const dynamic = 'force-dynamic';

export const metadata = buildPageMetadata({
  title: 'Protocol Dashboard',
  description:
    'Public Relay44 Protocol metrics: markets, agents, settlement volume, and USDC collateral on Base.',
  path: '/protocol',
  keywords: ['relay44 protocol', 'protocol metrics', 'base prediction markets', 'agents'],
});

interface ProtocolMetricsResponse {
  markets: { total: number; active: number };
  volume: { settlementUsdc: number; tableReportedUsdc: number };
  agents: { connected: number; active: number };
  collateral: { usdc: number };
  source: string;
  updatedAt: string;
}

function apiBase() {
  return (
    process.env.NEXT_PUBLIC_API_URL?.trim() ||
    'https://relay44-api.onrender.com/v1'
  ).replace(/\/+$/, '');
}

async function loadMetrics(): Promise<ProtocolMetricsResponse | null> {
  try {
    const response = await fetch(`${apiBase()}/protocol/metrics`, {
      next: { revalidate: 60 },
      headers: { accept: 'application/json' },
    });
    if (!response.ok) return null;
    return (await response.json()) as ProtocolMetricsResponse;
  } catch {
    return null;
  }
}

function formatNumber(value: number) {
  return new Intl.NumberFormat('en-US', {
    maximumFractionDigits: value >= 1000 ? 0 : 2,
  }).format(value);
}

function MetricCard({
  label,
  value,
  note,
}: {
  label: string;
  value: string;
  note: string;
}) {
  return (
    <Card className="p-6">
      <p className="text-[0.7rem] uppercase tracking-[0.2em] text-text-muted">
        {label}
      </p>
      <p className="mt-3 text-3xl font-semibold text-text-primary">{value}</p>
      <p className="mt-2 text-sm leading-6 text-text-secondary">{note}</p>
    </Card>
  );
}

export default async function ProtocolDashboardPage() {
  const metrics = await loadMetrics();

  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/protocol',
            name: 'Protocol Dashboard',
            description:
              'Public Relay44 Protocol metrics for markets, agents, settlement volume, and collateral.',
          }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Protocol Dashboard', url: '/protocol' },
          ]),
        ]}
      />

      <div className="max-w-5xl">
        <p className="text-[0.7rem] uppercase tracking-[0.22em] text-accent">
          Relay44 Protocol
        </p>
        <h1 className="mt-3 text-3xl font-semibold text-text-primary sm:text-5xl">
          Base prediction-market infrastructure, measured publicly.
        </h1>
        <p className="mt-4 max-w-3xl text-base leading-7 text-text-secondary">
          This dashboard reports protocol-level usage from the production API:
          markets, autonomous agents, settlement volume, and USDC collateral.
          It is intentionally separate from consumer trading UI analytics.
        </p>

        <div className="mt-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          <MetricCard
            label="Markets"
            value={
              metrics
                ? `${formatNumber(metrics.markets.active)} active`
                : 'Pending deploy'
            }
            note={
              metrics
                ? `${formatNumber(metrics.markets.total)} total markets tracked.`
                : 'The /v1/protocol/metrics endpoint will populate this after API deploy.'
            }
          />
          <MetricCard
            label="Connected Agents"
            value={
              metrics
                ? formatNumber(metrics.agents.connected)
                : 'Pending deploy'
            }
            note={
              metrics
                ? `${formatNumber(metrics.agents.active)} active agent records.`
                : 'Counts external, managed, and bootstrap protocol agents.'
            }
          />
          <MetricCard
            label="Settlement Volume"
            value={
              metrics
                ? `$${formatNumber(metrics.volume.settlementUsdc)}`
                : 'Pending deploy'
            }
            note="USDC-equivalent settlement flow from recorded trades."
          />
          <MetricCard
            label="USDC Collateral"
            value={
              metrics
                ? `$${formatNumber(metrics.collateral.usdc)}`
                : 'Pending deploy'
            }
            note="Collateral tracked by market tables where available."
          />
        </div>

        <div className="mt-8 grid gap-6 lg:grid-cols-[1.2fr_0.8fr]">
          <Card className="p-6">
            <h2 className="text-lg font-semibold text-text-primary">
              Builder surface
            </h2>
            <p className="mt-2 text-sm leading-6 text-text-secondary">
              Builders should start with the protocol package, deployed contract
              manifest, and quickstart example rather than copying addresses by hand.
            </p>
            <div className="mt-5 flex flex-wrap gap-3">
              <Link
                href="/docs/contracts"
                className="inline-flex h-9 items-center border border-border px-4 text-xs uppercase tracking-[0.16em] text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
              >
                Contract reference
              </Link>
              <Link
                href="/docs/developers/quickstart"
                className="inline-flex h-9 items-center border border-border px-4 text-xs uppercase tracking-[0.16em] text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
              >
                Quickstart
              </Link>
              <a
                href="https://x.com/Relay44OnBase"
                target="_blank"
                rel="noreferrer"
                className="inline-flex h-9 items-center border border-border px-4 text-xs uppercase tracking-[0.16em] text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
              >
                X updates
              </a>
            </div>
          </Card>

          <Card className="p-6">
            <h2 className="text-lg font-semibold text-text-primary">
              Metrics endpoint
            </h2>
            <p className="mt-2 text-sm leading-6 text-text-secondary">
              Public JSON endpoint for protocol dashboards and external monitors.
            </p>
            <code className="mt-4 block overflow-x-auto border border-border bg-bg-secondary p-3 font-mono text-xs text-text-primary">
              GET https://relay44-api.onrender.com/v1/protocol/metrics
            </code>
            {metrics ? (
              <p className="mt-3 text-xs uppercase tracking-[0.16em] text-text-muted">
                Updated {new Date(metrics.updatedAt).toLocaleString('en-US')} from{' '}
                {metrics.source}
              </p>
            ) : null}
          </Card>
        </div>
      </div>
    </>
  );
}
