import { NextRequest, NextResponse } from 'next/server';

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
  if (!API_PROXY_TARGET) {
    return NextResponse.json({ error: 'API proxy target is not configured' }, { status: 500 });
  }

