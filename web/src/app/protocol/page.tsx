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

interface RelayUtilityResponse {
  chainId: number;
  token: { address: string; totalSupplyHex: string; decimals: number };
  staking: {
    address: string;
    totalStakedHex: string;
    tiers: Array<{
      tier: number;
      name: string;
      minRelayWei: string;
      feeDiscountBps: number;
      x402Bypass: boolean;
    }>;
    x402BypassTier: number;
  };
  rewardDistributor: { address: string };
  flags: {
    feeDiscount: boolean;
    x402Discount: boolean;
    stakingRewards: boolean;
    agentRewards: boolean;
    creatorRewards: boolean;
    governance: boolean;
  };
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

async function loadRelayUtility(): Promise<RelayUtilityResponse | null> {
  try {
    const response = await fetch(`${apiBase()}/protocol/relay-utility`, {
      next: { revalidate: 60 },
      headers: { accept: 'application/json' },
    });
    if (!response.ok) return null;
    return (await response.json()) as RelayUtilityResponse;
  } catch {
    return null;
  }
}

function formatNumber(value: number) {
  return new Intl.NumberFormat('en-US', {
    maximumFractionDigits: value >= 1000 ? 0 : 2,
  }).format(value);
}

function hexToTokenAmount(hex: string, decimals: number): number {
  if (!hex) return 0;
  try {
    const wei = BigInt(hex);
    const scale = BigInt(10) ** BigInt(decimals);
    const whole = wei / scale;
    const fraction = wei % scale;
    return Number(whole) + Number(fraction) / Number(scale);
  } catch {
    return 0;
  }
}

function shortAddress(address: string): string {
  if (!address || address.length < 10) return address;
  return `${address.slice(0, 6)}…${address.slice(-4)}`;
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
  const [metrics, relay] = await Promise.all([loadMetrics(), loadRelayUtility()]);
  const totalSupply = relay
    ? hexToTokenAmount(relay.token.totalSupplyHex, relay.token.decimals)
    : 0;
  const totalStaked = relay
    ? hexToTokenAmount(relay.staking.totalStakedHex, relay.token.decimals)
    : 0;
  const stakedShare = totalSupply > 0 ? (totalStaked / totalSupply) * 100 : 0;

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

        <div className="mt-12">
          <div className="flex flex-wrap items-end justify-between gap-3">
            <div>
              <p className="text-[0.7rem] uppercase tracking-[0.22em] text-accent">
                $RELAY
              </p>
              <h2 className="mt-1 text-2xl font-semibold text-text-primary sm:text-3xl">
                Token utility, on-chain.
              </h2>
            </div>
            <Link
              href="/docs/protocol/relay-utility"
              className="inline-flex h-9 items-center border border-border px-4 text-xs uppercase tracking-[0.16em] text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
            >
              Utility reference
            </Link>
          </div>

          <div className="mt-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            <MetricCard
              label="Total Supply"
              value={
                relay ? `${formatNumber(totalSupply)} RELAY` : 'Pending deploy'
              }
              note={
                relay
                  ? `Cap-enforced ERC20 at ${shortAddress(relay.token.address)}.`
                  : 'Reads RelayToken.totalSupply from Base mainnet.'
              }
            />
            <MetricCard
              label="Total Staked"
              value={
                relay
                  ? `${formatNumber(totalStaked)} RELAY`
                  : 'Pending deploy'
              }
              note={
                relay && stakedShare > 0
                  ? `~${stakedShare.toFixed(1)}% of supply at ${shortAddress(relay.staking.address)}.`
                  : 'Reads RelayStaking.totalStaked from Base mainnet.'
              }
            />
            <MetricCard
              label="Reward Distributor"
              value={relay ? shortAddress(relay.rewardDistributor.address) : 'Pending'}
              note="Per-epoch split between stakers, agents, creators, treasury."
            />
            <MetricCard
              label="x402 Bypass"
              value={
                relay
                  ? `Tier ${relay.staking.x402BypassTier}+`
                  : 'Pending deploy'
              }
              note="Free machine-facing API access at Gold and Diamond tiers."
            />
          </div>

          {relay ? (
            <div className="mt-6 overflow-hidden border border-border">
              <div className="grid grid-cols-12 gap-4 border-b border-border bg-bg-secondary px-4 py-3 text-[0.65rem] uppercase tracking-[0.2em] text-text-muted">
                <span className="col-span-3">Tier</span>
                <span className="col-span-3">Minimum RELAY</span>
                <span className="col-span-3">Fee discount</span>
                <span className="col-span-3">x402 access</span>
              </div>
              {relay.staking.tiers.map((tier) => {
                const min = hexToTokenAmount(
                  '0x' + BigInt(tier.minRelayWei).toString(16),
                  relay.token.decimals,
                );
                return (
                  <div
                    key={tier.tier}
                    className="grid grid-cols-12 gap-4 border-b border-border px-4 py-4 text-xs text-text-secondary last:border-b-0"
                  >
                    <span className="col-span-3 text-sm font-semibold text-text-primary">
                      {tier.name}
                    </span>
                    <code className="col-span-3 text-text-primary">
                      {min === 0 ? '0' : formatNumber(min)}
                    </code>
                    <code className="col-span-3 text-text-primary">
                      {(tier.feeDiscountBps / 100).toFixed(0)}%
                    </code>
                    <span className="col-span-3">
                      {tier.x402Bypass ? 'Free (bypass)' : tier.feeDiscountBps > 0 ? '25% discount' : 'Full price'}
                    </span>
                  </div>
                );
              })}
            </div>
          ) : null}

          <div className="mt-4 flex flex-wrap gap-2 text-[0.65rem] uppercase tracking-[0.18em] text-text-muted">
            {relay
              ? Object.entries(relay.flags).map(([key, on]) => (
                  <span
                    key={key}
                    className={`border px-2 py-1 ${
                      on
                        ? 'border-accent text-accent'
                        : 'border-border text-text-muted'
                    }`}
                  >
                    {key.replace(/([A-Z])/g, ' $1').trim()} · {on ? 'live' : 'roadmap'}
                  </span>
                ))
              : null}
          </div>
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
