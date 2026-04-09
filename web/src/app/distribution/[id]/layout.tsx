import type { Metadata } from 'next';
import { StructuredData } from '@/components/seo/StructuredData';
import {
  buildBreadcrumbStructuredData,
  buildDistributionMarketDescription,
  buildDistributionMarketStructuredData,
  buildPageMetadata,
  SITE_NAME,
} from '@/lib/seo';
import { fetchSeoDistributionMarket } from '@/lib/server/seo';

interface DistributionLayoutProps {
  children: React.ReactNode;
  params: Promise<{ id: string }>;
}

export async function generateMetadata({
  params,
}: Omit<DistributionLayoutProps, 'children'>): Promise<Metadata> {
  const { id } = await params;
  const marketId = decodeURIComponent(id);
  const market = await fetchSeoDistributionMarket(marketId);

  if (!market) {
    return buildPageMetadata({
      title: `Distribution Market — ${marketId}`,
      description: `Trade on continuous probability distributions on ${SITE_NAME}. Predict where the outcome will land by setting your mean and standard deviation.`,
      path: `/distribution/${encodeURIComponent(marketId)}`,
      keywords: ['distribution', 'prediction market', 'continuous', 'probability'],
      openGraphType: 'article',
    });
  }

  const description = buildDistributionMarketDescription(market);

  return buildPageMetadata({
    title: market.question,
    description,
    path: `/distribution/${encodeURIComponent(market.id)}`,
    keywords: [
      market.category,
      'distribution',
      'prediction market',
      'continuous',
      'probability',
    ].filter(Boolean) as string[],
    openGraphType: 'article',
  });
}

export default async function DistributionLayout({
  children,
  params,
}: DistributionLayoutProps) {
  const { id } = await params;
  const marketId = decodeURIComponent(id);
  const market = await fetchSeoDistributionMarket(marketId);

  const breadcrumbs = buildBreadcrumbStructuredData([
    { name: 'Home', url: '/' },
    { name: 'Markets', url: '/markets' },
    { name: 'Distribution', url: '/distribution' },
    {
      name: market?.question || marketId,
      url: `/distribution/${encodeURIComponent(marketId)}`,
    },
  ]);

  return (
    <>
      <StructuredData
        data={
          market
            ? [breadcrumbs, buildDistributionMarketStructuredData(market)]
            : [breadcrumbs]
        }
      />
      {children}
    </>
  );
}
