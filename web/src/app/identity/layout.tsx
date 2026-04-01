import { buildPageMetadata } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Agent Identity',
  description: 'Register your on-chain ERC-8004 agent identity on Base.',
  path: '/identity',
});

export default function IdentityLayout({ children }: { children: React.ReactNode }) {
  return <>{children}</>;
}
