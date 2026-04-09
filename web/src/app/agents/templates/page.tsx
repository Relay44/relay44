import { buildPageMetadata } from '@/lib/seo';
import { AgentTemplatesClient } from './AgentTemplatesClient';

export const metadata = buildPageMetadata({
  title: 'Agent templates',
  description: 'Browse and deploy pre-built trading agent strategies on Relay44.',
  path: '/agents/templates',
  keywords: ['agent templates', 'managed agents', 'trading strategies'],
});

export default function AgentTemplatesPage() {
  return <AgentTemplatesClient />;
}
