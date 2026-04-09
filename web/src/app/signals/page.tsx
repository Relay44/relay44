import type { Metadata } from 'next';
import SignalsPageClient from './SignalsPageClient';
import { buildPageMetadata } from '@/lib/seo';

export const revalidate = 10;

export async function generateMetadata(): Promise<Metadata> {
  return buildPageMetadata({
    title: 'Signals',
    description: 'Browse signal providers with Brier-scored prediction track records on Relay44.',
    path: '/signals',
    keywords: ['signal providers', 'prediction signals', 'Brier score'],
  });
}

export default function SignalsPage() {
  return <SignalsPageClient />;
}
