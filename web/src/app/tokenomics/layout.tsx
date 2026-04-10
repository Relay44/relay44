import type { Metadata } from 'next';
import { PageShell } from '@/components/layout';
import { buildPageMetadata } from '@/lib/seo';

export const metadata: Metadata = buildPageMetadata({
  title: 'Tokenomics',
  description:
    'How $RELAY captures value from the Relay44 Protocol — fee routing, staking tiers, reward distribution, and the roadmap to fee-through-$RELAY.',
  path: '/tokenomics',
  image: '/tokenomics/opengraph-image',
  keywords: [
    'tokenomics',
    'RELAY token',
    'staking',
    'fee capture',
    'reward distribution',
    'protocol revenue',
    'base prediction markets',
  ],
});

export default function TokenomicsLayout({ children }: { children: React.ReactNode }) {
  return <PageShell>{children}</PageShell>;
}
