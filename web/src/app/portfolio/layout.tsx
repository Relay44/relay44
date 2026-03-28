import { buildPageMetadata } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Portfolio',
  description: 'Track active positions, unrealized P&L, open orders, and claimable balances on Relay44.',
  path: '/portfolio',
  noIndex: true,
});

export default function PortfolioLayout({ children }: { children: React.ReactNode }) {
  return <>{children}</>;
}
