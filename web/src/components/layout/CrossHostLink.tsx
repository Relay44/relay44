'use client';

import Link from 'next/link';
import { useEffect, useState } from 'react';

// On docs.relay44.com, only /docs/* routes exist (see web/src/middleware.ts).
// Links that target non-docs routes must jump to the apex host instead of
// staying on docs.relay44.com where they would 404.
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

interface CrossHostLinkProps {
  href: string;
  className?: string;
  children: React.ReactNode;
}

/**
 * Renders a next/link for same-host targets and a plain <a> for cross-host
 * targets. When served from docs.relay44.com, any href that does not start
 * with /docs is rewritten to the apex origin.
 */
export function CrossHostLink({ href, className, children }: CrossHostLinkProps) {
  const isDocsHost = useIsDocsHost();
  const crossHost = isDocsHost && !href.startsWith('/docs');
  const resolved = crossHost ? `${APEX_ORIGIN}${href}` : href;

  if (crossHost) {
    return (
      <a href={resolved} className={className}>
        {children}
      </a>
    );
  }
  return (
    <Link href={resolved} className={className}>
      {children}
    </Link>
  );
}
