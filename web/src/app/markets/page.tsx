import type { Metadata } from 'next';
import MarketsClient from './MarketsClient';
import { StructuredData } from '@/components/seo/StructuredData';
import {
  normalizeBaseMarketsResponse,
  type BaseMarketsResponse,
} from '@/lib/api';
import {
  absoluteUrl,
  buildCollectionPageStructuredData,
  buildPageMetadata,
} from '@/lib/seo';
import type { Market, PaginatedResponse } from '@/types';

interface MarketsPageProps {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}

export const revalidate = 5;

function getApiBases(): string[] {
  const primary = process.env.NEXT_PUBLIC_API_URL?.trim() || 'http://localhost:8080/v1';
  const fallback = process.env.NEXT_PUBLIC_API_FALLBACK_URL?.trim() || '';
  return [...new Set([primary, fallback].filter(Boolean))];
}

function normalizeCategory(input: string | string[] | undefined): string {
  const value = Array.isArray(input) ? input[0] : input;
  return value || 'All';
}

function normalizeQuery(input: string | string[] | undefined): string {
  const value = Array.isArray(input) ? input[0] : input;
  return value?.trim() || '';
}

function categoryDescription(category: string) {
  if (category === 'All') {
    return 'Browse live agentic prediction markets, market data, and outcome pricing across relay44 and connected venues.';
  }

  return `Browse ${category.toLowerCase()} prediction markets, live pricing, and market intelligence on relay44.`;
}

export async function generateMetadata({ searchParams }: MarketsPageProps): Promise<Metadata> {
  const params = searchParams ? await searchParams : {};
  const category = normalizeCategory(params.category);

  return buildPageMetadata({
    title: category === 'All' ? 'Markets' : `${category} markets`,
    description: categoryDescription(category),
    path: category === 'All' ? '/markets' : `/markets?category=${encodeURIComponent(category)}`,
    keywords: category === 'All' ? ['live markets'] : [category, `${category} prediction markets`],
  });
}

async function fetchMarketsFromBase(base: string): Promise<PaginatedResponse<Market> | null> {
  const query = new URLSearchParams({
    limit: '50',
    offset: '0',
    source: 'all',
    tradable: 'all',
  });
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 3000);
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

export default async function MarketsPage({ searchParams }: MarketsPageProps) {
  const params = searchParams ? await searchParams : {};
  const initialMarkets = await fetchInitialMarkets();
  const category = normalizeCategory(params.category);
  const searchQuery = normalizeQuery(params.q);
  const itemList = (initialMarkets?.data ?? []).map((market) => ({
    name: market.question,
    url: absoluteUrl(`/markets/${encodeURIComponent(market.id)}`),
  }));

  return (
    <>
      <StructuredData
        data={buildCollectionPageStructuredData({
          path: '/markets',
          name: category === 'All' ? 'Markets' : `${category} markets`,
          description: categoryDescription(category),
          items: itemList,
        })}
      />
      <MarketsClient
        initialCategory={category}
        initialMarkets={initialMarkets}
        initialSearchQuery={searchQuery}
      />
    </>
  );
}
