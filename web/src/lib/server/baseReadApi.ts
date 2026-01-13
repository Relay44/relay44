import { randomBytes } from 'node:crypto';
import {
  createPublicClient,
  http,
  isAddress,
  type Address,
  type Hex,
} from 'viem';
import { base, baseSepolia } from 'viem/chains';
import {
  MARKET_CORE_ABI,
  ORDER_BOOK_ABI,
  MARKET_CORE_ADDRESS,
  ORDER_BOOK_ADDRESS,
} from '@/lib/contracts';

const ORDER_FILLED_EVENT_ABI = [
  {
    type: 'event',
    name: 'OrderFilled',
    inputs: [
      { indexed: true, name: 'orderId', type: 'uint256' },
      { indexed: false, name: 'fillSize', type: 'uint128' },
      { indexed: false, name: 'remaining', type: 'uint128' },
      { indexed: true, name: 'matcher', type: 'address' },
    ],
  },
] as const;

const ERC20_STATE_ABI = [
  {
    type: 'function',
    name: 'totalSupply',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'decimals',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'uint8' }],
  },
] as const;

const MAX_MARKETS_PAGE_SIZE = 200;
const MAX_ORDERBOOK_DEPTH = 100;
const MAX_TRADES_PAGE_SIZE = 200;
const ORDERBOOK_SCAN_WINDOW = BigInt(150);
const TRADES_BLOCK_SCAN_WINDOW = BigInt(25_000);
const PRICE_SCALE = 10_000;

export class BaseApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = 'BaseApiError';
    this.status = status;
    this.code = code;
  }
}

type BaseMarketTuple = readonly [Hex, bigint, bigint, Address, boolean, boolean];
type BaseOrderTuple = readonly [Address, bigint, boolean, bigint, bigint, bigint, bigint, boolean];

interface BaseConfig {
  chainId: number;
  rpcUrl: string;
  marketCore: Address;
  orderBook: Address;
}

interface OrderSnapshot {
  marketId: number;
  isYes: boolean;
  priceBps: number;
  remaining: number;
  expiry: number;
  canceled: boolean;
}

interface TradeSnapshot {
  id: string;
  market_id: string;
  outcome: 'yes' | 'no';
  price: number;
  price_bps: number;
  quantity: number;
  tx_hash: string;
  block_number: number;
  created_at: string;
}

function nowUnix(): number {
  return Math.floor(Date.now() / 1000);
}

function toIso(timestampSeconds: bigint): string {
  const millis = Number(timestampSeconds) * 1000;
  if (!Number.isFinite(millis) || millis <= 0) {
    return new Date().toISOString();
  }
  return new Date(millis).toISOString();
}

function parseIntegerQuery(raw: string | null | undefined, fallback: number): number {
  if (!raw) return fallback;
  const parsed = Number(raw);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new BaseApiError(400, 'INVALID_QUERY_PARAM', 'Query parameter must be a positive integer');
  }
  return parsed;
}

function parseMarketId(raw: string): number {
  const parsed = Number(raw);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new BaseApiError(400, 'INVALID_MARKET_ID', 'market_id must be a positive integer');
  }
  return parsed;
}

function parseOutcome(raw: string | null, allowAll = false): 'yes' | 'no' | null {
  if (!raw && allowAll) return null;
  const value = raw ?? 'yes';
  if (value !== 'yes' && value !== 'no') {
    throw new BaseApiError(400, 'INVALID_OUTCOME', "outcome must be either 'yes' or 'no'");
  }
  return value;
}

function parseAddress(name: string, raw: string): Address {
  if (!raw || !isAddress(raw)) {
    throw new BaseApiError(400, `INVALID_${name}`, `${name} must be configured as a valid Base address`);
  }
  return raw as Address;
}

function parseChainId(): number {
  const raw = process.env.NEXT_PUBLIC_BASE_CHAIN_ID || process.env.BASE_CHAIN_ID || '8453';
  const chainId = Number(raw);
  if (!Number.isInteger(chainId) || chainId <= 0) {
    throw new BaseApiError(500, 'INVALID_BASE_CHAIN_ID', 'BASE chain ID is invalid');
  }
  return chainId;
}

function resolveRpcUrl(chainId: number): string {
  const configured = process.env.NEXT_PUBLIC_BASE_RPC_URL || process.env.BASE_RPC_URL || '';
  if (configured) return configured;
  return chainId === 8453 ? 'https://mainnet.base.org' : 'https://sepolia.base.org';
}

function getBaseConfig(): BaseConfig {
  const chainId = parseChainId();
  const rpcUrl = resolveRpcUrl(chainId);

  const marketCoreRaw = MARKET_CORE_ADDRESS || process.env.MARKET_CORE_ADDRESS || '';
  const orderBookRaw = ORDER_BOOK_ADDRESS || process.env.ORDER_BOOK_ADDRESS || '';

  return {
    chainId,
    rpcUrl,
    marketCore: parseAddress('MARKET_CORE_ADDRESS', marketCoreRaw),
    orderBook: parseAddress('ORDER_BOOK_ADDRESS', orderBookRaw),
  };
}

function buildClient(config: BaseConfig) {
  const chain = config.chainId === 8453 ? base : baseSepolia;
  return createPublicClient({
    chain,
    transport: http(config.rpcUrl, { timeout: 15_000 }),
  });
}

function formatMarketStatus(closeTime: bigint, resolved: boolean): string {
  if (resolved) return 'resolved';
  return Number(closeTime) <= nowUnix() ? 'closed' : 'active';
}

function asNumber(value: bigint): number {
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    return Number.MAX_SAFE_INTEGER;
  }
  return Number(value);
}

async function fetchOrder(
  client: ReturnType<typeof buildClient>,
  orderBook: Address,
  orderId: bigint
): Promise<OrderSnapshot | null> {
  const tuple = (await client.readContract({
    address: orderBook,
    abi: ORDER_BOOK_ABI,
    functionName: 'orders',
    args: [orderId],
  })) as BaseOrderTuple;

  const [maker, marketId, isYes, priceBps, _size, remaining, expiry, canceled] = tuple;
  if (maker === '0x0000000000000000000000000000000000000000') {
    return null;
  }

  return {
    marketId: asNumber(marketId),
    isYes,
    priceBps: asNumber(priceBps),
    remaining: asNumber(remaining),
    expiry: asNumber(expiry),
    canceled,
  };
}

export function generateSiweNonce(): string {
  return randomBytes(16).toString('hex');
}

export async function readHealth() {
  return {
    status: 'healthy',
    service: 'relay44-web',
    timestamp: new Date().toISOString(),
  };
}

export async function readDetailedHealth() {
  try {
    const config = getBaseConfig();
    const client = buildClient(config);
    const marketCount = (await client.readContract({
      address: config.marketCore,
      abi: MARKET_CORE_ABI,
      functionName: 'marketCount',
    })) as bigint;

    return {
      status: 'healthy',
      service: 'relay44-web',
      timestamp: new Date().toISOString(),
      checks: {
        runtime: {
          status: 'healthy',
          mode: 'web-only',
          service: 'relay44-web',
        },
        base: {
          status: 'healthy',
          chain_id: config.chainId,
          rpc_url: config.rpcUrl,
          market_core: config.marketCore,
          order_book: config.orderBook,
          market_count: asNumber(marketCount),
        },
      },
    };
  } catch (error) {
    return {
      status: 'degraded',
      timestamp: new Date().toISOString(),
      checks: {
        base: {
          status: 'unhealthy',
          error: error instanceof Error ? error.message : 'Unknown Base RPC error',
        },
      },
    };
  }
}

export async function readBaseMarkets(searchParams: URLSearchParams) {
  const config = getBaseConfig();
  const client = buildClient(config);

  const limit = Math.min(parseIntegerQuery(searchParams.get('limit'), 50), MAX_MARKETS_PAGE_SIZE);
  const offset = parseIntegerQuery(searchParams.get('offset'), 0);

  const totalBigInt = (await client.readContract({
    address: config.marketCore,
    abi: MARKET_CORE_ABI,
    functionName: 'marketCount',
  })) as bigint;
  const total = asNumber(totalBigInt);

  if (total === 0 || offset >= total) {
    return {
      markets: [],
      total,
      limit,
      offset,
      source: 'market_core',
    };
  }

  const end = Math.min(total, offset + limit);
  const markets = [];

  for (let index = offset + 1; index <= end; index += 1) {
    const tuple = (await client.readContract({
      address: config.marketCore,
      abi: MARKET_CORE_ABI,
      functionName: 'markets',
      args: [BigInt(index)],
    })) as BaseMarketTuple;

    const [questionHash, closeTime, resolveTime, resolver, resolved, outcome] = tuple;
    markets.push({
      id: String(index),
      question_hash: questionHash,
      resolver: resolver.toLowerCase(),
      close_time: asNumber(closeTime),
      resolve_time: asNumber(resolveTime),
      resolved,
      outcome: resolved ? (outcome ? 'yes' : 'no') : null,
      status: formatMarketStatus(closeTime, resolved),
    });
  }

  return {
    markets,
    total,
    limit,
    offset,
    source: 'market_core',
  };
}

export async function readBaseOrderbook(marketIdRaw: string, searchParams: URLSearchParams) {
  const marketId = parseMarketId(marketIdRaw);
  const outcome = parseOutcome(searchParams.get('outcome')) as 'yes' | 'no';
  const depth = Math.min(parseIntegerQuery(searchParams.get('depth'), 20), MAX_ORDERBOOK_DEPTH);

  const config = getBaseConfig();
  const client = buildClient(config);

  const totalOrders = (await client.readContract({
    address: config.orderBook,
    abi: ORDER_BOOK_ABI,
    functionName: 'orderCount',
  })) as bigint;

  if (totalOrders === BigInt(0)) {
    return {
      market_id: marketIdRaw,
      outcome,
      bids: [],
      asks: [],
      last_updated: new Date().toISOString(),
      source: 'order_book_contract',
    };
  }

  const startOrderId =
    totalOrders > ORDERBOOK_SCAN_WINDOW
      ? totalOrders - ORDERBOOK_SCAN_WINDOW + BigInt(1)
      : BigInt(1);
  const now = nowUnix();
  const outcomeIsYes = outcome === 'yes';

  const bids = new Map<number, { quantity: number; orders: number }>();
  const asks = new Map<number, { quantity: number; orders: number }>();

  let orderId = totalOrders;
  while (orderId >= startOrderId) {
    const order = await fetchOrder(client, config.orderBook, orderId);

    if (
      order &&
      order.marketId === marketId &&
      !order.canceled &&
      order.remaining > 0 &&
      order.expiry >= now &&
      order.priceBps > 0 &&
      order.priceBps < PRICE_SCALE
    ) {
      if (order.isYes === outcomeIsYes) {
        const level = bids.get(order.priceBps) ?? { quantity: 0, orders: 0 };
        level.quantity += order.remaining;
        level.orders += 1;
        bids.set(order.priceBps, level);
      } else {
        const askPrice = PRICE_SCALE - order.priceBps;
        if (askPrice > 0 && askPrice < PRICE_SCALE) {
          const level = asks.get(askPrice) ?? { quantity: 0, orders: 0 };
          level.quantity += order.remaining;
          level.orders += 1;
          asks.set(askPrice, level);
        }
      }
    }

    if (orderId === BigInt(1)) {
      break;
    }
    orderId -= BigInt(1);
  }

  const bidLevels = Array.from(bids.entries())
    .sort((a, b) => b[0] - a[0])
    .slice(0, depth)
    .map(([priceBps, level]) => ({
      price: priceBps / PRICE_SCALE,
      quantity: level.quantity,
      orders: level.orders,
    }));

  const askLevels = Array.from(asks.entries())
    .sort((a, b) => a[0] - b[0])
    .slice(0, depth)
    .map(([priceBps, level]) => ({
      price: priceBps / PRICE_SCALE,
      quantity: level.quantity,
      orders: level.orders,
    }));

  return {
    market_id: marketIdRaw,
    outcome,
    bids: bidLevels,
    asks: askLevels,
    last_updated: new Date().toISOString(),
    source: 'order_book_contract',
  };
}

export async function readBaseTrades(marketIdRaw: string, searchParams: URLSearchParams) {
  const marketId = parseMarketId(marketIdRaw);
  const outcomeFilter = parseOutcome(searchParams.get('outcome'), true);
  const limit = Math.min(parseIntegerQuery(searchParams.get('limit'), 50), MAX_TRADES_PAGE_SIZE);
  const offset = parseIntegerQuery(searchParams.get('offset'), 0);

  const config = getBaseConfig();
  const client = buildClient(config);

  const latestBlock = await client.getBlockNumber();
  const fromBlock =
    latestBlock > TRADES_BLOCK_SCAN_WINDOW ? latestBlock - TRADES_BLOCK_SCAN_WINDOW : BigInt(0);

  const logs = await client.getLogs({
    address: config.orderBook,
    event: ORDER_FILLED_EVENT_ABI[0],
    fromBlock,
    toBlock: latestBlock,
    strict: false,
  });

  const blockTimestampCache = new Map<string, bigint>();
  const orderCache = new Map<string, OrderSnapshot | null>();
  const trades: TradeSnapshot[] = [];

  for (const log of logs) {
    const orderIdBigInt = log.args.orderId;
    const fillSizeBigInt = log.args.fillSize;

    if (orderIdBigInt === undefined || fillSizeBigInt === undefined) {
      continue;
    }

    const fillSize = asNumber(fillSizeBigInt);
    if (fillSize <= 0) {
      continue;
    }

    const orderKey = orderIdBigInt.toString();
    let order = orderCache.get(orderKey);
    if (order === undefined) {
      order = await fetchOrder(client, config.orderBook, orderIdBigInt);
      orderCache.set(orderKey, order);
    }
    if (!order || order.marketId !== marketId || order.priceBps <= 0 || order.priceBps >= PRICE_SCALE) {
      continue;
    }

    const outcome = order.isYes ? 'yes' : 'no';
    if (outcomeFilter && outcome !== outcomeFilter) {
      continue;
    }
