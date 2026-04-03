import { buildPageMetadata } from '@/lib/seo';
import { PageShell } from '@/components/layout';
import { PortfolioNav } from '@/components/portfolio/PortfolioNav';

export const metadata = buildPageMetadata({
  title: 'Portfolio',
  description: 'Track active positions, unrealized P&L, open orders, and claimable balances on Relay44.',
  path: '/portfolio',
  noIndex: true,
});

export default function PortfolioLayout({ children }: { children: React.ReactNode }) {
  return (
    <PageShell>
      <PortfolioNav />
      {children}
    </PageShell>
  );
}
