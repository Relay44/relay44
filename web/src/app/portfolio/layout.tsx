import { buildPageMetadata } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Portfolio',
  description: 'Portfolio balances, positions, and open orders for relay44.',
  path: '/portfolio',
  noIndex: true,
});

export default function PortfolioLayout({ children }: { children: React.ReactNode }) {
  return children;
}
