import type { Metadata } from 'next';
import { MiniKitInit } from './MiniKitInit';

export const metadata: Metadata = {
  title: 'relay44 | Mini App',
  robots: { index: false, follow: false },
};

export default function MiniAppLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <div className="min-h-screen bg-bg-primary text-text-primary">
      <MiniKitInit />
      <main className="px-4 py-3 pb-safe">{children}</main>
    </div>
  );
}
