'use client';

import Link from 'next/link';
import { useEffect, useState } from 'react';
import { usePathname } from 'next/navigation';
import { cn } from '@/lib/utils';

// See SidebarMenu.tsx — on docs.relay44.com only /docs/* routes exist, so
// non-docs footer links must jump to the apex.
const DOCS_HOST = 'docs.relay44.com';
const APEX_ORIGIN = 'https://relay44.com';

function useIsDocsHost() {
  const [isDocs, setIsDocs] = useState(false);
  useEffect(() => {
    if (typeof window === 'undefined') return;
    setIsDocs(window.location.hostname === DOCS_HOST);
  }, []);
  return isDocs;
}

const productLinks = [
  { href: '/markets', label: 'Markets' },
  { href: '/distribution', label: 'Distribution' },
  { href: '/decisions', label: 'Decisions' },
  { href: '/agents', label: 'Swarm' },
  { href: '/portfolio', label: 'Portfolio' },
];

const platformLinks = [
  { href: '/staking', label: 'Staking' },
  { href: '/leaderboard', label: 'Leaderboard' },
  { href: '/insights', label: 'Insights' },
  { href: '/identity', label: 'Identity' },
  { href: '/context-graph', label: 'Context Graph' },
];

const resourceLinks = [
  { href: '/how-it-works', label: 'How it works' },
  { href: '/docs', label: 'Docs' },
  { href: '/docs/protocol', label: 'Protocol' },
  { href: '/docs/contracts', label: 'Protocol Reference' },
  { href: '/tokenomics', label: 'Tokenomics' },
];

const externalLinks = [
  { href: 'https://x.com/Relay44BASE', label: 'X / Relay44BASE' },
];

function FooterColumn({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <p className="text-[0.65rem] uppercase tracking-[0.2em] text-text-muted mb-3">
        {title}
      </p>
      <ul className="space-y-2">{children}</ul>
    </div>
  );
}

export function Footer() {
  const pathname = usePathname();
  const isDocsHost = useIsDocsHost();

  const linkClass = (href: string, crossHost: boolean) =>
    cn(
      'text-xs transition-colors',
      !crossHost &&
        (pathname === href || (href !== '/' && pathname.startsWith(href)))
        ? 'text-text-primary'
        : 'text-text-secondary hover:text-text-primary',
    );

  const renderLink = ({ href, label }: { href: string; label: string }) => {
    const crossHost = isDocsHost && !href.startsWith('/docs');
    const resolved = crossHost ? `${APEX_ORIGIN}${href}` : href;
    if (crossHost) {
      return (
        <a href={resolved} className={linkClass(href, true)}>
          {label}
        </a>
      );
    }
    return (
      <Link href={resolved} className={linkClass(href, false)}>
        {label}
      </Link>
    );
  };

  return (
    <footer className="border-t border-border bg-bg-base hidden md:block">
      <div className="max-w-[1400px] mx-auto px-4 sm:px-8 py-10">
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-8">
          <FooterColumn title="Product">
            {productLinks.map((link) => (
              <li key={link.href}>{renderLink(link)}</li>
            ))}
          </FooterColumn>

          <FooterColumn title="Platform">
            {platformLinks.map((link) => (
              <li key={link.href}>{renderLink(link)}</li>
            ))}
          </FooterColumn>

          <FooterColumn title="Resources">
            {resourceLinks.map((link) => (
              <li key={link.href}>{renderLink(link)}</li>
            ))}
          </FooterColumn>

          <FooterColumn title="Community">
            {externalLinks.map(({ href, label }) => (
              <li key={href}>
                <a
                  href={href}
                  target="_blank"
                  rel="noreferrer"
                  className="text-xs text-text-secondary hover:text-text-primary transition-colors"
                >
                  {label}
                </a>
              </li>
            ))}
          </FooterColumn>
        </div>

        <div className="mt-8 pt-6 border-t border-border/50 flex items-center justify-between">
          <p className="text-[0.65rem] text-text-muted font-mono">
            Relay44
          </p>
          <p className="text-[0.65rem] text-text-muted font-mono">
            Built on Base
          </p>
        </div>
      </div>
    </footer>
  );
}
