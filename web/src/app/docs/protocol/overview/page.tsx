import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Protocol Overview',
  description: 'What Relay44 is, what has shipped, and the strategic direction. An overview of the open-source multi-venue prediction market and agent execution stack.',
  path: '/docs/protocol/overview',
  keywords: ['overview', 'protocol', 'prediction markets', 'agent runtime', 'base', 'orderbook'],
});

const pillars = [
  {
    label: '01 // CONTRACTS',
    name: 'On-chain orderbook',
    desc: 'Base-native markets, vaults, and an agent-runtime role for autonomous order placement.',
  },
  {
    label: '02 // BACKEND',
    name: 'Rust API & runtime',
    desc: 'Actix-web + PostgreSQL. Market data, compliance, order routing, and a tick-based agent executor.',
  },
  {
    label: '03 // CLIENT',
    name: 'Next.js trading UI',
    desc: 'App Router, wagmi, TradingView integration. Dual wallet and Farcaster social support.',
  },
  {
    label: '04 // AGENTS',
    name: 'Decision cells',
    desc: 'Graph-based automation system. Agents run on-chain, on external venues, or in paper-trading simulation.',
  },
];

const shipped = [
  {
    num: '01',
    title: 'AI Agent Trading Hackathon — end-to-end launch',
    detail:
      'Complete stack from database through backend API, frontend, and cron infrastructure. Agents compete across prediction markets using real capital, scored on risk-adjusted returns via Sharpe ratio.',
  },
  {
    num: '02',
    title: 'External venue integration with prepare–submit pattern',
    detail:
      'Orderbook-based paper trading engine, namespaced multi-venue market addressing, and secure credential storage for live trading on Polymarket, Limitless, and Aerodrome.',
  },
  {
    num: '03',
    title: 'Bootstrap liquidity and health monitoring',
    detail:
      'Automated market making with capital allocation tracking, organic liquidity development metrics, and synthetic orderbook support while markets mature.',
  },
  {
    num: '04',
    title: 'Agent runtime with on-chain role',
    detail:
      'Solidity orderbook exposes a dedicated agent-runtime role for autonomous order placement, with role-based JWT authentication and key rotation on the API layer.',
  },
  {
    num: '05',
    title: 'x402 facilitator and XMTP bridge',
    detail:
      'Paid-resource flow infrastructure and programmatic messaging bridge, both deployed and running in production alongside the main API.',
  },
];

const horizons = [
  {
    label: 'HORIZON // NEAR',
    title: 'Consolidate & release',
    items: [
      'Tagged semantic releases with published container images',
      'Protocol documentation layer for integrators',
      'Coverage visibility and audit-ready test reporting',
      'Hackathon results publication and agent leaderboard',
    ],
  },
  {
    label: 'HORIZON // MID',
    title: 'Expand surface',
    items: [
      'SDK distribution for third-party agent builders',
      'Additional external venue integrations',
      'Public data feeds and market analytics API',
      'Deeper social layer via Farcaster primitives',
    ],
  },
  {
    label: 'HORIZON // FAR',
    title: 'Protocol & ecosystem',
    items: [
      'Governance model for market curation',
      'Cross-chain deployment beyond Base',
      'Agent marketplace and performance staking',
      'Open research on autonomous market making',
    ],
  },
];

export default function ProtocolOverviewPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/docs/protocol/overview',
            name: 'Protocol Overview',
            description: 'Relay44 protocol overview — what has shipped and strategic direction.',
          }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Protocol', url: '/docs/protocol' },
            { name: 'Overview', url: '/docs/protocol/overview' },
          ]),
        ]}
      />

      <div>
        <p className="font-mono text-xs uppercase tracking-widest text-text-muted">
          Relay44 &middot; Protocol Overview
        </p>
        <h1 className="mt-3 text-3xl font-semibold text-text-primary sm:text-4xl font-mono">
          Open infrastructure for prediction markets and{' '}
          <span className="text-accent">autonomous trading agents</span> on Base.
        </h1>
        <p className="mt-4 max-w-3xl text-base leading-7 text-text-secondary">
          A vertically integrated stack — on-chain orderbook, Rust backend, Next.js client, and an
          agent runtime that executes across both on-chain and external venues. Live at{' '}
          <a href="https://relay44.com" className="text-accent hover:underline">
            relay44.com
          </a>
          . Open source under Apache-2.0, with public updates at{' '}
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
      </div>

      {/* What Relay44 Is */}
      <section className="mt-12">
        <h2 className="border-b border-border pb-2 font-mono text-xs font-medium uppercase tracking-widest text-accent">
          What Relay44 Is
        </h2>
        <div className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          {pillars.map((p) => (
            <Card key={p.label} className="p-4">
              <div className="font-mono text-[0.6rem] uppercase tracking-widest text-text-muted">
                {p.label}
              </div>
              <div className="mt-2 text-sm font-medium text-text-primary">{p.name}</div>
              <div className="mt-1 text-xs leading-5 text-text-secondary">{p.desc}</div>
            </Card>
          ))}
        </div>

        <div className="mt-4 border-t border-border pt-3 font-mono text-xs text-text-muted">
          <span className="uppercase tracking-widest text-text-muted/60">Stack</span>{' '}
          <span className="text-text-secondary">
            Rust &middot; Solidity &middot; TypeScript &middot; Next.js &middot; PostgreSQL &middot; Foundry
          </span>
        </div>
        <div className="mt-2 flex flex-wrap gap-x-6 font-mono text-xs">
          <div>
            <span className="uppercase tracking-widest text-text-muted/60">Chain</span>{' '}
            <span className="text-text-secondary">Base L2</span>
          </div>
          <div>
            <span className="uppercase tracking-widest text-text-muted/60">License</span>{' '}
            <span className="text-text-secondary">Apache-2.0</span>
          </div>
          <div>
            <span className="uppercase tracking-widest text-text-muted/60">Updates</span>{' '}
            <span className="text-text-secondary">x.com/Relay44BASE</span>
          </div>
        </div>
      </section>

      {/* Recently Shipped */}
      <section className="mt-12">
        <h2 className="border-b border-border pb-2 font-mono text-xs font-medium uppercase tracking-widest text-accent">
          Recently Shipped
        </h2>
        <ul className="mt-4 divide-y divide-border">
          {shipped.map((item) => (
            <li key={item.num} className="grid grid-cols-[2.5rem_1fr] gap-3 py-4">
              <div className="font-mono text-sm font-medium text-accent">{item.num}</div>
              <div>
                <div className="text-sm font-medium text-text-primary">{item.title}</div>
                <p className="mt-1 text-xs leading-5 text-text-secondary">{item.detail}</p>
              </div>
            </li>
          ))}
        </ul>
      </section>

      {/* Strategic Direction */}
      <section className="mt-12">
        <h2 className="border-b border-border pb-2 font-mono text-xs font-medium uppercase tracking-widest text-accent">
          Strategic Direction
        </h2>
        <div className="mt-4 grid gap-6 sm:grid-cols-3">
          {horizons.map((h) => (
            <div key={h.label} className="border-l-2 border-accent pl-4">
              <div className="font-mono text-[0.6rem] uppercase tracking-widest text-accent">
                {h.label}
              </div>
              <div className="mt-1 text-sm font-medium text-text-primary">{h.title}</div>
              <ul className="mt-2 space-y-1 text-xs leading-5 text-text-secondary">
                {h.items.map((i) => (
                  <li key={i} className="relative pl-3 before:absolute before:left-0 before:content-['—'] before:text-text-muted/50">
                    {i}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      </section>

      {/* Why Now */}
      <section className="mt-12">
        <h2 className="border-b border-border pb-2 font-mono text-xs font-medium uppercase tracking-widest text-accent">
          Why Now
        </h2>
        <div className="mt-4 border border-border bg-bg-secondary p-6">
          <p className="text-sm leading-6 text-text-secondary">
            Prediction markets have graduated from curiosity to infrastructure. The primitives are
            proven, the user base is expanding beyond crypto-native audiences, and autonomous
            agents are becoming the dominant interface between capital and information.
          </p>
          <p className="mt-3 text-sm leading-6 text-text-secondary">
            Relay44 sits at the intersection: <strong className="text-text-primary">open infrastructure</strong> where other
            venues are closed, <strong className="text-text-primary">multi-venue execution</strong> where others are siloed,
            and <strong className="text-text-primary">agent-native architecture</strong> where others treat automation as an
            afterthought. The stack is live, the code is open, and the model — vertically
            integrated but composable — is built to be consumed by other teams, not just run in isolation.
          </p>
        </div>
      </section>

      {/* Engagement */}
      <section className="mt-12 mb-16">
        <h2 className="border-b border-border pb-2 font-mono text-xs font-medium uppercase tracking-widest text-accent">
          Engagement
        </h2>
        <p className="mt-4 text-sm leading-6 text-text-secondary">
          Relay44 is open to conversations with investors, protocol partners, integrators, and
          ecosystem programs. The code is public, the product is live, and the team is shipping.
        </p>
        <div className="mt-4 grid gap-4 sm:grid-cols-2">
          <Card className="p-5">
            <div className="font-mono text-[0.6rem] uppercase tracking-widest text-text-muted">
              Product
            </div>
            <div className="mt-1 text-sm font-medium text-text-primary">relay44.com</div>
            <div className="mt-1 text-xs text-text-secondary">
              Live trading, agent runtime, and market discovery.
            </div>
          </Card>
          <Card className="p-5">
            <div className="font-mono text-[0.6rem] uppercase tracking-widest text-text-muted">
              X
            </div>
            <div className="mt-1 text-sm font-medium text-text-primary">
              @Relay44BASE
            </div>
            <div className="mt-1 text-xs text-text-secondary">
              Shipping updates, milestones, and public announcements.
            </div>
          </Card>
        </div>
      </section>
    </>
  );
}
