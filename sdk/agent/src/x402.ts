// x402 tier-qualification helpers for agents calling Relay44 paid endpoints.
//
// The protocol charges a base x402 price per resource. Stakers get a discount
// (or full bypass at tier >= X402_BYPASS_TIER). These helpers are read-only
// and let an agent decide locally whether a wallet should attach a payment
// signature, request a bypass, or expect a discounted quote — without
// duplicating tier constants on the client.

import {
  RELAY_TIERS,
  RELAY_DECIMALS,
  X402_BYPASS_TIER,
  relayTierById,
  relayTierFromStakedWei,
  type RelayTier,
  type NetworkName,
  getRelayUtilityAddresses,
  RELAY_STAKING_ABI,
} from '@relay44/protocol';

export { RELAY_TIERS, RELAY_DECIMALS, X402_BYPASS_TIER, type RelayTier };

export interface X402Qualification {
  /** Resolved tier metadata. */
  tier: RelayTier;
  /** True iff the wallet bypasses x402 charges entirely. */
  bypassesX402: boolean;
  /**
   * Discount applied to the base x402 price, in basis points (10_000 = 100%).
   * Equal to 10_000 when the tier bypasses x402, otherwise the tier's
   * underlying fee-discount bps.
   */
  x402DiscountBps: number;
}

function buildQualification(tier: RelayTier): X402Qualification {
  return {
    tier,
    bypassesX402: tier.x402Bypass,
    x402DiscountBps: tier.x402Bypass ? 10_000 : tier.feeDiscountBps,
  };
}

/** Qualify x402 access from a known tier id (0..3). */
export function qualifyX402ByTier(tier: number): X402Qualification {
  return buildQualification(relayTierById(tier));
}

/** Qualify x402 access from a wallet's staked RELAY balance (in wei). */
export function qualifyX402FromStaked(stakedWei: bigint): X402Qualification {
  return buildQualification(relayTierFromStakedWei(stakedWei));
}

export interface X402PriceBreakdown {
  /** Base price quoted by the resource, in micro-USDC (10^-6 USDC). */
  baseMicroUsdc: bigint;
  /** Effective price after applying the wallet's tier, in micro-USDC. */
  effectiveMicroUsdc: bigint;
  qualification: X402Qualification;
}

/**
 * Compute the effective x402 price for a wallet's tier.
 *
 * Mirrors `app/src/services/staking::discounted_amount`: tier >= X402_BYPASS_TIER
 * pays nothing, otherwise the tier's fee-discount bps are applied.
 */
export function priceForX402Tier(
  baseMicroUsdc: bigint,
  qualification: X402Qualification,
): X402PriceBreakdown {
  if (qualification.bypassesX402) {
    return { baseMicroUsdc, effectiveMicroUsdc: 0n, qualification };
  }
  if (qualification.x402DiscountBps === 0) {
    return { baseMicroUsdc, effectiveMicroUsdc: baseMicroUsdc, qualification };
  }
  const discount = (baseMicroUsdc * BigInt(qualification.x402DiscountBps)) / 10_000n;
  return {
    baseMicroUsdc,
    effectiveMicroUsdc: baseMicroUsdc - discount,
    qualification,
  };
}

/**
 * Minimal read-client contract — anything with an `eth_call` of `getTier(address)`
 * works (e.g. a viem `PublicClient.readContract` or a hand-rolled JSON-RPC).
 */
export interface RelayStakingReader {
  readContract: (args: {
    address: `0x${string}`;
    abi: typeof RELAY_STAKING_ABI;
    functionName: 'getTier';
    args: [`0x${string}`];
  }) => Promise<bigint | number>;
}

export interface QualifyOnChainOptions {
  client: RelayStakingReader;
  network: NetworkName;
  wallet: `0x${string}`;
}

/**
 * Read the wallet's tier from the deployed RelayStaking contract on the given
 * network and return its x402 qualification.
 */
export async function qualifyX402OnChain({
  client,
  network,
  wallet,
}: QualifyOnChainOptions): Promise<X402Qualification> {
  const { staking } = getRelayUtilityAddresses(network);
  const result = await client.readContract({
    address: staking,
    abi: RELAY_STAKING_ABI,
    functionName: 'getTier',
    args: [wallet],
  });
  const tierNum = typeof result === 'bigint' ? Number(result) : result;
  return qualifyX402ByTier(tierNum);
}
