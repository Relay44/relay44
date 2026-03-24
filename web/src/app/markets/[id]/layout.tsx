import type { Metadata } from 'next';
import { StructuredData } from '@/components/seo/StructuredData';
import {
  buildBreadcrumbStructuredData,
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

  return buildPageMetadata({
    title: market.question,
    description: market.description || `Track live pricing and market context for ${market.question}.`,
    path: `/markets/${encodeURIComponent(marketId)}`,
    image: market.imageUrl,
    keywords: [market.category, market.provider, market.source],
    openGraphType: 'article',
  });
}

export default async function MarketLayout({ children, params }: MarketLayoutProps) {
  const { id } = await params;
  const marketId = decodeURIComponent(id);
  const market = await fetchSeoMarket(marketId);

  return (
    <>
      {market ? (
        <StructuredData
          data={[
            buildBreadcrumbStructuredData([
              { name: 'Home', url: '/' },
              { name: 'Markets', url: '/markets' },
              { name: market.question, url: `/markets/${encodeURIComponent(marketId)}` },
            ]),
            buildMarketStructuredData(market),
          ]}
        />
      ) : null}
      {children}
    </>
  );
}
