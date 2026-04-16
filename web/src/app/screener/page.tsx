import type { Metadata } from 'next';
import ScreenerClient from './ScreenerClient';
import { buildPageMetadata } from '@/lib/seo';

export const revalidate = 10;

export async function generateMetadata(): Promise<Metadata> {
  return buildPageMetadata({
    title: 'Screener',
    description:
      'Filter live Polymarket opportunities by liquidity, score, mispricing, and category.',
    path: '/screener',
    keywords: ['market screener', 'polymarket', 'opportunity scanner'],
  });
}

export default function ScreenerPage() {
  return <ScreenerClient />;
}
