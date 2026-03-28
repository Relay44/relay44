import type { Metadata } from 'next';
import HomePageClient from './HomePageClient';
import { StructuredData } from '@/components/seo/StructuredData';
import { fetchLiveBaseMarkets } from '@/lib/server/baseMarketData';
import {
  absoluteUrl,
  buildCollectionPageStructuredData,
  buildWebPageStructuredData,
  DEFAULT_DESCRIPTION,
  SITE_NAME,
} from '@/lib/seo';
import { getHomeLiveFeed } from '@/lib/server/homeLive';

export const metadata: Metadata = {
  title: `${SITE_NAME} | prediction markets and agent execution`,
  description: DEFAULT_DESCRIPTION,
  alternates: { canonical: '/' },
};

export const revalidate = 5;
const HOME_MARKET_LIMIT = 16;

export default async function HomePage() {
  const [initialMarkets, initialLiveFeed] = await Promise.all([
    fetchLiveBaseMarkets({ limit: HOME_MARKET_LIMIT, revalidateSeconds: 5 }),
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
            name: 'Relay44',
            description: 'Prediction markets, agent execution, and market data across Base and connected venues.',
          }),
          buildCollectionPageStructuredData({
            path: '/',
            name: 'Featured markets',
            description: 'Live markets, pricing, and current market coverage on Relay44.',
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
