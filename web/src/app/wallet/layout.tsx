import { buildPageMetadata } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Wallet',
  description: 'Wallet balances, transfers, and account state for relay44.',
  path: '/wallet',
  noIndex: true,
});

export default function WalletLayout({ children }: { children: React.ReactNode }) {
  return children;
}
