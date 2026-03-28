import { NextRequest, NextResponse } from 'next/server';
import { resolveApiProxyTarget } from '@/lib/server/apiTarget';

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

export async function proxyApiGet(request: NextRequest, path: string) {
  return proxyApiRequest(request, path);
}

export async function proxyApiPost(request: NextRequest, path: string) {
  return proxyApiRequest(request, path);
}

export async function proxyApiRequest(request: NextRequest, path: string) {
  const proxyTarget = resolveApiProxyTarget();

  if (!proxyTarget) {
    return NextResponse.json({ error: 'API proxy target is not configured' }, { status: 500 });
  }

  const target = new URL(`${proxyTarget}/${path}`);
  request.nextUrl.searchParams.forEach((value, key) => {
    target.searchParams.append(key, value);
  });

  const method = request.method.toUpperCase();
  const body =
    method === 'GET' || method === 'HEAD' ? undefined : await request.arrayBuffer();

  const response = await fetch(target, {
    method,
    headers: buildProxyHeaders(request),
    body: body && body.byteLength > 0 ? body : undefined,
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
