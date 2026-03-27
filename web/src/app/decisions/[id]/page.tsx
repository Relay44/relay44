import { buildPageMetadata } from '@/lib/seo';

import DecisionCellPageClient from './DecisionCellPageClient';

export const metadata = buildPageMetadata({
  title: 'Decision cell',
  description: 'Review a private decision cell, linked markets, alerts, and agent actions.',
  path: '/decisions',
  noIndex: true,
});

export default async function DecisionCellPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  return <DecisionCellPageClient cellId={decodeURIComponent(id)} />;
}
