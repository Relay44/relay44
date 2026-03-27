import { buildPageMetadata } from '@/lib/seo';

import DecisionsPageClient from './DecisionsPageClient';

export const metadata = buildPageMetadata({
  title: 'Decision cells',
  description: 'Private decision cells for linking markets, alerts, and agent actions.',
  path: '/decisions',
  noIndex: true,
});

export default function DecisionsPage() {
  return <DecisionsPageClient />;
}
