'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { ReactNode, useState } from 'react';
import { docsNav, flattenNav, isNavGroup } from '@/lib/docs-nav';

interface DocsLayoutProps {
  children: ReactNode;
}

export function DocsLayout({ children }: DocsLayoutProps) {
  const pathname = usePathname();
  const [sidebarOpen, setSidebarOpen] = useState(false);

  const flat = flattenNav();
  const currentIndex = flat.findIndex((item) => item.href === pathname);
  const prev = currentIndex > 0 ? flat[currentIndex - 1] : null;
  const next = currentIndex >= 0 && currentIndex < flat.length - 1 ? flat[currentIndex + 1] : null;

  return (
    <div className="mx-auto max-w-7xl lg:grid lg:grid-cols-[16rem_minmax(0,1fr)] lg:gap-8">
      {/* Mobile toggle */}
      <button
        onClick={() => setSidebarOpen(!sidebarOpen)}
        className="mb-4 flex items-center gap-2 text-sm text-text-secondary transition-colors hover:text-text-primary lg:hidden"
      >
        <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d={sidebarOpen ? 'M6 18L18 6M6 6l12 12' : 'M4 6h16M4 12h16M4 18h16'} />
        </svg>
        {sidebarOpen ? 'Close menu' : 'Docs menu'}
      </button>

      {/* Sidebar */}
      <aside className={`${sidebarOpen ? 'block' : 'hidden'} mb-8 lg:sticky lg:top-20 lg:block lg:h-[calc(100vh-5rem)] lg:overflow-y-auto lg:pr-4`}>
        <nav className="space-y-6">
          {docsNav.map((entry) => {
            if (isNavGroup(entry)) {
              return (
                <div key={entry.title}>
                  <h3 className="mb-2 text-[0.7rem] font-medium uppercase tracking-widest text-text-muted">
                    {entry.title}
                  </h3>
                  <ul className="space-y-1">
                    {entry.items.map((item) => (
                      <li key={item.href}>
                        <Link
                          href={item.href}
                          onClick={() => setSidebarOpen(false)}
                          className={`block py-1 text-sm transition-colors ${
                            pathname === item.href
                              ? 'font-medium text-text-primary'
                              : 'text-text-secondary hover:text-text-primary'
                          }`}
                        >
                          {item.title}
                        </Link>
                      </li>
                    ))}
                  </ul>
                </div>
              );
            }

            return (
              <Link
                key={entry.href}
                href={entry.href}
                onClick={() => setSidebarOpen(false)}
                className={`block text-sm transition-colors ${
                  pathname === entry.href
                    ? 'font-medium text-text-primary'
                    : 'text-text-secondary hover:text-text-primary'
                }`}
              >
                {entry.title}
              </Link>
            );
          })}
        </nav>
      </aside>

      {/* Main content */}
      <div className="min-w-0">
        <div className="max-w-3xl">{children}</div>

        {/* Prev / Next */}
        {(prev || next) && (
          <div className="mt-12 flex items-center justify-between border-t border-border pt-6">
            {prev ? (
              <Link
                href={prev.href}
                className="text-sm text-text-secondary transition-colors hover:text-text-primary"
              >
                &larr; {prev.title}
              </Link>
            ) : (
              <span />
            )}
            {next ? (
              <Link
                href={next.href}
                className="text-sm text-text-secondary transition-colors hover:text-text-primary"
              >
                {next.title} &rarr;
              </Link>
            ) : (
              <span />
            )}
          </div>
        )}
      </div>
    </div>
  );
}
