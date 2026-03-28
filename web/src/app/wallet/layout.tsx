import { buildPageMetadata } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Wallet',
  description: 'Review vault balance, transaction history, and transfer actions for your connected wallet on Relay44.',
  path: '/wallet',
  noIndex: true,
});

export default function WalletLayout({ children }: { children: React.ReactNode }) {
  return <>{children}</>;
}
