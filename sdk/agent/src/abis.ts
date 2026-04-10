// Canonical ABIs for Relay44 core contracts.
//
// These static ABIs are kept in sync with `web/src/lib/contracts.ts` (the
// source of truth used to power the public `/api/contracts/:name/abi`
// endpoint at https://relay44.com). A parity check runs in `scripts/
// check-abi-parity.mjs` to make sure the two copies cannot drift silently.
//
// Consumers that want to always fetch the latest on-chain ABI without
// redeploying can call `fetchContractAbi('market-core')` — see the `fetchAbi`
// helper exported from `./index`.

export const MARKET_CORE_ABI = [
  {
    type: 'function',
    name: 'createMarket',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'questionHash', type: 'bytes32' },
      { name: 'closeTime', type: 'uint64' },
      { name: 'resolver', type: 'address' },
    ],
    outputs: [{ name: 'marketId', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'createMarketRich',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'question', type: 'string' },
      { name: 'description', type: 'string' },
      { name: 'category', type: 'string' },
      { name: 'resolutionSource', type: 'string' },
      { name: 'closeTime', type: 'uint64' },
      { name: 'resolver', type: 'address' },
    ],
    outputs: [{ name: 'marketId', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'marketCount',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'markets',
    stateMutability: 'view',
    inputs: [{ name: 'marketId', type: 'uint256' }],
    outputs: [
      { name: 'questionHash', type: 'bytes32' },
      { name: 'closeTime', type: 'uint64' },
      { name: 'resolveTime', type: 'uint64' },
      { name: 'resolver', type: 'address' },
      { name: 'resolved', type: 'bool' },
      { name: 'outcome', type: 'bool' },
    ],
  },
  {
    type: 'function',
    name: 'getMarketMetadata',
    stateMutability: 'view',
    inputs: [{ name: 'marketId', type: 'uint256' }],
    outputs: [
      { name: 'question', type: 'string' },
      { name: 'description', type: 'string' },
      { name: 'category', type: 'string' },
      { name: 'resolutionSource', type: 'string' },
    ],
  },
  {
    type: 'function',
    name: 'resolveMarket',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'marketId', type: 'uint256' },
      { name: 'outcome', type: 'bool' },
    ],
    outputs: [],
  },
] as const;

export const ORDER_BOOK_ABI = [
  {
    type: 'function',
    name: 'placeOrder',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'marketId', type: 'uint256' },
      { name: 'isYes', type: 'bool' },
      { name: 'priceBps', type: 'uint128' },
      { name: 'size', type: 'uint128' },
      { name: 'expiry', type: 'uint64' },
    ],
    outputs: [{ name: 'orderId', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'cancelOrder',
    stateMutability: 'nonpayable',
    inputs: [{ name: 'orderId', type: 'uint256' }],
    outputs: [],
  },
  {
    type: 'function',
    name: 'claim',
    stateMutability: 'nonpayable',
    inputs: [{ name: 'marketId', type: 'uint256' }],
    outputs: [{ name: 'payout', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'claimFor',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'user', type: 'address' },
      { name: 'marketId', type: 'uint256' },
    ],
    outputs: [{ name: 'payout', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'claimable',
    stateMutability: 'view',
    inputs: [
      { name: 'marketId', type: 'uint256' },
      { name: 'user', type: 'address' },
    ],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'orderCount',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'orders',
    stateMutability: 'view',
    inputs: [{ name: 'orderId', type: 'uint256' }],
    outputs: [
      { name: 'maker', type: 'address' },
      { name: 'marketId', type: 'uint256' },
      { name: 'isYes', type: 'bool' },
      { name: 'priceBps', type: 'uint128' },
      { name: 'size', type: 'uint128' },
      { name: 'remaining', type: 'uint128' },
      { name: 'expiry', type: 'uint64' },
      { name: 'canceled', type: 'bool' },
    ],
  },
  {
    type: 'function',
    name: 'positions',
    stateMutability: 'view',
    inputs: [
      { name: 'marketId', type: 'uint256' },
      { name: 'user', type: 'address' },
    ],
    outputs: [
      { name: 'yesShares', type: 'uint128' },
      { name: 'noShares', type: 'uint128' },
      { name: 'claimed', type: 'bool' },
    ],
  },
] as const;

export const ERC20_ABI = [
  {
    type: 'function',
    name: 'balanceOf',
    stateMutability: 'view',
    inputs: [{ name: 'account', type: 'address' }],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'allowance',
    stateMutability: 'view',
    inputs: [
      { name: 'owner', type: 'address' },
      { name: 'spender', type: 'address' },
    ],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'approve',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'spender', type: 'address' },
      { name: 'amount', type: 'uint256' },
    ],
    outputs: [{ name: '', type: 'bool' }],
  },
  {
    type: 'function',
    name: 'decimals',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'uint8' }],
  },
  {
    type: 'function',
    name: 'symbol',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'string' }],
  },
  {
    type: 'function',
    name: 'totalSupply',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'uint256' }],
  },
] as const;

export const RELAY_STAKING_ABI = [
  {
    type: 'function',
    name: 'stake',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'amount', type: 'uint256' },
      { name: 'lockDuration', type: 'uint64' },
    ],
    outputs: [],
  },
  {
    type: 'function',
    name: 'unstake',
    stateMutability: 'nonpayable',
    inputs: [],
    outputs: [],
  },
  {
    type: 'function',
    name: 'claimRewards',
    stateMutability: 'nonpayable',
    inputs: [],
    outputs: [],
  },
  {
    type: 'function',
    name: 'extendLock',
    stateMutability: 'nonpayable',
    inputs: [{ name: 'newUnlockAt', type: 'uint64' }],
    outputs: [],
  },
  {
    type: 'function',
    name: 'stakes',
    stateMutability: 'view',
    inputs: [{ name: 'user', type: 'address' }],
    outputs: [
      { name: 'amount', type: 'uint256' },
      { name: 'lockedAt', type: 'uint64' },
      { name: 'unlockAt', type: 'uint64' },
      { name: 'rewardDebt', type: 'uint256' },
    ],
  },
  {
    type: 'function',
    name: 'stakeOf',
    stateMutability: 'view',
    inputs: [{ name: 'user', type: 'address' }],
    outputs: [
      { name: 'amount', type: 'uint256' },
      { name: 'unlockAt', type: 'uint64' },
    ],
  },
  {
    type: 'function',
    name: 'getTier',
    stateMutability: 'view',
    inputs: [{ name: 'user', type: 'address' }],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'pendingRewardOf',
    stateMutability: 'view',
    inputs: [{ name: 'user', type: 'address' }],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'totalStaked',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'accRewardPerShare',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'uint256' }],
  },
] as const;

export const MARKET_CREATED_EVENT_ABI = [
  {
    type: 'event',
    name: 'MarketCreated',
    inputs: [
      { indexed: true, name: 'marketId', type: 'uint256' },
      { indexed: true, name: 'questionHash', type: 'bytes32' },
      { indexed: false, name: 'closeTime', type: 'uint64' },
      { indexed: false, name: 'resolver', type: 'address' },
    ],
  },
] as const;

export const ORDER_PLACED_EVENT_ABI = [
  {
    type: 'event',
    name: 'OrderPlaced',
    inputs: [
      { indexed: true, name: 'orderId', type: 'uint256' },
      { indexed: true, name: 'maker', type: 'address' },
      { indexed: true, name: 'marketId', type: 'uint256' },
      { indexed: false, name: 'isYes', type: 'bool' },
      { indexed: false, name: 'priceBps', type: 'uint128' },
      { indexed: false, name: 'size', type: 'uint128' },
      { indexed: false, name: 'expiry', type: 'uint64' },
    ],
  },
] as const;

// ---------------------------------------------------------------------------
// Live ABI fetching
// ---------------------------------------------------------------------------

/**
 * Names of ABIs published at https://relay44.com/api/contracts/:name/abi.
 * Mirror the keys used by `web/src/app/api/contracts/[name]/abi/route.ts`.
 */
export type ContractAbiName =
  | 'market-core'
  | 'order-book'
  | 'erc20'
  | 'relay-staking';

/**
 * Default base URL for the public ABI endpoint. Override with
 * `RELAY44_CONTRACTS_URL` or the `baseUrl` argument when running against a
 * preview/staging deploy.
 */
export const DEFAULT_CONTRACTS_BASE_URL = 'https://relay44.com';

interface AbiResponse {
  name: string;
  abi: readonly Record<string, unknown>[];
}

/**
 * Fetch the canonical ABI for a Relay44 core contract from the public
 * contracts endpoint. Prefer this helper over hardcoded ABIs when running
 * long-lived agents that might outlive a contract upgrade.
 *
 * @example
 * ```ts
 * const { abi } = await fetchContractAbi('market-core');
 * const count = await publicClient.readContract({
 *   address: MARKET_CORE,
 *   abi,
 *   functionName: 'marketCount',
 * });
 * ```
 */
export async function fetchContractAbi(
  name: ContractAbiName,
  options: { baseUrl?: string; fetchImpl?: typeof fetch } = {},
): Promise<AbiResponse> {
  const baseUrl = (
    options.baseUrl ||
    (typeof process !== 'undefined' && process.env && process.env.RELAY44_CONTRACTS_URL) ||
    DEFAULT_CONTRACTS_BASE_URL
  ).replace(/\/+$/, '');

  const fetchImpl =
    options.fetchImpl ||
    (typeof fetch === 'function' ? fetch : undefined);
  if (!fetchImpl) {
    throw new Error(
      'fetchContractAbi: global fetch is not available. Pass `fetchImpl` explicitly.',
    );
  }

  const url = `${baseUrl}/api/contracts/${name}/abi`;
  const response = await fetchImpl(url, {
    headers: { accept: 'application/json' },
  });

  if (!response.ok) {
    throw new Error(
      `fetchContractAbi(${name}) failed: ${response.status} ${response.statusText} (${url})`,
    );
  }

  const payload = (await response.json()) as AbiResponse;
  if (!payload || !Array.isArray(payload.abi)) {
    throw new Error(
      `fetchContractAbi(${name}) returned an unexpected payload shape`,
    );
  }

  return payload;
}
