'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { Home, TrendingUp, Bot, GitBranch } from 'lucide-react';
import { cn } from '@/lib/utils';

const navItems = [
  { href: '/', label: 'Home', icon: Home },
  { href: '/markets', label: 'Markets', icon: TrendingUp },
  { href: '/decisions', label: 'Decisions', icon: GitBranch },
  { href: '/agents', label: 'Agents', icon: Bot },
];

export function BottomNav() {
  const pathname = usePathname();

  return (
    <nav className="fixed bottom-0 left-0 right-0 z-sticky glass border-t border-border md:hidden safe-area-inset">
      <div className="flex h-14 items-center justify-around">
        {navItems.map(({ href, label, icon: Icon }) => {
          const isActive = pathname === href || (href !== '/' && pathname.startsWith(href));
          return (
            <Link
              key={href}
              href={href}
              className={cn(
                'flex h-full w-full flex-col items-center justify-center gap-0.5',
                'transition-all duration-fast',
                isActive
                  ? 'text-accent'
                  : 'text-text-muted hover:text-text-secondary'
              )}
            >
              <Icon
                className={cn(
                  'w-5 h-5 transition-transform duration-fast',
                  isActive && 'scale-110'
                )}
              />
              <span className={cn(
                'text-xs leading-none',
                isActive ? 'font-medium' : 'font-normal'
              )}>
                {label}
              </span>
            </Link>
          );
        })}
      </div>
    </nav>
  );
}
