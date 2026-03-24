'use client';

import { Moon, Sun } from 'lucide-react';
import { useTheme } from '@/components/ThemeProvider';
import { cn } from '@/lib/utils';

interface ThemeToggleProps {
  className?: string;
  iconless?: boolean;
}

export function ThemeToggle({ className, iconless = false }: ThemeToggleProps) {
  const { resolvedTheme, toggleTheme, mounted } = useTheme();
  const nextThemeLabel = resolvedTheme === 'dark' ? 'Day' : 'Night';

  // Render placeholder during SSR to prevent hydration mismatch
  if (!mounted) {
    return (
      <div
        className={cn(
          iconless
            ? 'inline-flex h-10 min-w-[88px] items-center justify-center border border-border bg-bg-secondary px-4 text-xs uppercase tracking-[0.16em] text-text-secondary'
            : 'relative flex h-10 w-10 items-center justify-center border border-border bg-bg-secondary',
          className
        )}
      />
    );
  }

  if (iconless) {
    return (
      <button
        onClick={toggleTheme}
        className={cn(
          'inline-flex h-10 min-w-[88px] cursor-pointer items-center justify-center border border-border bg-transparent px-4 text-xs uppercase tracking-[0.16em] text-text-primary transition-all duration-fast hover:border-border-hover hover:bg-bg-primary',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-bg-base',
          className
        )}
        aria-label={`Switch to ${resolvedTheme === 'dark' ? 'light' : 'dark'} mode`}
      >
        {nextThemeLabel}
      </button>
    );
  }

  return (
    <button
      onClick={toggleTheme}
      className={cn(
        'relative flex items-center justify-center w-10 h-10  cursor-pointer',
        'bg-bg-secondary border border-border hover:border-border-hover',
        'transition-all duration-fast',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-bg-base',
        className
      )}
      aria-label={`Switch to ${resolvedTheme === 'dark' ? 'light' : 'dark'} mode`}
    >
      <Sun
        className={cn(
          'h-5 w-5 transition-all duration-normal',
          resolvedTheme === 'dark'
            ? 'rotate-0 scale-100 text-text-secondary'
            : 'rotate-90 scale-0 text-text-secondary'
        )}
      />
      <Moon
        className={cn(
          'absolute h-5 w-5 transition-all duration-normal',
          resolvedTheme === 'dark'
            ? '-rotate-90 scale-0 text-text-secondary'
            : 'rotate-0 scale-100 text-text-secondary'
        )}
      />
    </button>
  );
}
