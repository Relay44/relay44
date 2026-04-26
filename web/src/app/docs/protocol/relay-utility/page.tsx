import Link from 'next/link';

import { StructuredData } from '@/components/seo/StructuredData';
import { CodeBlock } from '@/components/docs';
import { Card } from '@/components/ui';
import {
  buildBreadcrumbStructuredData,
  buildPageMetadata,
  buildWebPageStructuredData,
} from '@/lib/seo';
import {
  PROTOCOL_NETWORKS,
  STAKING_TIERS,
  basescanAddressUrl,
} from '@/lib/protocol';

export const metadata = buildPageMetadata({
  title: '$RELAY Utility',
  description:
    'How $RELAY is used in the Relay44 Protocol on Base — staking tiers, fee discounts, free x402 access, reward eligibility, and the public utility endpoint and SDK exports for humans and agents.',
  path: '/docs/protocol/relay-utility',
  keywords: ['relay token', 'utility', 'staking', 'x402', 'fee discount', 'base'],
});

const prod = PROTOCOL_NETWORKS.production;

const ENDPOINT = `curl https://relay44-api.onrender.com/v1/protocol/relay-utility | jq`;

const ENDPOINT_RESPONSE = `{
  "chainId": 8453,
  "token": {
    "address": "0x580ff5ae64ec792a949c6123386a8a936c7ebb07",
    "totalSupplyHex": "0x...",
    "decimals": 18
  },
  "staking": {
    "address": "0x709d6006f026950b531d4883260c8416650c5ab7",
    "totalStakedHex": "0x...",
    "tiers": [
      { "tier": 0, "name": "Bronze",  "minRelayWei": "0",                          "feeDiscountBps": 0,    "x402Bypass": false },
      { "tier": 1, "name": "Silver",  "minRelayWei": "1000000000000000000000",     "feeDiscountBps": 2500, "x402Bypass": false },
      { "tier": 2, "name": "Gold",    "minRelayWei": "10000000000000000000000",    "feeDiscountBps": 5000, "x402Bypass": true  },
      { "tier": 3, "name": "Diamond", "minRelayWei": "100000000000000000000000",   "feeDiscountBps": 7500, "x402Bypass": true  }
    ],
    "x402BypassTier": 2
  },
  "rewardDistributor": {
    "address": "0x3c4c0a74f9d108f966908a835a9b4b8d946bbce3"
  },
  "flags": {
    "feeDiscount": true,
    "x402Discount": true,
    "stakingRewards": true,
    "agentRewards": true,
    "creatorRewards": true,
    "governance": false
  },
  "source": "relay44-api",
  "updatedAt": "2026-04-26T..."
}`;

const SDK_USAGE = `import {
  RELAY_TIERS,
  X402_BYPASS_TIER,
  getRelayUtilityAddresses,
  relayTierFromStakedWei,
} from '@relay44/protocol';

const { token, staking, rewardDistributor } = getRelayUtilityAddresses('production');

// Inspect tier metadata without an RPC call:
for (const tier of RELAY_TIERS) {
  console.log(tier.name, tier.minRelayWei.toString(), tier.feeDiscountBps);
}

// Resolve a tier from a wallet's staked balance (wei, 18 decimals):
const tier = relayTierFromStakedWei(15_000n * 10n ** 18n);
console.log(tier.name);          // 'Gold'
console.log(tier.x402Bypass);    // true`;

const AGENT_USAGE = `import { createPublicClient, http } from 'viem';
import { base } from 'viem/chains';
import { qualifyX402OnChain, priceForX402Tier } from '@relay44/agent-sdk';

const client = createPublicClient({ chain: base, transport: http() });

const qualification = await qualifyX402OnChain({
  client,
  network: 'production',
  wallet: '0xYourAgentWallet',
});

if (qualification.bypassesX402) {
  // Tier >= Gold: paid endpoints are free for this wallet.
  return fetch(endpoint);
}

// Otherwise, compute the discounted price the API will quote:
const breakdown = priceForX402Tier(2_500n /* base price in micro-USDC */, qualification);
console.log(breakdown.effectiveMicroUsdc);`;

export default function RelayUtilityPage() {
  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/docs/protocol/relay-utility',
            name: '$RELAY Utility',
            description:
              'How $RELAY is used in the Relay44 Protocol — staking, fee discounts, x402 access, rewards.',
          }),
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Docs', url: '/docs' },
            { name: 'Protocol', url: '/docs/protocol' },
            { name: '$RELAY Utility', url: '/docs/protocol/relay-utility' },
          ]),
        ]}
      />

      <h1 className="text-3xl font-semibold text-text-primary sm:text-4xl">
        $RELAY Utility
      </h1>
      <p className="mt-4 max-w-3xl text-base leading-7 text-text-secondary">
        $RELAY exists to reduce protocol costs and unlock machine-facing access
        on Relay44. Stakers get on-chain order-fee discounts, free x402 API
        access at Gold and above, and eligibility for per-epoch reward
        distributions. This page is the canonical reference for humans and
        agents — the same constants and addresses are exposed via a public
        endpoint and via importable packages.
      </p>

      <Card className="mt-8 p-6">
        <h2 className="text-lg font-semibold text-text-primary">
          What $RELAY does today
        </h2>
        <div className="mt-4 grid gap-6 md:grid-cols-2">
          <div>
            <p className="text-[0.65rem] uppercase tracking-[0.2em] text-text-muted">
              Enforced on-chain
            </p>
            <ul className="mt-2 space-y-1 text-sm leading-6 text-text-secondary">
              <li>• Order-fee discount by staking tier in OrderBook</li>
              <li>• Stake / unstake / claim flows in RelayStaking</li>
              <li>• Per-share reward accounting in RelayStaking</li>
              <li>• Claim paths for agent and creator rewards</li>
            </ul>
          </div>
          <div>
            <p className="text-[0.65rem] uppercase tracking-[0.2em] text-text-muted">
              Enforced server-side
            </p>
            <ul className="mt-2 space-y-1 text-sm leading-6 text-text-secondary">
              <li>• x402 fee bypass at tier ≥ Gold</li>
              <li>• x402 discount at Silver (25%)</li>
              <li>• Per-tier price quoted by <code>/v1/payments/x402/quote</code></li>
            </ul>
          </div>
        </div>
      </Card>

      <Card className="mt-6 p-6">
        <h2 className="text-lg font-semibold text-text-primary">Tier table</h2>
        <p className="mt-2 max-w-3xl text-sm leading-6 text-text-secondary">
          Tiers are read from{' '}
          <a
            href={basescanAddressUrl(prod.contracts.relayStaking!, 'production')}
            target="_blank"
            rel="noreferrer"
            className="font-mono text-text-primary underline-offset-2 hover:underline"
          >
            RelayStaking.getTier(address)
          </a>
          . The same thresholds back the <code>RELAY_TIERS</code> export in{' '}
          <code>@relay44/protocol</code>.
        </p>
        <div className="mt-4 overflow-hidden border border-border">
          <div className="grid grid-cols-12 gap-4 border-b border-border bg-bg-secondary px-4 py-3 text-[0.65rem] uppercase tracking-[0.2em] text-text-muted">
            <span className="col-span-3">Tier</span>
            <span className="col-span-3">Minimum RELAY</span>
            <span className="col-span-2">Fee discount</span>
            <span className="col-span-4">x402 access</span>
          </div>
          {STAKING_TIERS.map((tier, i) => (
            <div
              key={tier.name}
              className="grid grid-cols-12 gap-4 border-b border-border px-4 py-4 text-xs text-text-secondary last:border-b-0"
            >
              <span className="col-span-3 text-sm font-semibold text-text-primary">
                {tier.name}
              </span>
              <code className="col-span-3 text-text-primary">{tier.min}</code>
              <code className="col-span-2 text-text-primary">{tier.feeDiscount}</code>
              <span className="col-span-4">
                {i >= 2 ? 'Free (bypass)' : i === 1 ? '25% discount' : 'Full price'}
              </span>
            </div>
          ))}
        </div>
      </Card>

      <Card className="mt-6 p-6">
        <h2 className="text-lg font-semibold text-text-primary">
          Public endpoint
        </h2>
        <p className="mt-2 max-w-3xl text-sm leading-6 text-text-secondary">
          The single-call utility endpoint returns chain id, RELAY token state
          (totalSupply + decimals), staking address, totalStaked, the four-tier
          table, the reward distributor address, and live utility flags. Safe to
          cache for tens of seconds; updates every block-relevant read.
        </p>
        <div className="mt-4 grid gap-4">
          <CodeBlock language="bash" code={ENDPOINT} />
          <CodeBlock language="json" code={ENDPOINT_RESPONSE} />
        </div>
      </Card>

      <Card className="mt-6 p-6">
        <h2 className="text-lg font-semibold text-text-primary">
          TypeScript: <code>@relay44/protocol</code>
        </h2>
        <p className="mt-2 max-w-3xl text-sm leading-6 text-text-secondary">
          Tier thresholds use <code>bigint</code> to preserve precision past{' '}
          <code>Number.MAX_SAFE_INTEGER</code> and mirror the Solidity
          constants. Addresses come from the deployment manifest so frontends
          and agents never duplicate them.
        </p>
        <div className="mt-4">
          <CodeBlock language="typescript" code={SDK_USAGE} />
        </div>
      </Card>

      <Card className="mt-6 p-6">
        <h2 className="text-lg font-semibold text-text-primary">
          Agents: <code>@relay44/agent-sdk</code>
        </h2>
        <p className="mt-2 max-w-3xl text-sm leading-6 text-text-secondary">
          Agents can resolve their own x402 access state without re-encoding
          tier constants. <code>qualifyX402OnChain</code> reads tier from the
          deployed RelayStaking address;{' '}
          <code>priceForX402Tier</code> mirrors the Rust{' '}
          <code>discounted_amount</code> math so client-side price expectations
          stay aligned with the API.
        </p>
        <div className="mt-4">
          <CodeBlock language="typescript" code={AGENT_USAGE} />
        </div>
      </Card>

      <div className="mt-8 flex flex-wrap gap-3">
        <Link
          href="/tokenomics"
          className="inline-flex h-10 items-center border border-border bg-bg-secondary px-5 text-xs uppercase tracking-widest text-text-primary transition-colors hover:border-border-hover"
        >
          Tokenomics
        </Link>
        <Link
          href="/staking"
          className="inline-flex h-10 items-center border border-border px-5 text-xs uppercase tracking-widest text-text-secondary transition-colors hover:border-border-hover hover:text-text-primary"
        >
          Stake $RELAY
        </Link>
        <Link
          href="/docs/contracts#relayStaking"
          className="inline-flex h-10 items-center border border-border px-5 text-xs uppercase tracking-widest text-text-secondary transition-colors hover:border-border-hover hover:text-text-primary"
        >
          Contract reference
        </Link>
      </div>
    </>
  );
}
