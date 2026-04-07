import { buildPageMetadata } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Staking',
  description: 'Stake $RELAY to earn rewards, unlock fee discounts, and gain platform benefits on Relay44.',
  path: '/staking',
});

export default function StakingLayout({ children }: { children: React.ReactNode }) {
  return <>{children}</>;
}
