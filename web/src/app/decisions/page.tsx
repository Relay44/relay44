import { buildPageMetadata } from '@/lib/seo';

import DecisionsPageClient from './DecisionsPageClient';

export const metadata = buildPageMetadata({
  title: 'Decision cells',
  description: 'Private decision systems driven by linked markets, thresholds, and external-agent automation.',
  path: '/decisions',
  noIndex: true,
});

export default function DecisionsPage() {
  return <DecisionsPageClient />;
}
