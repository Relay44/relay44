import { NextRequest, NextResponse } from 'next/server';
import { resolveApiProxyTarget } from '@/lib/server/apiTarget';

export const runtime = 'nodejs';
export const dynamic = 'force-dynamic';

const READ_ONLY_MODE = ['1', 'true', 'yes', 'on'].includes(
  String(process.env.NEXT_PUBLIC_READ_ONLY_MODE || '')
    .trim()
    .toLowerCase()
);

const HOP_BY_HOP_HEADERS = new Set([
  'connection',
  'content-length',
  'host',
  'keep-alive',
  'proxy-authenticate',
  'proxy-authorization',
  'te',
  'trailer',
  'transfer-encoding',
  'upgrade',
]);

function buildTargetUrl(request: NextRequest, path: string[]) {
  const proxyTarget = resolveApiProxyTarget();
  const target = new URL(`${proxyTarget}/${path.join('/')}`);
  request.nextUrl.searchParams.forEach((value, key) => {
    target.searchParams.append(key, value);
  });
  return target;
}

function buildLocalTargetUrl(request: NextRequest, path: string[]) {
  const normalizedPath = path[0] === 'v1' ? path.join('/') : `v1/${path.join('/')}`;
  const target = new URL(`/${normalizedPath}`, request.nextUrl.origin);
  request.nextUrl.searchParams.forEach((value, key) => {
    target.searchParams.append(key, value);
  });
  return target;
}

function buildProxyHeaders(request: NextRequest) {
  const headers = new Headers();

  request.headers.forEach((value, key) => {
    const normalized = key.toLowerCase();
    if (
      HOP_BY_HOP_HEADERS.has(normalized) ||
      normalized === 'origin' ||
      normalized === 'referer'
    ) {
      return;
    }
    headers.set(key, value);
  });

  headers.set('x-forwarded-host', request.headers.get('host') || 'localhost:3000');
  headers.set('x-forwarded-proto', request.nextUrl.protocol.replace(':', ''));
  return headers;
}

async function proxyRequest(
  request: NextRequest,
  { params }: { params: Promise<{ path: string[] }> }
) {
  const { path } = await params;
  if (!path || path.length === 0) {
    return NextResponse.json({ error: 'Missing proxy path' }, { status: 400 });
  }
  const method = request.method.toUpperCase();

  if (READ_ONLY_MODE && !['GET', 'HEAD', 'OPTIONS'].includes(method)) {
    return NextResponse.json(
      { error: 'This action is unavailable in this environment' },
      { status: 403 }
    );
  }

  const proxyTarget = resolveApiProxyTarget();
  const canUseLocalFallback = ['GET', 'HEAD', 'OPTIONS'].includes(method);

  if (!proxyTarget && !canUseLocalFallback) {
    return NextResponse.json({ error: 'API proxy target is not configured' }, { status: 500 });
  }

  const headers = buildProxyHeaders(request);
  const body = method === 'GET' || method === 'HEAD' ? undefined : await request.text();
  const localTarget = buildLocalTargetUrl(request, path);

  const send = (target: URL) =>
    fetch(target, {
      method,
      headers,
      body,
      cache: 'no-store',
      redirect: 'manual',
    });

  let response: Response;
  let usedLocalTarget = !proxyTarget;
  try {
    response = proxyTarget ? await send(buildTargetUrl(request, path)) : await send(localTarget);
  } catch (error) {
    if (!canUseLocalFallback || !proxyTarget) {
      throw error;
    }
    usedLocalTarget = true;
    response = await send(localTarget);
  }

  if (proxyTarget && canUseLocalFallback && response.status >= 500) {
    const fallback = await send(localTarget);
    if (fallback.ok || fallback.status < 500) {
      usedLocalTarget = true;
      response = fallback;
    }
  }

  const contentType = response.headers.get('content-type')?.toLowerCase() || '';
  if (usedLocalTarget && response.status === 404 && contentType.includes('text/html')) {
    return NextResponse.json(
      { error: 'Endpoint is unavailable in this deployment' },
      { status: 404 }
    );
  }

  const responseHeaders = new Headers(response.headers);
  responseHeaders.delete('content-encoding');
  responseHeaders.delete('content-length');

  return new NextResponse(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers: responseHeaders,
  });
}

export { proxyRequest as GET };
export { proxyRequest as POST };
export { proxyRequest as PATCH };
export { proxyRequest as PUT };
export { proxyRequest as DELETE };
