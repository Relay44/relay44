import type { Metadata } from 'next';
import DistributionPageClient from './DistributionPageClient';
import { StructuredData } from '@/components/seo/StructuredData';
import {
  buildCollectionPageStructuredData,
  buildPageMetadata,
} from '@/lib/seo';

export const revalidate = 5;

export async function generateMetadata(): Promise<Metadata> {
  return buildPageMetadata({
    title: 'Distribution Markets',
    description:
      'Trade continuous outcome markets. Express beliefs as probability distributions — set a mean, choose your confidence, and let the market aggregate collective conviction.',
    path: '/distribution',
    keywords: ['distribution markets', 'continuous prediction markets', 'Gaussian', 'LMSR'],
  });
}

export default function DistributionPage() {
  return (
    <>
      <StructuredData
        data={buildCollectionPageStructuredData({
          path: '/distribution',
          name: 'Distribution Markets',
          description:
            'Continuous outcome prediction markets powered by Gaussian LMSR pricing.',
          items: [],
        })}
      />
      <DistributionPageClient />
    </>
  );
}
