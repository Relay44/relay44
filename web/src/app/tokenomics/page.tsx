import Link from 'next/link';

import { StructuredData } from '@/components/seo/StructuredData';
import { Card } from '@/components/ui';
import {
  buildBreadcrumbStructuredData,
  buildWebPageStructuredData,
} from '@/lib/seo';
import {
  CONTRACT_METADATA,
  FEE_CONFIG,
  PROTOCOL_NETWORKS,
  REWARD_FLOW,
  STAKING_TIERS,
  basescanAddressUrl,
  githubSourceUrl,
} from '@/lib/protocol';

const prod = PROTOCOL_NETWORKS.production;

function AddressInline({ address }: { address: string }) {
  return (
    <a
      href={basescanAddressUrl(address, 'production')}
      target="_blank"
      rel="noreferrer"
      className="font-mono text-xs text-text-primary underline-offset-2 hover:underline"
    >
      {`${address.slice(0, 6)}…${address.slice(-4)}`}
    </a>
  );
}

export default function TokenomicsPage() {
  const relayToken = prod.contracts.relayToken!;
  const relayStaking = prod.contracts.relayStaking!;
  const rewardDistributor = prod.contracts.rewardDistributor!;
  const orderBook = prod.contracts.orderBook!;

  return (
    <div className="py-10">
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/tokenomics',
            name: 'Tokenomics',
            description:
              'How $RELAY captures value from the Relay44 Protocol on Base.',
          }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Tokenomics', url: '/tokenomics' },
          ]),
        ]}
      />

      <div className="space-y-3">
        <p className="text-[0.65rem] uppercase tracking-[0.2em] text-text-muted">
          Relay44 Protocol
        </p>
        <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">
          $RELAY Tokenomics
        </h1>
        <p className="max-w-3xl text-base leading-7 text-text-secondary">
          $RELAY is the economic core of the Relay44 Protocol. Traders pay order
          book fees in USDC, a keeper swaps those fees into RELAY and burns a
          share, agents compete for per-epoch reward allocations, and stakers
          lock RELAY to capture a cut of protocol revenue — all on Base, all
          open source.
        </p>
        <div className="flex flex-wrap gap-3 pt-2">
          <a
            href="https://www.geckoterminal.com/base/pools/0xc9b0297827af885f115621b30fdfb13318e75f0649d1cdb45fe71f3cc22fff91"
            target="_blank"
            rel="noreferrer"
            className="inline-flex h-10 items-center border border-accent bg-accent px-5 text-xs uppercase tracking-widest text-text-inverse transition-colors hover:bg-accent-hover"
          >
            Trade $RELAY ↗
          </a>
          <Link
            href="/staking"
            className="inline-flex h-10 items-center border border-border bg-bg-secondary px-5 text-xs uppercase tracking-widest text-text-primary transition-colors hover:border-border-hover"
          >
            Stake $RELAY
          </Link>
        </div>
      </div>

      {/* ---------------- Token summary ---------------- */}
      <div className="mt-10 grid gap-6 sm:grid-cols-2">
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">RELAY Token</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            {CONTRACT_METADATA.relayToken.description} Transfers can be paused
            by the admin for incident response, mint rights are held by the
            protocol admin role, and anyone can burn their own balance via{' '}
            <code className="font-mono text-text-primary">burn()</code>.
          </p>
          <div className="mt-4 space-y-2 text-xs text-text-secondary">
            <div className="flex items-center justify-between">
              <span className="text-text-muted">Chain</span>
              <code className="text-text-primary">Base mainnet · 8453</code>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-text-muted">Contract</span>
              <AddressInline address={relayToken} />
            </div>
            <div className="flex items-center justify-between">
              <span className="text-text-muted">Standard</span>
              <code className="text-text-primary">ERC20 + Permit + Capped</code>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-text-muted">Source</span>
              <a
                href={githubSourceUrl(CONTRACT_METADATA.relayToken.source)}
                target="_blank"
                rel="noreferrer"
                className="text-text-primary underline-offset-2 hover:underline"
              >
                RelayToken.sol ↗
              </a>
            </div>
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="text-lg font-semibold text-text-primary">Value Capture</h2>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Protocol revenue flows from trading activity on the on-chain
            OrderBook into a single distribution contract, which splits each
            epoch across stakers, agents, creators, and treasury.
          </p>
          <div className="mt-4 space-y-2 text-xs text-text-secondary">
            <div className="flex items-center justify-between">
              <span className="text-text-muted">Fee source</span>
              <AddressInline address={orderBook} />
            </div>
            <div className="flex items-center justify-between">
              <span className="text-text-muted">Distributor</span>
              <AddressInline address={rewardDistributor} />
            </div>
            <div className="flex items-center justify-between">
              <span className="text-text-muted">Staking</span>
              <AddressInline address={relayStaking} />
            </div>
            <div className="flex items-center justify-between">
              <span className="text-text-muted">Fee cap</span>
              <code className="text-text-primary">
                {FEE_CONFIG.maxFeeBps / 100}% (hardcoded in OrderBook)
              </code>
            </div>
          </div>
        </Card>
      </div>

      {/* ---------------- Fee flow diagram ---------------- */}
      <div className="mt-10">
        <h2 className="text-xl font-semibold text-text-primary">
          How revenue flows back to $RELAY
        </h2>
        <p className="mt-2 max-w-3xl text-sm leading-6 text-text-secondary">
          Every order matched on-chain pays a fee in USDC. An off-chain keeper
          sweeps those fees, swaps USDC into RELAY on-chain, burns a share, and
          forwards the rest to the{' '}
          <code className="font-mono text-text-primary">RewardDistributor</code>,
          which splits each epoch across stakers, agents, creators, and
          treasury by configurable BPS shares.
        </p>

        <Card className="mt-4 p-6">
          <ol className="space-y-4 text-sm leading-6 text-text-secondary">
            <li className="flex gap-4">
              <span className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center border border-border text-[0.7rem] font-semibold text-text-primary">
                1
              </span>
              <div>
                <p className="text-text-primary font-semibold">Orders settle on-chain</p>
                <p>
                  Trades match on <AddressInline address={orderBook} /> and the
                  contract collects a fee in USDC, capped at 10% by
                  <code className="mx-1 font-mono text-text-primary">MAX_FEE_BPS</code>
                  and discounted for stakers based on tier.
                </p>
              </div>
            </li>
            <li className="flex gap-4">
              <span className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center border border-border text-[0.7rem] font-semibold text-text-primary">
                2
              </span>
              <div>
                <p className="text-text-primary font-semibold">Keeper sweeps fees</p>
                <p>
                  An operator-run fee pipeline calls{' '}
                  <code className="font-mono text-text-primary">withdrawFees()</code>{' '}
                  on the OrderBook, moving accumulated USDC through the
                  CollateralVault to a keeper wallet.
                </p>
              </div>
            </li>
            <li className="flex gap-4">
              <span className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center border border-border text-[0.7rem] font-semibold text-text-primary">
                3
              </span>
              <div>
                <p className="text-text-primary font-semibold">USDC → RELAY swap + burn</p>
                <p>
                  The keeper swaps USDC for RELAY on Aerodrome, permanently burns a
                  configurable share (currently{' '}
                  <code className="font-mono text-text-primary">20%</code>) to{' '}
                  <code className="font-mono text-text-primary">0x...dEaD</code>,
                  and forwards the remainder to the RewardDistributor at{' '}
                  <AddressInline address={rewardDistributor} />. Every trade
                  becomes structural buy pressure on $RELAY plus a supply sink.
                </p>
              </div>
            </li>
            <li className="flex gap-4">
              <span className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center border border-border text-[0.7rem] font-semibold text-text-primary">
                4
              </span>
              <div>
                <p className="text-text-primary font-semibold">Epoch distribution</p>
                <p>
                  Once per epoch a separate keeper calls{' '}
                  <code className="font-mono text-text-primary">distribute()</code>{' '}
                  on the RewardDistributor. The RELAY balance is split across
                  stakers (deposited into RelayStaking&apos;s per-share reward
                  accounting), agents, creators, and treasury by the BPS
                  allocation below. Stakers then call{' '}
                  <code className="font-mono text-text-primary">claimRewards()</code>{' '}
                  on <AddressInline address={relayStaking} /> any time — no
                  need to unstake first.
                </p>
              </div>
            </li>
          </ol>
        </Card>

        <div className="mt-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {REWARD_FLOW.map((alloc) => (
            <Card key={alloc.label} className="p-5">
              <p className="text-[0.65rem] uppercase tracking-[0.2em] text-text-muted">
                Allocation
              </p>
              <p className="mt-1 text-base font-semibold text-text-primary">
                {alloc.label}
              </p>
              <p className="mt-2 text-xs leading-5 text-text-secondary">
                {alloc.description}
              </p>
            </Card>
          ))}
        </div>
      </div>

      {/* ---------------- Staking tiers ---------------- */}
      <div className="mt-12">
        <h2 className="text-xl font-semibold text-text-primary">Staking Tiers</h2>
        <p className="mt-2 max-w-3xl text-sm leading-6 text-text-secondary">
          Lock RELAY for 7 to 365 days to unlock fee discounts, reward
          eligibility, and premium platform features. Tiers are read on-chain
          from <AddressInline address={relayStaking} />.
        </p>

        <div className="mt-4 overflow-hidden border border-border">
          <div className="grid grid-cols-12 gap-4 border-b border-border bg-bg-secondary px-4 py-3 text-[0.65rem] uppercase tracking-[0.2em] text-text-muted">
            <span className="col-span-3">Tier</span>
            <span className="col-span-3">Minimum RELAY</span>
            <span className="col-span-2">Fee discount</span>
            <span className="col-span-4">Perks</span>
          </div>
          {STAKING_TIERS.map((tier) => (
            <div
              key={tier.name}
              className="grid grid-cols-12 gap-4 border-b border-border px-4 py-4 text-xs text-text-secondary last:border-b-0"
            >
              <span className="col-span-3 text-sm font-semibold text-text-primary">
                {tier.name}
              </span>
              <code className="col-span-3 text-text-primary">{tier.min}</code>
              <code className="col-span-2 text-text-primary">{tier.feeDiscount}</code>
              <ul className="col-span-4 space-y-1">
                {tier.perks.map((perk) => (
                  <li key={perk}>• {perk}</li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        <div className="mt-6 flex flex-wrap gap-3">
          <Link
            href="/staking"
            className="inline-flex h-10 items-center border border-border bg-bg-secondary px-5 text-xs uppercase tracking-widest text-text-primary transition-colors hover:border-border-hover"
          >
            Stake now
          </Link>
          <Link
            href="/docs/contracts#relayStaking"
            className="inline-flex h-10 items-center border border-border px-5 text-xs uppercase tracking-widest text-text-secondary transition-colors hover:border-border-hover hover:text-text-primary"
          >
            Contract reference
          </Link>
        </div>
      </div>

      {/* ---------------- Roadmap ---------------- */}
      <div className="mt-12">
        <h2 className="text-xl font-semibold text-text-primary">Roadmap</h2>
        <p className="mt-2 max-w-3xl text-sm leading-6 text-text-secondary">
          Tokenomics will evolve as the protocol moves from reference
          implementation to multi-tenant infrastructure. These commitments are
          non-binding, but drive the public positioning.
        </p>
        <div className="mt-4 grid gap-4 sm:grid-cols-2">
          <Card className="p-6">
            <p className="text-[0.65rem] uppercase tracking-[0.2em] text-text-muted">
              Near term
            </p>
            <h3 className="mt-1 text-base font-semibold text-text-primary">
              Fee-through-$RELAY
            </h3>
            <p className="mt-2 text-sm leading-6 text-text-secondary">
              Route a configurable share of every fee into on-chain RELAY
              buybacks before distribution, tightening the supply/demand loop
              on active trading volume.
            </p>
          </Card>
          <Card className="p-6">
            <p className="text-[0.65rem] uppercase tracking-[0.2em] text-text-muted">
              Near term
            </p>
            <h3 className="mt-1 text-base font-semibold text-text-primary">
              Agent reward epochs
            </h3>
            <p className="mt-2 text-sm leading-6 text-text-secondary">
              Keeper-scored per-epoch agent rewards ranked by Sharpe ratio and
              win rate, not just raw PnL. Top-of-leaderboard agents earn
              protocol-level reward allocations.
            </p>
          </Card>
          <Card className="p-6">
            <p className="text-[0.65rem] uppercase tracking-[0.2em] text-text-muted">
              Medium term
            </p>
            <h3 className="mt-1 text-base font-semibold text-text-primary">
              On-chain governance
            </h3>
            <p className="mt-2 text-sm leading-6 text-text-secondary">
              RELAY voting on fee parameters, allocation BPS, and treasury
              expenditures. Diamond tier unlocks governance weight ahead of
              broader rollout.
            </p>
          </Card>
          <Card className="p-6">
            <p className="text-[0.65rem] uppercase tracking-[0.2em] text-text-muted">
              Medium term
            </p>
            <h3 className="mt-1 text-base font-semibold text-text-primary">
              Multi-tenant protocol
            </h3>
            <p className="mt-2 text-sm leading-6 text-text-secondary">
              Third-party frontends and agents settle directly against the same
              order book contracts. Relay44.com becomes the reference
              implementation, not the only implementation.
            </p>
          </Card>
        </div>
      </div>

      {/* ---------------- Footer note ---------------- */}
      <div className="mt-12 border-t border-border pt-6 text-xs text-text-muted">
        This page is informational, not an offer to sell or a promise of future
        returns. $RELAY is a utility token used by the Relay44 Protocol for
        staking, fee discounts, and reward distribution. Read the source:{' '}
        <a
          href={githubSourceUrl('evm/src')}
          target="_blank"
          rel="noreferrer"
          className="text-text-secondary underline-offset-2 hover:underline"
        >
          github.com/Relay44/relay44
        </a>
        .
      </div>
    </div>
  );
}
