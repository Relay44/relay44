import { ReactNode } from 'react';
import { Header } from './Header';
import { BottomNav } from './BottomNav';

export interface PageShellProps {
  children: ReactNode;
}

export function PageShell({ children }: PageShellProps) {
  return (
    <div className="min-h-screen">
      <Header />
      <main className="container-app pt-page pb-24 md:pb-6">{children}</main>
      <BottomNav />
    </div>
  );
}
