import HomePageClient from './HomePageClient';
import { StructuredData } from '@/components/seo/StructuredData';
import {
  normalizeBaseMarketsResponse,
  type BaseMarketsResponse,
} from '@/lib/api';
import {
  absoluteUrl,
  buildCollectionPageStructuredData,
  buildWebPageStructuredData,
} from '@/lib/seo';
import { getHomeLiveFeed } from '@/lib/server/homeLive';
import type { Market, PaginatedResponse } from '@/types';

export const revalidate = 5;

function getApiBases(): string[] {
  const primary = process.env.NEXT_PUBLIC_API_URL?.trim() || 'http://localhost:8080/v1';
  const fallback = process.env.NEXT_PUBLIC_API_FALLBACK_URL?.trim() || '';
  return [...new Set([primary, fallback].filter(Boolean))];
}

async function fetchMarketsFromBase(base: string): Promise<PaginatedResponse<Market> | null> {
  const query = new URLSearchParams({
    limit: '12',
    offset: '0',
    source: 'all',
    tradable: 'all',
  });
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 2500);
  try {
    const res = await fetch(`${base}/evm/markets?${query.toString()}`, {
      method: 'GET',
      next: { revalidate: 5 },
      signal: controller.signal,
    });
    if (!res.ok) return null;
    const payload = (await res.json()) as BaseMarketsResponse;
    if (!Array.isArray(payload.markets)) return null;
    return normalizeBaseMarketsResponse(payload);
  } catch {
    return null;
  } finally {
    clearTimeout(timeout);
  }
}

async function fetchInitialMarkets(): Promise<PaginatedResponse<Market> | null> {
  const bases = getApiBases();
  if (bases.length === 0) return null;

  const attempts = bases.map(async (base) => {
    const payload = await fetchMarketsFromBase(base);
    if (!payload || payload.data.length === 0) {
      throw new Error(`Empty markets payload from ${base}`);
    }
    return payload;
  });

  try {
    return await Promise.any(attempts);
  } catch {
    for (const base of bases) {
      const payload = await fetchMarketsFromBase(base);
      if (payload && payload.data.length > 0) {
        return payload;
      }
    }
    return null;
  }
}

export default async function HomePage() {
  const [initialMarkets, initialLiveFeed] = await Promise.all([
    fetchInitialMarkets(),
    getHomeLiveFeed(),
  ]);
  const marketItems = (initialMarkets?.data ?? []).map((market) => ({
    name: market.question,
    url: absoluteUrl(`/markets/${encodeURIComponent(market.id)}`),
  }));

  return (
    <>
      <StructuredData
        data={[
          buildWebPageStructuredData({
            path: '/',
            name: 'relay44',
            description:
              'Live agentic prediction markets, world signal monitoring, and machine-native market intelligence.',
          }),
          buildCollectionPageStructuredData({
            path: '/',
            name: 'Featured markets',
            description:
              'Live prediction markets, signal feeds, and world-desk market ideas on relay44.',
            items: marketItems,
          }),
        ]}
      />
      <HomePageClient
        initialMarkets={initialMarkets}
        initialLiveFeed={initialLiveFeed}
      />
    </>
  );
}
