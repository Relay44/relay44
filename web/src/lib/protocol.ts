/**
 * Protocol reference metadata.
 *
 * Single source of truth for public docs pages (`/docs/contracts`, `/tokenomics`)
 * describing the Relay44 Protocol deployment on Base. Addresses are mirrored from
 * `config/deployments/base-addresses.json` so the docs pages stay in sync with the
 * operator-maintained deployment record without a build-time import from outside
 * `web/`.
 */

export type ContractName =
  | 'marketCore'
  | 'orderBook'
  | 'collateralVault'
  | 'agentRuntime'
  | 'collateralToken'
  | 'relayToken'
  | 'relayStaking'
  | 'rewardDistributor'
  | 'agentIdentityRegistry'
  | 'agentReputationRegistry'
  | 'erc8004IdentityRegistry'
  | 'erc8004ReputationRegistry'
  | 'erc8004ValidationRegistry';

export type NetworkName = 'production' | 'staging';

interface NetworkEntry {
  label: string;
  chain: string;
  chainId: number;
  rpc: string;
  explorer: string;
  contracts: Partial<Record<ContractName, string | null>>;
}

/**
 * Mainnet and sepolia contract addresses. Mirror of
 * `config/deployments/base-addresses.json` — update in both places when a new
 * deployment is cut.
 */
export const PROTOCOL_NETWORKS: Record<NetworkName, NetworkEntry> = {
  production: {
    label: 'Mainnet',
    chain: 'Base',
    chainId: 8453,
    rpc: 'https://mainnet.base.org',
    explorer: 'https://basescan.org',
    contracts: {
      marketCore: '0xc9259a18696Ecbf7636C1a01F40Bc9d47e249AE8',
      orderBook: '0xFe8aA303Ab953037023b12326D354f6d2484D4ce',
      collateralVault: '0x4420dd803e6E363e6af079e6b77CA03B93f8dAe0',
      agentRuntime: '0xC44d686548513FF2a921201fa0811B1f30AA1a65',
      collateralToken: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
      relayToken: '0x580fF5Ae64eC792A949c6123386A8A936c7EBB07',
      relayStaking: '0x709D6006f026950b531d4883260c8416650c5AB7',
      rewardDistributor: '0x3c4c0A74F9d108F966908a835a9b4b8D946bBce3',
      agentIdentityRegistry: '0xae5c5f3970f4d3AadbE77368B09f424B6F1C09FC',
      agentReputationRegistry: '0x7A6424954D3f0ACAcC9Db798e1801650A48D9e49',
      erc8004IdentityRegistry: '0xda55218BC69C0cD52b4927F908606cB84D7ab994',
      erc8004ReputationRegistry: '0x56636eE11F4c42272C669a0E6C1cdC63ADAbF03e',
      erc8004ValidationRegistry: '0x095D11eE0AdEb7c3d5088f3FB7ea8ebB7d05A540',
    },
  },
  staging: {
    label: 'Sepolia',
    chain: 'Base Sepolia',
    chainId: 84532,
    rpc: 'https://sepolia.base.org',
    explorer: 'https://sepolia.basescan.org',
    contracts: {
      marketCore: '0x823d54726ddc48a784ee6eb53235f5d68c94f1c0',
      orderBook: '0xc6ec840da50fc708bf155b5b15a585c5d0004ebf',
      collateralVault: '0x588e19e5831ddc8aed5c8e0d687f86884ff98ee2',
      agentRuntime: null,
      collateralToken: '0x036CbD53842c5426634e7929541eC2318f3dCF7e',
    },
  },
};

export function basescanAddressUrl(
  address: string,
  network: NetworkName = 'production',
): string {
  return `${PROTOCOL_NETWORKS[network].explorer}/address/${address}`;
}

export interface ContractMeta {
  label: string;
  short: string;
  description: string;
  category: 'core' | 'token' | 'agent' | 'identity';
  /** ABI identifier served at `/api/contracts/[name]/abi`. Null means not exported. */
  abiKey: string | null;
  /** Path within the monorepo for the source file. */
  source: string;
}

/**
 * Human-readable metadata for each contract. Ordered in the canonical order we
 * display on `/docs/contracts`.
 */
export const CONTRACT_METADATA: Record<ContractName, ContractMeta> = {
  marketCore: {
    label: 'MarketCore',
    short: 'Markets',
    category: 'core',
    abiKey: 'market-core',
    source: 'evm/src/MarketCore.sol',
    description:
      'Creates binary prediction markets, mints outcome shares, and settles from the oracle. The primary entry point for market lifecycle on Base.',
  },
  orderBook: {
    label: 'OrderBook',
    short: 'Matching',
    category: 'core',
    abiKey: 'order-book',
    source: 'evm/src/OrderBook.sol',
    description:
      'On-chain limit order book with price-time priority. Supports maker/taker fees, tier-based fee discounts for stakers, and agent-executed orders.',
  },
  collateralVault: {
    label: 'CollateralVault',
    short: 'Vault',
    category: 'core',
    abiKey: null,
    source: 'evm/src/CollateralVault.sol',
    description:
      'Holds USDC collateral for open positions and pending orders. Non-custodial — only the market and order book contracts can move balances.',
  },
  agentRuntime: {
    label: 'AgentRuntime',
    short: 'Runtime',
    category: 'agent',
    abiKey: null,
    source: 'evm/src/AgentRuntime.sol',
    description:
      'Lets whitelisted agents place and cancel orders on behalf of their owners without holding keys. Enforces per-agent risk limits.',
  },
  collateralToken: {
    label: 'USDC (Collateral)',
    short: 'USDC',
    category: 'token',
    abiKey: 'erc20',
    source: 'evm/src/CollateralVault.sol',
    description:
      'Native USDC on Base. The platform-wide collateral asset for all orders, positions, and distribution markets.',
  },
  relayToken: {
    label: 'RELAY Token',
    short: '$RELAY',
    category: 'token',
    abiKey: 'erc20',
    source: 'evm/src/RelayToken.sol',
    description:
      'Capped-supply, pausable ERC20Permit. Used for staking, fee rebates, and reward distribution across the protocol.',
  },
  relayStaking: {
    label: 'RelayStaking',
    short: 'Staking',
    category: 'token',
    abiKey: 'relay-staking',
    source: 'evm/src/RelayStaking.sol',
    description:
      'Lock RELAY for 7–365 days to unlock Silver/Gold/Diamond tiers, fee discounts, and a per-share claim on protocol rewards.',
  },
  rewardDistributor: {
    label: 'RewardDistributor',
    short: 'Rewards',
    category: 'token',
    abiKey: null,
    source: 'evm/src/RewardDistributor.sol',
    description:
      'Routes incoming fee revenue to stakers, top agents, market creators, and treasury each epoch. Keeper-triggered distribution with configurable BPS shares.',
  },
  agentIdentityRegistry: {
    label: 'AgentIdentityRegistry',
    short: 'Agent ID',
    category: 'agent',
    abiKey: null,
    source: 'evm/src/AgentIdentityRegistry.sol',
    description:
      'Registers owner-controlled agents with strategy metadata. Primary on-chain lookup table for AgentRuntime authorization.',
  },
  agentReputationRegistry: {
    label: 'AgentReputationRegistry',
    short: 'Agent Rep',
    category: 'agent',
    abiKey: null,
    source: 'evm/src/AgentReputationRegistry.sol',
    description:
      'Tracks trading outcomes per agent and derives a reputation score used for tier gating and reward eligibility.',
  },
  erc8004IdentityRegistry: {
    label: 'ERC-8004 Identity',
    short: 'ERC-8004 ID',
    category: 'identity',
    abiKey: null,
    source: 'evm/src/ERC8004IdentityRegistry.sol',
    description:
      'Soulbound identity NFTs implementing the ERC-8004 identity standard for humans and agents.',
  },
  erc8004ReputationRegistry: {
    label: 'ERC-8004 Reputation',
    short: 'ERC-8004 Rep',
    category: 'identity',
    abiKey: null,
    source: 'evm/src/ERC8004ReputationRegistry.sol',
    description:
      'Dual-feedback (positive/negative) reputation registry with tag-based filtering. Compatible with ERC-8004 identity subjects.',
  },
  erc8004ValidationRegistry: {
    label: 'ERC-8004 Validation',
    short: 'ERC-8004 Val',
    category: 'identity',
    abiKey: null,
    source: 'evm/src/ERC8004ValidationRegistry.sol',
    description:
      'Validator attestations for ERC-8004 subjects. Supports challenge-response and off-chain proof verification.',
  },
};

/** Canonical display order on the contracts reference page. */
export const CONTRACT_ORDER: ContractName[] = [
  'marketCore',
  'orderBook',
  'collateralVault',
  'agentRuntime',
  'relayToken',
  'relayStaking',
  'rewardDistributor',
  'collateralToken',
  'agentIdentityRegistry',
  'agentReputationRegistry',
  'erc8004IdentityRegistry',
  'erc8004ReputationRegistry',
  'erc8004ValidationRegistry',
];

// ---------------------------------------------------------------------------
// Tokenomics constants
// ---------------------------------------------------------------------------

export interface StakingTier {
  name: 'Bronze' | 'Silver' | 'Gold' | 'Diamond';
  min: string;
  feeDiscount: string;
  perks: string[];
}

export const STAKING_TIERS: StakingTier[] = [
  {
    name: 'Bronze',
    min: '0',
    feeDiscount: '0%',
    perks: ['Base access to trading, markets, and agents'],
  },
  {
    name: 'Silver',
    min: '1,000',
    feeDiscount: '25%',
    perks: ['25% maker/taker fee discount', 'Priority market data'],
  },
  {
    name: 'Gold',
    min: '10,000',
    feeDiscount: '50%',
    perks: ['50% fee discount', 'Agent API access', 'Reward share multiplier'],
  },
  {
    name: 'Diamond',
    min: '100,000',
    feeDiscount: '75%',
    perks: [
      '75% fee discount',
      'Top reward share tier',
      'Premium agent templates',
      'Governance weight (planned)',
    ],
  },
];

export interface RewardAllocation {
  label: string;
  description: string;
}

export const REWARD_FLOW: RewardAllocation[] = [
  {
    label: 'Stakers — 20%',
    description:
      'Deposited into RelayStaking each epoch and paid out through a per-share accRewardPerShare model. Claimable any time without unstaking.',
  },
  {
    label: 'Agents — 40%',
    description:
      'Held per-epoch for top-performing agents. Keeper assigns per-agent amounts based on rankings; agents claim via claimAgentReward().',
  },
  {
    label: 'Creators — 30%',
    description:
      'Held per-epoch for market creators. Keeper allocates based on volume traded against their markets; creators claim via claimCreatorReward().',
  },
  {
    label: 'Treasury — 10%',
    description:
      'Transferred directly to the protocol treasury each epoch to fund audits, reserves, and ecosystem grants. Shares are adjustable by admin via setShares().',
  },
];

export const FEE_CONFIG = {
  maxFeeBps: 1000,
  description:
    'OrderBook.setFeeConfig caps the protocol fee at 10% (1000 bps). Current live rates are published in the RPC view calls on each order book — stakers get an automatic discount from the tier table above.',
};
