import { buildPageMetadata } from '@/lib/seo';

import DecisionCellPageClient from './DecisionCellPageClient';

export const metadata = buildPageMetadata({
  title: 'Decision cell',
  description: 'Inspect a private decision cell, its linked nodes, thresholds, and external-agent automation.',
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
