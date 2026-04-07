import { NextRequest, NextResponse } from 'next/server';

export const runtime = 'nodejs';
export const dynamic = 'force-dynamic';

const CONTEXT_GRAPH_URL = process.env.NEXT_PUBLIC_CONTEXT_GRAPH_URL;
const SHARED_SECRET = process.env.CONTEXT_GRAPH_SHARED_SECRET;

async function proxy(request: NextRequest, { params }: { params: Promise<{ path: string[] }> }) {
  if (!CONTEXT_GRAPH_URL) {
    return NextResponse.json({ error: 'Context graph service not configured' }, { status: 503 });
  }

  const { path } = await params;
  const target = new URL(`${CONTEXT_GRAPH_URL}/${path.join('/')}`);
  request.nextUrl.searchParams.forEach((v, k) => target.searchParams.append(k, v));

  const method = request.method.toUpperCase();
  const body = method === 'GET' || method === 'HEAD'
    ? undefined
    : await request.arrayBuffer();

  const headers = new Headers();
  headers.set('Content-Type', request.headers.get('Content-Type') || 'application/json');
  if (SHARED_SECRET) {
    headers.set('Authorization', `Bearer ${SHARED_SECRET}`);
  }

  const upstream = await fetch(target, {
    method,
    headers,
    body: body && body.byteLength > 0 ? body : undefined,
    cache: 'no-store',
  });

  const responseHeaders = new Headers(upstream.headers);
  responseHeaders.delete('content-encoding');
  responseHeaders.delete('content-length');

  return new NextResponse(upstream.body, {
    status: upstream.status,
    statusText: upstream.statusText,
    headers: responseHeaders,
  });
}

export const GET = proxy;
export const POST = proxy;
export const PUT = proxy;
export const DELETE = proxy;
