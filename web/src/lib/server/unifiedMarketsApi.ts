import type { BaseMarketSnapshot } from '@/lib/api';
import {
  BaseApiError,
  readBaseMarket,
  readBaseMarkets,
  readBaseOrderbook,
  readBaseTrades,
} from '@/lib/server/baseReadApi';

type MarketSource = 'all' | 'internal' | 'limitless' | 'polymarket';
type TradableFilter = 'all' | 'user' | 'agent';
type ExternalProvider = 'limitless' | 'polymarket';

const MAX_MARKETS_PAGE_SIZE = 200;
const MAX_ORDERBOOK_DEPTH = 100;
const MAX_TRADES_PAGE_SIZE = 200;
const LIMITLESS_API_BASE =
  process.env.LIMITLESS_API_BASE?.trim() || 'https://api.limitless.exchange';
const POLYMARKET_GAMMA_API_BASE =
  process.env.POLYMARKET_GAMMA_API_BASE?.trim() || 'https://gamma-api.polymarket.com';
const POLYMARKET_CLOB_API_BASE =
  process.env.POLYMARKET_CLOB_API_BASE?.trim() || 'https://clob.polymarket.com';

interface ExternalMarketId {
  provider: ExternalProvider;
  value: string;
}

interface OrderBookLevel {
  price: number;
  quantity: number;
  orders: number;
}

function clampProbability(value: number): number {
  if (!Number.isFinite(value)) return 0.5;
  return Math.max(0, Math.min(1, value));
}

function parseNumber(value: unknown): number {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return 0;
}

function parseString(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function parseStringList(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.filter((entry): entry is string => typeof entry === 'string');
  }

  if (typeof value === 'string') {
    try {
      const parsed = JSON.parse(value);
      if (Array.isArray(parsed)) {
        return parsed.filter((entry): entry is string => typeof entry === 'string');
      }
    } catch {
      return [];
    }
  }

  return [];
}

function nowIso(): string {
  return new Date().toISOString();
}

function parseIntegerQuery(raw: string | null | undefined, fallback: number): number {
  if (!raw) return fallback;
  const parsed = Number(raw);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new BaseApiError(400, 'INVALID_QUERY_PARAM', 'Query parameter must be a positive integer');
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

function parseSource(raw: string | null): MarketSource {
  if (!raw || raw === 'all') return 'all';
  if (raw === 'internal' || raw === 'limitless' || raw === 'polymarket') {
    return raw;
  }
  throw new BaseApiError(
    400,
    'INVALID_MARKET_SOURCE',
    'source must be one of: all, internal, limitless, polymarket'
  );
}

function parseTradable(raw: string | null): TradableFilter {
  if (!raw || raw === 'all') return 'all';
  if (raw === 'user' || raw === 'agent') return raw;
  throw new BaseApiError(
    400,
    'INVALID_TRADABLE_FILTER',
    'tradable must be one of: all, user, agent'
  );
}

function parseExternalMarketId(raw: string): ExternalMarketId | null {
  const [provider, value] = raw.split(':');
  if (!provider || !value) return null;
  if (provider !== 'limitless' && provider !== 'polymarket') {
    throw new BaseApiError(
      400,
      'INVALID_MARKET_SOURCE',
      'market source must be one of: limitless, polymarket'
    );
  }
  return { provider, value };
}

function includeMarket(
  tradable: TradableFilter,
  market: Pick<BaseMarketSnapshot, 'execution_users' | 'execution_agents'>
) {
  if (tradable === 'user') return market.execution_users ?? true;
  if (tradable === 'agent') return market.execution_agents ?? true;
  return true;
}

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url, {
    method: 'GET',
    next: { revalidate: 15 },
  });

  if (!response.ok) {
    throw new Error(`${response.status} ${response.statusText}`);
  }

  return response.json() as Promise<T>;
}

function buildLimitlessMarket(entry: Record<string, unknown>): BaseMarketSnapshot | null {
  const slug = parseString(entry.slug);
  if (!slug) return null;

  const prices = Array.isArray(entry.prices) ? entry.prices : [];
  const outcomeIndex = entry.winningOutcomeIndex;
  const outcomeValue =
    outcomeIndex === 0 || outcomeIndex === '0'
      ? 'yes'
      : outcomeIndex === 1 || outcomeIndex === '1'
        ? 'no'
        : null;
  const resolved = parseString(entry.status).toLowerCase() === 'resolved' || outcomeValue !== null;

  let yesPrice = clampProbability(parseNumber(prices[0]));
  let noPrice = clampProbability(parseNumber(prices[1]));
  if (!(yesPrice > 0 || noPrice > 0)) {
    yesPrice = 0.5;
    noPrice = 0.5;
  }

  if (resolved && outcomeValue === 'yes') {
    yesPrice = 1;
    noPrice = 0;
  } else if (resolved && outcomeValue === 'no') {
    yesPrice = 0;
    noPrice = 1;
  }

  const outcomes = [
    { label: 'Yes', probability: yesPrice },
    { label: 'No', probability: noPrice },
  ];
  const executable = true;

  return {
    id: `limitless:${slug}`,
    question_hash: parseString(entry.id) || slug,
    question: parseString(entry.title) || parseString(entry.proxyTitle) || slug.replace(/[-_]+/g, ' '),
    description:
      parseString(entry.description)
      || `Binary prediction market on Limitless for "${parseString(entry.title) || slug.replace(/[-_]+/g, ' ')}".`,
    category:
      Array.isArray(entry.categories) && typeof entry.categories[0] === 'string'
        ? String(entry.categories[0]).toLowerCase()
        : 'external',
    resolution_source: `https://limitless.exchange/markets/${slug}`,
    resolver: '',
    close_time: Math.floor(parseNumber(entry.expirationTimestamp) / 1000),
    resolve_time: Math.floor(parseNumber(entry.expirationTimestamp) / 1000),
    resolved,
    outcome: outcomeValue,
    status: resolved ? 'resolved' : 'active',
    source: 'external_limitless',
    provider: 'limitless',
    is_external: true,
    external_url: `https://limitless.exchange/markets/${slug}`,
    chain_id: 8453,
    requires_credentials: true,
    execution_users: executable,
    execution_agents: executable,
    outcomes,
    yes_price: yesPrice,
    no_price: noPrice,
    volume: parseNumber(entry.volume),
    provider_market_ref: parseString(entry.id),
  };
}

function buildPolymarketMarket(entry: Record<string, unknown>): BaseMarketSnapshot | null {
  const id = parseString(entry.id);
  const slug = parseString(entry.slug);
  if (!id || !slug) return null;

  const labels = parseStringList(entry.outcomes);
  const prices = parseStringList(entry.outcomePrices).map((value) => clampProbability(parseNumber(value)));
  const outcomes = (labels.length > 0 ? labels : ['Yes', 'No']).map((label, index) => ({
    label,
    probability: prices[index] ?? 0.5,
  }));
  const yesPrice =
    outcomes.find((entry) => entry.label.toLowerCase() === 'yes')?.probability ?? 0.5;
  const noPrice =
    outcomes.find((entry) => entry.label.toLowerCase() === 'no')?.probability ?? (1 - yesPrice);
  const active = Boolean(entry.active);
  const closed = Boolean(entry.closed);
  const resolved = Boolean(entry.resolved) || closed;
  const enableOrderBook = Boolean(entry.enableOrderBook);

  return {
    id: `polymarket:${id}`,
    question_hash: id,
    question: parseString(entry.question),
    description: parseString(entry.description),
    category: parseString(entry.category).toLowerCase() || 'external',
    resolution_source: `https://polymarket.com/event/${slug}`,
    resolver: '',
    close_time: Math.floor(new Date(parseString(entry.endDate)).getTime() / 1000),
    resolve_time: Math.floor(new Date(parseString(entry.endDate)).getTime() / 1000),
    resolved,
    outcome: null,
    status: resolved ? 'resolved' : active ? 'active' : 'closed',
    source: 'external_polymarket',
    provider: 'polymarket',
    is_external: true,
    external_url: `https://polymarket.com/event/${slug}`,
    chain_id: 137,
    requires_credentials: true,
    execution_users: enableOrderBook,
    execution_agents: enableOrderBook,
    outcomes,
    yes_price: yesPrice,
    no_price: noPrice,
    volume: parseNumber(entry.volume),
    provider_market_ref: id,
  };
}

async function fetchLimitlessMarkets(limit: number, offset: number): Promise<BaseMarketSnapshot[]> {
  const pageSize = Math.max(1, Math.min(limit, 25));
  let page = Math.floor(offset / pageSize) + 1;
  let skipped = offset % pageSize;
  const markets: BaseMarketSnapshot[] = [];

  while (markets.length < limit) {
    const payload = await fetchJson<{ data?: Record<string, unknown>[] }>(
      `${LIMITLESS_API_BASE.replace(/\/$/, '')}/markets/active?limit=${pageSize}&page=${page}`
    );
    const rows = payload.data ?? [];
    if (rows.length === 0) break;

    let addedThisPage = 0;
    for (const row of rows) {
      if (skipped > 0) {
        skipped -= 1;
        continue;
      }
      const market = buildLimitlessMarket(row);
      if (!market) continue;
      markets.push(market);
      addedThisPage += 1;
      if (markets.length >= limit) break;
    }

    if (rows.length < pageSize || addedThisPage === 0) break;
    page += 1;
  }

  return markets;
}

async function fetchPolymarketMarkets(limit: number, offset: number): Promise<BaseMarketSnapshot[]> {
  const rows = await fetchJson<Record<string, unknown>[]>(
    `${POLYMARKET_GAMMA_API_BASE.replace(/\/$/, '')}/markets?limit=${limit}&offset=${offset}&active=true&closed=false`
  );

  return rows
    .map(buildPolymarketMarket)
    .filter((market): market is BaseMarketSnapshot => Boolean(market));
}

async function fetchLimitlessMarket(slug: string): Promise<BaseMarketSnapshot> {
  const market = buildLimitlessMarket(
    await fetchJson<Record<string, unknown>>(
      `${LIMITLESS_API_BASE.replace(/\/$/, '')}/markets/${slug.trim()}`
    )
  );

  if (!market) {
    throw new BaseApiError(404, 'MARKET_NOT_FOUND', 'failed to parse Limitless market payload');
  }

  return market;
}

async function fetchPolymarketMarket(id: string): Promise<BaseMarketSnapshot> {
  const market = buildPolymarketMarket(
    await fetchJson<Record<string, unknown>>(
      `${POLYMARKET_GAMMA_API_BASE.replace(/\/$/, '')}/markets/${id.trim()}`
    )
  );

  if (!market) {
    throw new BaseApiError(404, 'MARKET_NOT_FOUND', 'failed to parse Polymarket market payload');
  }

  return market;
}

function parseOrderBookLevels(values: unknown): OrderBookLevel[] {
  if (!Array.isArray(values)) return [];

  return values
    .map((row) => {
      const entry = row as Record<string, unknown>;
      const price = clampProbability(parseNumber(entry.price));
      const quantity = Math.max(0, parseNumber(entry.size));
      if (price <= 0 || quantity <= 0) return null;
      return {
        price,
        quantity,
        orders: Math.max(1, Math.round(parseNumber(entry.count) || 1)),
      };
    })
    .filter((entry): entry is OrderBookLevel => Boolean(entry));
}

async function fetchLimitlessOrderbook(slug: string, outcome: 'yes' | 'no', depth: number) {
  const url = `${LIMITLESS_API_BASE.replace(/\/$/, '')}/markets/${slug.trim()}/orderbook`;
  const response = await fetch(url, { method: 'GET', next: { revalidate: 15 } });

  if (!response.ok) {
    const payload = await response.json().catch(() => ({}));
    const message = parseString((payload as Record<string, unknown>).message).toLowerCase();
    if (response.status === 400 && (message.includes('does not support orderbook') || message.includes('amm market'))) {
      return {
        market_id: `limitless:${slug}`,
        outcome,
        bids: [],
        asks: [],
        last_updated: nowIso(),
        provider: 'limitless',
        chain_id: 8453,
        provider_market_ref: '',
        is_synthetic: false,
      };
    }

    throw new BaseApiError(502, 'EXTERNAL_ORDERBOOK_FAILED', `limitless orderbook request failed: ${response.status}`);
  }

  const payload = (await response.json()) as Record<string, unknown>;
  return {
    market_id: `limitless:${slug}`,
    outcome,
    bids: parseOrderBookLevels(payload.bids).slice(0, depth),
    asks: parseOrderBookLevels(payload.asks).slice(0, depth),
    last_updated: nowIso(),
    provider: 'limitless',
    chain_id: 8453,
    provider_market_ref: parseString(payload.tokenId),
    is_synthetic: false,
  };
}

async function fetchLimitlessTrades(
  slug: string,
  outcomeFilter: 'yes' | 'no' | null,
  limit: number,
  offset: number
) {
  const safeLimit = Math.max(1, Math.min(limit, 200));
  const pageNumber = Math.floor(offset / safeLimit) + 1;
  const localOffset = offset % safeLimit;
  const payload = await fetchJson<{ events?: Record<string, unknown>[] }>(
    `${LIMITLESS_API_BASE.replace(/\/$/, '')}/markets/${slug.trim()}/events?limit=${safeLimit}&page=${pageNumber}`
  );

  const trades = (payload.events ?? [])
    .map((event, index) => {
      const side = parseNumber(event.side);
      const outcome = side === 1 ? 'no' : 'yes';
      if (outcomeFilter && outcome !== outcomeFilter) return null;
      const price = clampProbability(parseNumber(event.price));
      const quantity = Math.max(0, Math.round(parseNumber(event.matchedSize)));
      const id = parseString(event.id) || `${slug}:${index}`;

      return {
        id: `limitless:${id}`,
        market_id: `limitless:${slug}`,
        outcome,
        price,
        price_bps: Math.round(price * 10_000),
        quantity,
        tx_hash: parseString(event.transactionHash),
        block_number: Math.round(parseNumber(event.blockNumber)),
        created_at: parseString(event.createdAt) || nowIso(),
      };
    })
    .filter((trade): trade is NonNullable<typeof trade> => Boolean(trade));

  const start = Math.min(localOffset, trades.length);
  const end = Math.min(start + safeLimit, trades.length);
  const pageTrades = trades.slice(start, end);

  return {
    trades: pageTrades,
    total: trades.length,
    limit: safeLimit,
    offset,
    has_more: end < trades.length,
    provider: 'limitless',
    chain_id: 8453,
    provider_market_ref: slug,
    is_synthetic: false,
  };
}

function tokenForOutcome(labels: string[], tokenIds: string[], outcome: 'yes' | 'no') {
  const normalizedTarget = outcome.toLowerCase();
  for (let index = 0; index < labels.length; index += 1) {
    if (labels[index]?.toLowerCase() === normalizedTarget) {
      return tokenIds[index] ?? null;
    }
  }
  return outcome === 'yes' ? tokenIds[0] ?? null : tokenIds[1] ?? null;
}

async function fetchPolymarketOrderbook(marketId: string, outcome: 'yes' | 'no', depth: number) {
  const market = await fetchJson<Record<string, unknown>>(
    `${POLYMARKET_GAMMA_API_BASE.replace(/\/$/, '')}/markets/${marketId.trim()}`
  );
  const labels = parseStringList(market.outcomes);
  const tokenIds = parseStringList(market.clobTokenIds);
  const tokenId = tokenForOutcome(labels, tokenIds, outcome);

  if (!tokenId) {
    throw new BaseApiError(404, 'POLYMARKET_TOKEN_NOT_FOUND', 'unable to map outcome to polymarket token id');
  }

  const payload = await fetchJson<Record<string, unknown>>(
    `${POLYMARKET_CLOB_API_BASE.replace(/\/$/, '')}/book?token_id=${encodeURIComponent(tokenId)}`
  );

  return {
    market_id: `polymarket:${marketId}`,
    outcome,
    bids: parseOrderBookLevels(payload.bids).slice(0, depth),
    asks: parseOrderBookLevels(payload.asks).slice(0, depth),
    last_updated: nowIso(),
    provider: 'polymarket',
    chain_id: 137,
    provider_market_ref: tokenId,
    is_synthetic: false,
  };
}

async function fetchPolymarketTrades(
  marketId: string,
  outcomeFilter: 'yes' | 'no' | null,
  limit: number,
  offset: number
) {
  const market = await fetchJson<Record<string, unknown>>(
    `${POLYMARKET_GAMMA_API_BASE.replace(/\/$/, '')}/markets/${marketId.trim()}`
  );
  const labels = parseStringList(market.outcomes);
  const tokenIds = parseStringList(market.clobTokenIds);
  const targets: Array<{ outcome: 'yes' | 'no'; tokenId: string }> = [];

  if (!outcomeFilter || outcomeFilter === 'yes') {
    const yesToken = tokenForOutcome(labels, tokenIds, 'yes');
    if (yesToken) targets.push({ outcome: 'yes', tokenId: yesToken });
  }
  if (!outcomeFilter || outcomeFilter === 'no') {
    const noToken = tokenForOutcome(labels, tokenIds, 'no');
    if (noToken) targets.push({ outcome: 'no', tokenId: noToken });
  }

  const trades = (
    await Promise.all(
      targets.map(async ({ outcome, tokenId }) => {
        const payload = await fetchJson<{ history?: Array<{ t?: number; p?: number | string }> }>(
          `${POLYMARKET_CLOB_API_BASE.replace(/\/$/, '')}/prices-history?market=${encodeURIComponent(tokenId)}&interval=1h&fidelity=60`
        );

        return (payload.history ?? []).map((item) => {
          const timestamp = parseNumber(item.t);
          const price = clampProbability(parseNumber(item.p));
          return {
            id: `polymarket:${marketId}:${outcome}:${timestamp}`,
            market_id: `polymarket:${marketId}`,
            outcome,
            price,
            price_bps: Math.round(price * 10_000),
            quantity: 0,
            tx_hash: '',
            block_number: 0,
            created_at: timestamp > 0 ? new Date(timestamp * 1000).toISOString() : nowIso(),
          };
        });
      })
    )
  ).flat();

  trades.sort((left, right) => right.created_at.localeCompare(left.created_at));
  const start = Math.min(offset, trades.length);
  const end = Math.min(start + limit, trades.length);

  return {
    trades: trades.slice(start, end),
    total: trades.length,
    limit,
    offset,
    has_more: end < trades.length,
    provider: 'polymarket',
    chain_id: 137,
    provider_market_ref: tokenIds[0] ?? marketId,
    is_synthetic: true,
  };
}

export async function readUnifiedMarkets(searchParams: URLSearchParams) {
  const source = parseSource(searchParams.get('source'));
  const tradable = parseTradable(searchParams.get('tradable'));
  const limit = Math.min(parseIntegerQuery(searchParams.get('limit'), 50), MAX_MARKETS_PAGE_SIZE);
  const offset = parseIntegerQuery(searchParams.get('offset'), 0);
  const fetchWindow = Math.min(limit + offset, MAX_MARKETS_PAGE_SIZE);

  const markets: BaseMarketSnapshot[] = [];

  if (source === 'all' || source === 'internal') {
    const internalParams = new URLSearchParams({
      limit: String(fetchWindow),
      offset: '0',
    });
    const internalResponse = await readBaseMarkets(internalParams);
    markets.push(...internalResponse.markets);
  }

  const externalFetches: Array<Promise<BaseMarketSnapshot[]>> = [];
  if (source === 'all' || source === 'limitless') {
    externalFetches.push(fetchLimitlessMarkets(fetchWindow, 0));
  }
  if (source === 'all' || source === 'polymarket') {
    externalFetches.push(fetchPolymarketMarkets(fetchWindow, 0));
  }

  const externalResults = await Promise.allSettled(externalFetches);
  for (const result of externalResults) {
    if (result.status === 'fulfilled') {
      markets.push(...result.value);
    } else if (source !== 'all') {
      throw new BaseApiError(502, 'EXTERNAL_MARKETS_FAILED', result.reason instanceof Error ? result.reason.message : 'failed to load external markets');
    }
  }

  const filtered = markets.filter((market) => includeMarket(tradable, market));
  if (source !== 'internal') {
    filtered.sort(
      (left, right) => right.close_time - left.close_time || left.id.localeCompare(right.id)
    );
  }

  const total = filtered.length;
  const page = total === 0 || offset >= total ? [] : filtered.slice(offset, offset + limit);

  return {
    markets: page,
    total,
    limit,
    offset,
    source,
  };
}

export async function readUnifiedMarket(marketIdRaw: string) {
  const externalId = parseExternalMarketId(marketIdRaw);
  if (!externalId) {
    return readBaseMarket(marketIdRaw);
  }

  if (externalId.provider === 'limitless') {
    return fetchLimitlessMarket(externalId.value);
  }

  return fetchPolymarketMarket(externalId.value);
}

export async function readUnifiedOrderbook(marketIdRaw: string, searchParams: URLSearchParams) {
  const externalId = parseExternalMarketId(marketIdRaw);
  if (!externalId) {
    return readBaseOrderbook(marketIdRaw, searchParams);
  }

  const outcome = parseOutcome(searchParams.get('outcome')) as 'yes' | 'no';
  const depth = Math.min(parseIntegerQuery(searchParams.get('depth'), 20), MAX_ORDERBOOK_DEPTH);

  if (externalId.provider === 'limitless') {
    return fetchLimitlessOrderbook(externalId.value, outcome, depth);
  }

  return fetchPolymarketOrderbook(externalId.value, outcome, depth);
}

export async function readUnifiedTrades(marketIdRaw: string, searchParams: URLSearchParams) {
  const externalId = parseExternalMarketId(marketIdRaw);
  if (!externalId) {
    return readBaseTrades(marketIdRaw, searchParams);
  }

  const outcome = parseOutcome(searchParams.get('outcome'), true);
  const limit = Math.min(parseIntegerQuery(searchParams.get('limit'), 50), MAX_TRADES_PAGE_SIZE);
  const offset = parseIntegerQuery(searchParams.get('offset'), 0);

  if (externalId.provider === 'limitless') {
    return fetchLimitlessTrades(externalId.value, outcome, limit, offset);
  }

  return fetchPolymarketTrades(externalId.value, outcome, limit, offset);
}
