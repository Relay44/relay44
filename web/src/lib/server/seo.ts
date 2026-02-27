import {
  mapBaseSnapshotToMarket,
  normalizeBaseMarketsResponse,
  type BaseMarketSnapshot,
  type BaseMarketsResponse,
} from '@/lib/api';
import type { Leaderboard, LeaderboardEntry, Market, PublicProfile } from '@/types';

const DEFAULT_API_BASE = 'http://localhost:8080/v1';
const REQUEST_TIMEOUT_MS = 8_000;
const SEO_REVALIDATE_SECONDS = 300;

function getApiBases(): string[] {
  const primary = process.env.NEXT_PUBLIC_API_URL?.trim() || DEFAULT_API_BASE;
  const fallback = process.env.NEXT_PUBLIC_API_FALLBACK_URL?.trim() || '';
  return [...new Set([primary, fallback].filter(Boolean))];
}

async function fetchJsonFromBase<T>(base: string, path: string): Promise<T | null> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);

  try {
    const response = await fetch(`${base}${path}`, {
      method: 'GET',
      signal: controller.signal,
      next: { revalidate: SEO_REVALIDATE_SECONDS },
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

async function fetchJsonFromBases<T>(path: string): Promise<T | null> {
  for (const base of getApiBases()) {
    const payload = await fetchJsonFromBase<T>(base, path);
    if (payload) {
      return payload;
    }
  }

  return null;
}

export async function fetchSeoMarketsPage(
  limit = 100,
  offset = 0
): Promise<{ data: Market[]; hasMore: boolean; total: number; limit: number; offset: number } | null> {
  const query = new URLSearchParams({
    limit: String(limit),
    offset: String(offset),
    source: 'all',
    tradable: 'all',
  });
  const payload = await fetchJsonFromBases<BaseMarketsResponse>(`/evm/markets?${query.toString()}`);

  if (!payload || !Array.isArray(payload.markets)) {
    return null;
  }

  return normalizeBaseMarketsResponse(payload);
}

export async function fetchSeoMarkets(limit = 50): Promise<Market[]> {
  const page = await fetchSeoMarketsPage(limit, 0);
  return page?.data ?? [];
}

export async function fetchAllSeoMarkets(limit = 100, maxPages = 10): Promise<Market[]> {
  const markets: Market[] = [];
  let offset = 0;

  for (let pageIndex = 0; pageIndex < maxPages; pageIndex += 1) {
    const page = await fetchSeoMarketsPage(limit, offset);
    if (!page || page.data.length === 0) {
      break;
    }

    markets.push(...page.data);
    if (!page.hasMore) {
      break;
    }

    offset += page.limit || limit;
  }

  return markets;
}

export async function fetchSeoMarket(id: string): Promise<Market | null> {
  const payload = await fetchJsonFromBases<BaseMarketSnapshot>(
    `/evm/markets/${encodeURIComponent(id)}`
  );

  if (!payload || typeof payload.id !== 'string') {
    return null;
  }

  return mapBaseSnapshotToMarket(payload);
}

export async function fetchSeoLeaderboard(limit = 25): Promise<LeaderboardEntry[]> {
  const query = new URLSearchParams({
    period: 'weekly',
    metric: 'pnl',
    limit: String(limit),
  });
  const payload = await fetchJsonFromBases<Leaderboard>(`/leaderboard?${query.toString()}`);

  if (!payload || !Array.isArray(payload.entries)) {
    return [];
  }

  return payload.entries;
}

export async function fetchSeoProfile(wallet: string): Promise<PublicProfile | null> {
  const payload = await fetchJsonFromBases<PublicProfile>(`/profiles/${encodeURIComponent(wallet)}`);

  if (!payload || typeof payload.wallet !== 'string') {
    return null;
  }

  return payload;
}
