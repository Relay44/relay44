import { NextRequest, NextResponse } from 'next/server';

export const runtime = 'nodejs';
export const dynamic = 'force-dynamic';

function normalizeTarget(raw: string | undefined) {
  const trimmed = String(raw || '').trim().replace(/\/$/, '');
  if (!trimmed) {
    return '';
  }
  if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
    return trimmed;
  }
  return `http://${trimmed}`;
}

const API_PROXY_TARGET = normalizeTarget(
  process.env.API_PROXY_TARGET ||
    process.env.NEXT_PUBLIC_API_URL ||
    'http://localhost:8080/v1'
);
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
  const target = new URL(`${API_PROXY_TARGET}/${path.join('/')}`);
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
  if (!API_PROXY_TARGET) {
    return NextResponse.json({ error: 'API proxy target is not configured' }, { status: 500 });
  }

  const { path } = await params;
  if (!path || path.length === 0) {
    return NextResponse.json({ error: 'Missing proxy path' }, { status: 400 });
  }

  const target = buildTargetUrl(request, path);
  const method = request.method.toUpperCase();

  if (READ_ONLY_MODE && !['GET', 'HEAD', 'OPTIONS'].includes(method)) {
    return NextResponse.json(
      { error: 'This action is disabled in read-only mode' },
      { status: 403 }
    );
  }

  const headers = buildProxyHeaders(request);
  const body = method === 'GET' || method === 'HEAD' ? undefined : await request.text();

  const response = await fetch(target, {
    method,
    headers,
    body,
    cache: 'no-store',
    redirect: 'manual',
  });

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
