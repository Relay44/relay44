// Static RELAY token utility metadata. Mirrors the constants defined in
// `evm/src/RelayStaking.sol::getTier` and `evm/src/OrderBook.sol::TIERn_THRESHOLD`,
// the runtime selectors in `app/src/services/staking.rs`, and the public response
// shape from `GET /v1/protocol/relay-utility`.
//
// Address values are sourced from the generated deployment manifest so that this
// file never duplicates the address constants.

import {
  type Address,
  type NetworkName,
  deploymentManifest,
  getContractAddress,
} from './generated';

export const RELAY_DECIMALS = 18;

/** Tier id at which an x402-priced request is fully bypassed (free access). */
export const X402_BYPASS_TIER = 2;

export type RelayTierId = 0 | 1 | 2 | 3;

export interface RelayTier {
  readonly tier: RelayTierId;
  readonly name: 'Bronze' | 'Silver' | 'Gold' | 'Diamond';
  /** Minimum staked RELAY (in wei, 18 decimals) to qualify for this tier. */
  readonly minRelayWei: bigint;
  /** Order-fee discount applied at match time, in bps (10_000 = 100%). */
  readonly feeDiscountBps: number;
  /** True if x402-priced API access is free for this tier. */
  readonly x402Bypass: boolean;
}

export const RELAY_TIERS: readonly RelayTier[] = [
  {
    tier: 0,
    name: 'Bronze',
    minRelayWei: 0n,
    feeDiscountBps: 0,
    x402Bypass: false,
  },
  {
    tier: 1,
    name: 'Silver',
    minRelayWei: 1_000n * 10n ** 18n,
    feeDiscountBps: 2_500,
    x402Bypass: false,
  },
  {
    tier: 2,
    name: 'Gold',
    minRelayWei: 10_000n * 10n ** 18n,
    feeDiscountBps: 5_000,
    x402Bypass: true,
  },
  {
    tier: 3,
    name: 'Diamond',
    minRelayWei: 100_000n * 10n ** 18n,
    feeDiscountBps: 7_500,
    x402Bypass: true,
  },
] as const;

/** Returns the tier metadata for a given staked-RELAY balance (in wei). */
export function relayTierFromStakedWei(stakedWei: bigint): RelayTier {
  let match = RELAY_TIERS[0];
  for (const tier of RELAY_TIERS) {
    if (stakedWei >= tier.minRelayWei) {
      match = tier;
    }
  }
  return match;
}

/** Returns the tier metadata for a tier id (0..3). */
export function relayTierById(tier: number): RelayTier {
  const found = RELAY_TIERS.find((t) => t.tier === tier);
  if (!found) {
    throw new Error(`Unknown RELAY tier id: ${tier}`);
  }
  return found;
}

export interface RelayUtilityAddresses {
  readonly token: Address;
  readonly staking: Address;
  readonly rewardDistributor: Address;
}

/** Returns the RELAY utility contract addresses for a deployed network. */
export function getRelayUtilityAddresses(network: NetworkName): RelayUtilityAddresses {
  return {
    token: getContractAddress(network, 'relayToken'),
    staking: getContractAddress(network, 'relayStaking'),
    rewardDistributor: getContractAddress(network, 'rewardDistributor'),
  };
}

/** Convenience: chain id for the requested deployment environment. */
export function getRelayChainId(network: NetworkName): number {
  return deploymentManifest.environments[network].chainId;
}
