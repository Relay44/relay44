import {
  mapBaseSnapshotToMarket,
  normalizeBaseMarketsResponse,
  type BaseMarketSnapshot,
  type BaseMarketsResponse,
} from '@/lib/api';
import { readUnifiedMarket, readUnifiedMarkets } from '@/lib/server/unifiedMarketsApi';
import type { Market, MarketSource, PaginatedResponse, TradableFilter } from '@/types';

const DEFAULT_API_BASE = 'http://localhost:8080/v1';
const REQUEST_TIMEOUT_MS = 4_000;

interface FetchBaseMarketsOptions {
  limit?: number;
  offset?: number;
  source?: MarketSource;
  tradable?: TradableFilter;
  revalidateSeconds?: number;
}

function getApiBases(): string[] {
  const primary = process.env.NEXT_PUBLIC_API_URL?.trim() || DEFAULT_API_BASE;
  const fallback = process.env.NEXT_PUBLIC_API_FALLBACK_URL?.trim() || '';
  return [...new Set([primary, fallback].filter(Boolean))];
}

async function fetchJsonFromBase<T>(
  base: string,
  path: string,
  revalidateSeconds: number
): Promise<T | null> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);

  try {
    const response = await fetch(`${base}${path}`, {
      method: 'GET',
      signal: controller.signal,
      next: { revalidate: revalidateSeconds },
    });

    if (!response.ok) {
      return null;
    }

    return (await response.json()) as T;
  } catch {
    return null;
  } finally {
    clearTimeout(timeout);
  }
}

export async function fetchLiveBaseMarkets(
  options: FetchBaseMarketsOptions = {}
): Promise<PaginatedResponse<Market> | null> {
  const query = new URLSearchParams({
    limit: String(options.limit ?? 50),
    offset: String(options.offset ?? 0),
    source: options.source ?? 'all',
    tradable: options.tradable ?? 'all',
  });
  const path = `/evm/markets?${query.toString()}`;
  const revalidateSeconds = options.revalidateSeconds ?? 5;

  for (const base of getApiBases()) {
    const payload = await fetchJsonFromBase<BaseMarketsResponse>(
      base,
      path,
      revalidateSeconds
    );

    if (payload && Array.isArray(payload.markets)) {
      return normalizeBaseMarketsResponse(payload);
    }
  }

  try {
    return normalizeBaseMarketsResponse(await readUnifiedMarkets(query));
  } catch {
    return null;
  }
}

export async function fetchLiveBaseMarket(
  id: string,
  revalidateSeconds = 300
): Promise<Market | null> {
  const path = `/evm/markets/${encodeURIComponent(id)}`;

  for (const base of getApiBases()) {
    const payload = await fetchJsonFromBase<BaseMarketSnapshot>(
      base,
      path,
      revalidateSeconds
    );

    if (payload && typeof payload.id === 'string') {
      return mapBaseSnapshotToMarket(payload);
    }
  }

  try {
    return mapBaseSnapshotToMarket(await readUnifiedMarket(id));
  } catch {
    return null;
  }
}
