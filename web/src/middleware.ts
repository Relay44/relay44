import { NextRequest, NextResponse } from 'next/server';

const DOCS_HOST = 'docs.relay44.com';

export function middleware(request: NextRequest) {
  const host = request.headers.get('host')?.replace(/:\d+$/, '') ?? '';

  if (host !== DOCS_HOST) {
    return NextResponse.next();
  }

  // On docs subdomain: rewrite /path → /docs/path
  const { pathname } = request.nextUrl;

  // Already under /docs — pass through (handles internal Next.js navigation)
  if (pathname.startsWith('/docs')) {
    return NextResponse.next();
  }

  // Root → /docs landing
  const target = pathname === '/' ? '/docs' : `/docs${pathname}`;

  const url = request.nextUrl.clone();
  url.pathname = target;
  return NextResponse.rewrite(url);
}

export const config = {
  matcher: [
    // Match all paths except Next.js internals and static files
    '/((?!_next/static|_next/image|favicon\\.ico|.*\\.(?:svg|png|jpg|jpeg|gif|webp|ico|json|txt|xml|webmanifest)).*)',
  ],
};
