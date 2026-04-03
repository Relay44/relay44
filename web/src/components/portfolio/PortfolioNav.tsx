'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';

import { cn } from '@/lib/utils';

const items = [
  { href: '/portfolio', label: 'Positions' },
  { href: '/portfolio/creator', label: 'Creator' },
];

export function PortfolioNav() {
  const pathname = usePathname();

  return (
    <nav className="mb-6 border-b border-border">
      <div className="flex flex-wrap gap-2">
        {items.map((item) => {
          const active = pathname === item.href;

          return (
            <Link
              key={item.href}
              href={item.href}
              className={cn(
                'inline-flex h-10 items-center border-b-2 px-1 text-sm font-medium transition-colors',
                active
                  ? 'border-accent text-text-primary'
                  : 'border-transparent text-text-secondary hover:text-text-primary',
              )}
            >
              {item.label}
            </Link>
          );
        })}
      </div>
    </nav>
  );
}
