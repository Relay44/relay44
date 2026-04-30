import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import { buildBreadcrumbStructuredData, buildPageMetadata, buildWebPageStructuredData } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Technical Roadmap',
  description: 'Architect-level deep dive into the Relay44 stack — system layers, core subsystems, design principles, and the technical workstreams shaping the open-source prediction market and agent execution protocol.',
  path: '/docs/protocol/roadmap',
  keywords: [
    'technical roadmap',
    'architecture',
    'orderbook',
    'agent runtime',
    'erc-8004',
    'multi-venue',
    'rust',
    'solidity',
    'base',
  ],
});

const layers = [
  {
    id: 'L0',
    name: 'Chain layer',
    desc: 'Base L2. Solidity 0.8.24 with OpenZeppelin AccessControl, Pausable, ReentrancyGuard. Markets, vaults, staking, reward distribution, and ERC-8004 identity/reputation/validation registries.',
    items: [
      'MarketCore — market registry and settlement coordinator',
      'OrderBook — onchain limit order matching with AGENT_RUNTIME_ROLE',
      'CollateralVault — USDC collateral and position accounting',
      'AgentRuntime — non-custodial order placement on behalf of agents',
      'DistributionMarket, ParlayEscrow — multi-outcome and combination products',
      'RelayToken, RelayStaking, RewardDistributor — protocol economics',
      'ERC-8004 registries — identity, reputation, and validation for agents',
    ],
  },
  {
    id: 'L1',
    name: 'Data layer',
    desc: 'PostgreSQL with 54+ migrations. Normalized market, position, order, decision, and agent run history. Encrypted external credential storage with per-user key derivation.',
    items: [
      'Market snapshots and orderbook state',
      'Agent runs, decisions, and cell graph history',
      'Hackathon leaderboards and Sharpe-based scoring',
      'Encrypted credential vault for external venue keys',
      'Bootstrap liquidity tracking and health metrics',
    ],
  },
  {
    id: 'L2',
    name: 'Service layer',
    desc: 'Rust (Actix-web). Domain services for risk, hedging, liquidity, market data, scanning, and multi-venue execution. Tick-based agent executor runs decision cells on schedule.',
    items: [
      'risk_governor, kelly, hedge_engine — position sizing and risk limits',
      'liquidity_mirror — synthetic orderbook for bootstrap liquidity',
      'pyth — oracle integration for price feeds',
      'polymarket_scanner, limitless_scanner, aerodrome_scanner — external venue discovery',
      'managed_agent_runner — tick executor for decision cell graphs',
      'external/credentials, ledger, paper, strategy — non-custodial venue bridge',
    ],
  },
  {
    id: 'L3',
    name: 'API layer',
    desc: 'Actix-web HTTP + WebSocket. 40+ modules covering markets, orders, agents, decisions, hackathon, signals, parlays, x402 paid resources, and compliance surfaces.',
    items: [
      'REST API with JWT and role-based auth (user, agent runtime, internal service)',
      'WebSocket for realtime orderbook and market events',
      'x402 facilitator for paid-resource flows',
      'XMTP bridge for programmatic agent messaging',
      'MCP server for agent authoring and tool use',
    ],
  },
  {
    id: 'L4',
    name: 'Client layer',
    desc: 'Next.js App Router. Trading UI with TradingView, wagmi wallet, Farcaster social, agent dashboards, hackathon leaderboard, and full docs site.',
    items: [
      'Trading UI with TradingView integration',
      'Dual wallet — EOA and Farcaster-native flows',
      'Agent dashboards and decision cell authoring',
      'Hackathon leaderboard and live agent monitoring',
      'Docs site — guides, API reference, protocol overview',
    ],
  },
];

const principles = [
  {
    title: 'Non-custodial by default',
    desc: 'Prepare–submit pattern for all external venues. User keys sign. API never holds private key material for external venues.',
  },
  {
    title: 'Namespaced market identity',
    desc: 'Every market is addressed as venue:market_id. Relay44-native, Polymarket, Limitless, and Aerodrome all share the same identifier space.',
  },
  {
    title: 'Role-separated onchain trust',
    desc: 'OrderBook distinguishes user orders from AGENT_RUNTIME_ROLE orders. Agents never gain user permissions — they place orders only via an explicit runtime role.',
  },
  {
    title: 'Deterministic agent execution',
    desc: 'Decision cells run on a tick. Same inputs produce the same outputs. All decisions logged for replay and audit.',
  },
  {
    title: 'Risk-first position sizing',
    desc: 'Kelly fraction with governor overrides. Hedge engine tracks net exposure. No agent can bypass risk_governor limits.',
  },
  {
    title: 'Open by construction',
    desc: 'Apache-2.0. Monorepo. Every service, contract, and client is in one public tree. No private forks or closed components.',
  },
];

const subsystems = [
  {
    title: 'Onchain orderbook',
    code: 'evm/src/OrderBook.sol',
    body: 'Base-native limit orderbook. AccessControl separates user orders from AGENT_RUNTIME_ROLE orders. Pausable for emergency response. ReentrancyGuard on all state-mutating entry points. Settlement flows through CollateralVault with explicit position accounting.',
    points: [
      'User-signed order placement via standard ERC-2612 permit flow',
      'Agent runtime order placement via placeOrderFor, authenticated by role',
      'MarketCore registers markets and coordinates settlement',
      'Non-custodial — collateral held in CollateralVault, never in OrderBook itself',
    ],
  },
  {
    title: 'Agent runtime',
    code: 'app/src/services/managed_agent_runner.rs + evm/src/AgentRuntime.sol',
    body: 'Tick-based executor that runs decision cell graphs on schedule. Each agent has an ERC-8004 identity. Onchain AgentRuntime contract enforces that only authenticated runners can place orders on behalf of a registered agent identity.',
    points: [
      'Decision cells compose into directed graphs — signal → decision → execution',
      'Paper, onchain, and external venue modes run the same cell graph',
      'Agent identity registered via ERC-8004 IdentityRegistry',
      'RELAY token burn on agent registration ties economic stake to identity',
    ],
  },
  {
    title: 'External venue bridge',
    code: 'app/src/services/external/',
    body: 'Prepare–submit pattern for Polymarket, Limitless, and Aerodrome. API prepares unsigned transactions and orders; user or agent signs; API submits on confirmation. Encrypted credential vault stores per-user external venue keys with master-key-derived encryption.',
    points: [
      'Namespaced market identifiers — polymarket:0x..., limitless:0x..., aerodrome:0x...',
      'Orderbook-based paper trading engine mirrors live venue state',
      'Credential vault with rotation and per-user derivation',
      'Strategy layer abstracts venue-specific quirks behind a common interface',
    ],
  },
  {
    title: 'Risk and sizing',
    code: 'app/src/services/{risk_governor,kelly,hedge_engine}.rs',
    body: 'Three-layer risk stack. Kelly computes base position size from edge and odds. risk_governor applies per-agent, per-market, and global limits. hedge_engine tracks net exposure across correlated markets and opens offsetting positions when thresholds are breached.',
    points: [
      'Kelly fraction with configurable safety multiplier',
      'Per-agent capital allocation and daily drawdown limits',
      'Cross-market hedging for correlated prediction contracts',
      'All sizing decisions logged to the decision history for audit',
    ],
  },
  {
    title: 'Liquidity bootstrap',
    code: 'app/src/services/liquidity_mirror.rs',
    body: 'Synthetic orderbook that mirrors oracle and external venue pricing while Relay44-native markets mature. Automated market making against tracked capital allocation. Health metrics distinguish bootstrap liquidity from organic flow so the market owner can measure genuine adoption.',
    points: [
      'Synthetic quotes derived from Pyth and external venue mid-prices',
      'Capital allocation tracking per market and per maker',
      'Organic liquidity percentage as a market health metric',
      'Graceful handoff from synthetic to organic as real flow appears',
    ],
  },
  {
    title: 'Agent identity and reputation',
    code: 'evm/src/ERC8004*.sol',
    body: 'Full ERC-8004 implementation. IdentityRegistry mints a non-transferable identity per agent. ReputationRegistry accumulates signed attestations from validators. ValidationRegistry coordinates third-party validation of agent claims. Together they provide an onchain trust surface for autonomous participants.',
    points: [
      'Non-transferable agent identity tokens',
      'Signed reputation attestations from whitelisted validators',
      'Validator set governed by the protocol role structure',
      'Hackathon scoring feeds into reputation updates automatically',
    ],
  },
];

const workstreams = [
  {
    num: '01',
    title: 'SDK distribution',
    body: 'TypeScript and Rust SDKs for third-party agent builders. Wraps REST, WebSocket, and MCP surfaces. Typed clients, signed request helpers, and paper-trading harness for local iteration.',
  },
  {
    num: '02',
    title: 'Multi-venue expansion',
    body: 'Additional external venues via the existing external/ bridge pattern. Each new venue adds a provider module and a namespace prefix — no changes to the core agent runtime or risk stack.',
  },
  {
    num: '03',
    title: 'Data and analytics API',
    body: 'Public data feeds for market snapshots, historical orderbook state, and agent performance timeseries. Foundation for third-party dashboards and research.',
  },
  {
    num: '04',
    title: 'Advanced market types',
    body: 'DistributionMarket and ParlayEscrow in production use. Scalar markets, combinatorial parlays, and conditional markets on top of the existing MarketCore primitive.',
  },
  {
    num: '05',
    title: 'Cross-chain deployment',
    body: 'Contracts are written against standard EVM primitives. Deployment beyond Base requires bridge design for RELAY token and decision on per-chain vs. unified orderbook state.',
  },
  {
    num: '06',
    title: 'Agent marketplace',
    body: 'Discovery and subscription layer on top of ERC-8004 reputation. Performance staking so capital can follow proven agents. Revenue share via RewardDistributor.',
  },
  {
    num: '07',
    title: 'Governance and curation',
    body: 'Market curation authority via the RELAY token. Proposal and vote surface for new market approvals, oracle selections, and risk parameter updates.',
  },
  {
    num: '08',
    title: 'Research and publication',
    body: 'Open research on autonomous market making, agent risk budgeting, and the economics of multi-venue execution. Hackathon data is the first public dataset.',
  },
];

const invariants = [
  'Agents never hold user keys. All external venue orders are prepared by the API and signed by the user or the agent runtime role.',
  'Every order placed onchain is attributable to either a user signature or the AGENT_RUNTIME_ROLE — no ambiguous authority paths.',
  'Decision cells are deterministic. Same inputs, same outputs. All runs are logged for replay and audit.',
  'Risk limits are enforced before execution, not after. risk_governor can reject orders that kelly would otherwise size.',
  'The monorepo is the source of truth. No private forks, no closed components, no out-of-band deployments.',
];

const techRef = [
  { k: 'Language', v: 'Rust (backend) · Solidity 0.8.24 · TypeScript · Next.js' },
  { k: 'Chain', v: 'Base L2' },
  { k: 'Database', v: 'PostgreSQL (54+ migrations)' },
  { k: 'Web framework', v: 'Actix-web · Next.js App Router' },
  { k: 'Contracts', v: 'OpenZeppelin AccessControl · Pausable · ReentrancyGuard · Foundry' },
  { k: 'Oracles', v: 'Pyth Network' },
  { k: 'External venues', v: 'Polymarket · Limitless · Aerodrome' },
  { k: 'Agent standards', v: 'ERC-8004 · MCP · A2A · XMTP · x402' },
  { k: 'License', v: 'Apache-2.0' },
];

export default function ProtocolRoadmapPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/docs/protocol/roadmap',
            name: 'Technical Roadmap',
            description: 'Architect-level deep dive into the Relay44 stack.',
          }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Protocol', url: '/docs/protocol' },
            { name: 'Technical Roadmap', url: '/docs/protocol/roadmap' },
          ]),
        ]}
      />

      <div>
        <p className="font-mono text-xs uppercase tracking-widest text-text-muted">
          Relay44 &middot; Technical Roadmap
        </p>
        <h1 className="mt-3 text-3xl font-semibold text-text-primary sm:text-4xl font-mono">
          Architect-level view of the <span className="text-accent">Relay44 stack</span>.
        </h1>
        <p className="mt-4 max-w-3xl text-base leading-7 text-text-secondary">
          A technical reference for engineers, integrators, and researchers. Every component named
          here lives in the Apache-2.0 monorepo. Public updates land at{' '}
          <a
            href="https://x.com/Relay44BASE"
            className="text-accent hover:underline"
            target="_blank"
            rel="noopener noreferrer"
          >
            x.com/Relay44BASE
          </a>
          . Paths and module names are real — use them as entry points into the code.
        </p>
      </div>

      {/* System at a glance */}
      <section className="mt-12">
        <h2 className="border-b border-border pb-2 font-mono text-xs font-medium uppercase tracking-widest text-accent">
          System at a glance
        </h2>
        <div className="mt-4 space-y-3">
          {layers.map((layer) => (
            <Card key={layer.id} className="p-5">
              <div className="flex items-baseline gap-3">
                <div className="font-mono text-sm font-medium text-accent">{layer.id}</div>
                <div className="text-sm font-medium text-text-primary">{layer.name}</div>
              </div>
              <p className="mt-2 text-xs leading-5 text-text-secondary">{layer.desc}</p>
              <ul className="mt-3 space-y-1 font-mono text-xs text-text-secondary">
                {layer.items.map((i) => (
                  <li key={i} className="relative pl-4 before:absolute before:left-0 before:content-['›'] before:text-text-muted/60">
                    {i}
                  </li>
                ))}
              </ul>
            </Card>
          ))}
        </div>
      </section>

      {/* Design principles */}
      <section className="mt-12">
        <h2 className="border-b border-border pb-2 font-mono text-xs font-medium uppercase tracking-widest text-accent">
          Design principles
        </h2>
        <div className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {principles.map((p) => (
            <Card key={p.title} className="p-4">
              <div className="text-sm font-medium text-text-primary">{p.title}</div>
              <p className="mt-1 text-xs leading-5 text-text-secondary">{p.desc}</p>
            </Card>
          ))}
        </div>
      </section>

      {/* Core subsystems */}
      <section className="mt-12">
        <h2 className="border-b border-border pb-2 font-mono text-xs font-medium uppercase tracking-widest text-accent">
          Core subsystems
        </h2>
        <div className="mt-4 space-y-6">
          {subsystems.map((s) => (
            <div key={s.title} className="border-l-2 border-accent pl-4">
              <div className="text-sm font-medium text-text-primary">{s.title}</div>
              <div className="mt-1 font-mono text-[0.65rem] uppercase tracking-widest text-text-muted">
                {s.code}
              </div>
              <p className="mt-2 text-xs leading-5 text-text-secondary">{s.body}</p>
              <ul className="mt-2 space-y-1 text-xs leading-5 text-text-secondary">
                {s.points.map((pt) => (
                  <li key={pt} className="relative pl-3 before:absolute before:left-0 before:content-['—'] before:text-text-muted/50">
                    {pt}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      </section>

      {/* Technical directions */}
      <section className="mt-12">
        <h2 className="border-b border-border pb-2 font-mono text-xs font-medium uppercase tracking-widest text-accent">
          Technical directions
        </h2>
        <ul className="mt-4 divide-y divide-border">
          {workstreams.map((w) => (
            <li key={w.num} className="grid grid-cols-[2.5rem_1fr] gap-3 py-4">
              <div className="font-mono text-sm font-medium text-accent">{w.num}</div>
              <div>
                <div className="text-sm font-medium text-text-primary">{w.title}</div>
                <p className="mt-1 text-xs leading-5 text-text-secondary">{w.body}</p>
              </div>
            </li>
          ))}
        </ul>
      </section>

      {/* Invariants */}
      <section className="mt-12">
        <h2 className="border-b border-border pb-2 font-mono text-xs font-medium uppercase tracking-widest text-accent">
          Invariants
        </h2>
        <div className="mt-4 border border-border bg-bg-secondary p-6">
          <ul className="space-y-3 text-sm leading-6 text-text-secondary">
            {invariants.map((inv, idx) => (
              <li key={idx} className="relative pl-5 before:absolute before:left-0 before:content-['§'] before:text-accent before:font-mono">
                {inv}
              </li>
            ))}
          </ul>
        </div>
      </section>

      {/* Technical reference */}
      <section className="mt-12 mb-16">
        <h2 className="border-b border-border pb-2 font-mono text-xs font-medium uppercase tracking-widest text-accent">
          Technical reference
        </h2>
        <dl className="mt-4 divide-y divide-border border border-border">
          {techRef.map((r) => (
            <div key={r.k} className="grid grid-cols-[10rem_1fr] gap-3 px-4 py-3">
              <dt className="font-mono text-[0.65rem] uppercase tracking-widest text-text-muted">
                {r.k}
              </dt>
              <dd className="font-mono text-xs text-text-secondary">{r.v}</dd>
            </div>
          ))}
        </dl>
      </section>
    </>
  );
}
