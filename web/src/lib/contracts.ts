import { isAddress } from 'viem';

export const MARKET_CORE_ADDRESS = process.env.NEXT_PUBLIC_MARKET_CORE_ADDRESS || '';
export const ORDER_BOOK_ADDRESS = process.env.NEXT_PUBLIC_ORDER_BOOK_ADDRESS || '';
export const COLLATERAL_TOKEN_ADDRESS = process.env.NEXT_PUBLIC_COLLATERAL_TOKEN_ADDRESS || '';
export const COLLATERAL_VAULT_ADDRESS = process.env.NEXT_PUBLIC_COLLATERAL_VAULT_ADDRESS || '';

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

export function assertContractAddress(address: string, envName: string): `0x${string}` {
  if (!address || !isAddress(address)) {
    throw new Error(`${envName} is not configured as a valid Base address`);
  }
  return address as `0x${string}`;
}
