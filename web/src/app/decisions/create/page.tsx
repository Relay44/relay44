import { buildPageMetadata } from '@/lib/seo';

import CreateDecisionCellClient from './CreateDecisionCellClient';

export const metadata = buildPageMetadata({
  title: 'Create decision cell',
  description: 'Create a private decision cell with linked markets, thresholds, and external-agent automation.',
  path: '/decisions/create',
  noIndex: true,
});

export default function CreateDecisionCellPage() {
  return <CreateDecisionCellClient />;
}
