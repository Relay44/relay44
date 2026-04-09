import type { Metadata } from 'next';
import { StructuredData } from '@/components/seo/StructuredData';
import {
  buildBreadcrumbStructuredData,
  buildMarketDescription,
  buildMarketEventStructuredData,
  buildMarketStructuredData,
  buildPageMetadata,
} from '@/lib/seo';
import { fetchSeoMarket } from '@/lib/server/seo';

interface MarketLayoutProps {
  children: React.ReactNode;
  params: Promise<{ id: string }>;
}

export async function generateMetadata({ params }: Omit<MarketLayoutProps, 'children'>): Promise<Metadata> {
  const { id } = await params;
  const marketId = decodeURIComponent(id);
  const market = await fetchSeoMarket(marketId);

  if (!market) {
    return buildPageMetadata({
      title: 'Market not found',
      description: 'The requested market could not be found on relay44.',
      path: `/markets/${encodeURIComponent(marketId)}`,
      noIndex: true,
    });
  }

  const description = buildMarketDescription(market);

  return buildPageMetadata({
    title: market.question,
    description,
    path: `/markets/${encodeURIComponent(marketId)}`,
    image: market.imageUrl,
    keywords: [
      market.category,
      market.provider,
      market.source,
      'prediction market',
      'binary market',
    ].filter(Boolean),
    openGraphType: 'article',
  });
}

export default async function MarketLayout({ children, params }: MarketLayoutProps) {
  const { id } = await params;
  const marketId = decodeURIComponent(id);
  const market = await fetchSeoMarket(marketId);

  const breadcrumbs = buildBreadcrumbStructuredData([
    { name: 'Home', url: '/' },
    { name: 'Markets', url: '/markets' },
    { name: market?.question || marketId, url: `/markets/${encodeURIComponent(marketId)}` },
  ]);

  return (
    <>
      <StructuredData
        data={
          market
            ? [breadcrumbs, buildMarketStructuredData(market), buildMarketEventStructuredData(market)]
            : [breadcrumbs]
        }
      />
      {children}
    </>
  );
}
