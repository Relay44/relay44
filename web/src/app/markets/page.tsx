import type { Metadata } from 'next';
import MarketsClient from './MarketsClient';
import { StructuredData } from '@/components/seo/StructuredData';
import { fetchLiveBaseMarkets } from '@/lib/server/baseMarketData';
import {
  absoluteUrl,
  buildCollectionPageStructuredData,
  buildPageMetadata,
} from '@/lib/seo';

interface MarketsPageProps {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}

export const revalidate = 5;

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
    return 'Browse live prediction markets, market data, and pricing across Relay44 and connected venues.';
  }

  return `Browse ${category.toLowerCase()} prediction markets and live pricing on Relay44.`;
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

export default async function MarketsPage({ searchParams }: MarketsPageProps) {
  const params = searchParams ? await searchParams : {};
  const initialMarkets = await fetchLiveBaseMarkets({
    limit: 50,
    revalidateSeconds: 5,
  });
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
