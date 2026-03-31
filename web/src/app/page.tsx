import { randomInt } from 'node:crypto';
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

export const dynamic = 'force-dynamic';
const HOME_MARKET_LIMIT = 100;
const HOME_HERO_IMAGE_SRCS = [
  '/home-hero-slides/643927642.jpg',
  '/home-hero-slides/65465146546.jpg',
  '/home-hero-slides/68880184-283f-4bad-9f22-62194696309f.jpg',
  '/home-hero-slides/b79d5c4f-4a29-4f88-87ab-8ad587370502.jpg',
  '/home-hero-slides/775554444.jpg',
  '/home-hero-slides/94845465454.jpg',
  '/home-hero-slides/884355.jpg',
  '/home-hero-slides/44445611654.jpg',
  '/home-hero-slides/92716739691.jpg',
] as const;

export default async function HomePage() {
  const [initialMarkets, initialLiveFeed] = await Promise.all([
    fetchLiveBaseMarkets({
      limit: HOME_MARKET_LIMIT,
      revalidateSeconds: 5,
      sort: 'volume',
    }),
    getHomeLiveFeed(),
  ]);
  const marketItems = (initialMarkets?.data ?? []).map((market) => ({
    name: market.question,
    url: absoluteUrl(`/markets/${encodeURIComponent(market.id)}`),
  }));
  const heroBackgroundImageSrc =
    HOME_HERO_IMAGE_SRCS[randomInt(0, HOME_HERO_IMAGE_SRCS.length)];

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
        heroBackgroundImageSrc={heroBackgroundImageSrc}
      />
    </>
  );
}
