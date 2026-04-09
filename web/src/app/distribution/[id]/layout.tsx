import type { Metadata } from 'next';
import { StructuredData } from '@/components/seo/StructuredData';
import { buildBreadcrumbStructuredData, buildPageMetadata } from '@/lib/seo';

interface DistributionLayoutProps {
  children: React.ReactNode;
  params: Promise<{ id: string }>;
}

export async function generateMetadata({
  params,
}: Omit<DistributionLayoutProps, 'children'>): Promise<Metadata> {
  const { id } = await params;
  const marketId = decodeURIComponent(id);

  return buildPageMetadata({
    title: `Distribution Market — ${marketId}`,
    description: `Trade on continuous probability distributions. Predict where the outcome will land by setting your mean and standard deviation.`,
    path: `/distribution/${encodeURIComponent(marketId)}`,
    keywords: ['distribution', 'prediction market', 'continuous', 'probability'],
    openGraphType: 'article',
  });
}

export default async function DistributionLayout({
  children,
  params,
}: DistributionLayoutProps) {
  const { id } = await params;
  const marketId = decodeURIComponent(id);

  return (
    <>
      <StructuredData
        data={[
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Markets', url: '/markets' },
            { name: 'Distribution', url: `/distribution/${encodeURIComponent(marketId)}` },
          ]),
        ]}
      />
      {children}
    </>
  );
}
