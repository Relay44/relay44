import { buildPageMetadata } from '@/lib/seo';
import { MiniKitInit } from './MiniKitInit';

export const metadata = buildPageMetadata({
  title: 'Miniapp',
  description: 'Trade prediction markets natively inside Farcaster with the Relay44 miniapp.',
  path: '/miniapp',
  keywords: ['farcaster', 'miniapp', 'warpcast', 'prediction markets'],
});

export default function MiniappLayout({ children }: { children: React.ReactNode }) {
  return (
    <>
      <MiniKitInit />
      {children}
    </>
  );
}
