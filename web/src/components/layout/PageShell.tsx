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
      <main className="container-app pb-24 pt-20 md:pb-6">{children}</main>
      <BottomNav />
    </div>
  );
}
