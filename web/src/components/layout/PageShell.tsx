import { ReactNode } from 'react';
import { Header } from './Header';
import { BottomNav } from './BottomNav';
import { Footer } from './Footer';

export interface PageShellProps {
  children: ReactNode;
}

export function PageShell({ children }: PageShellProps) {
  return (
    <div className="flex min-h-screen flex-col">
      <Header />
      <main className="container-app flex-1 pt-page pb-24 md:pb-6">{children}</main>
      <Footer />
      <BottomNav />
    </div>
  );
}
