import { NextRequest, NextResponse } from 'next/server';

export const runtime = 'nodejs';
export const dynamic = 'force-dynamic';

const CONTEXT_GRAPH_URL = process.env.NEXT_PUBLIC_CONTEXT_GRAPH_URL || '';
const SHARED_SECRET = process.env.CONTEXT_GRAPH_SHARED_SECRET || '';

function resolveTarget(path: string[], search: URLSearchParams): string {
  const base = CONTEXT_GRAPH_URL.replace(/\/+$/, '');
  const url = new URL(`${base}/api/context-graph/${path.join('/')}`);
  search.forEach((v, k) => url.searchParams.append(k, v));
  return url.toString();
}

async function proxyRequest(request: NextRequest, { params }: { params: Promise<{ path: string[] }> }) {
  const { path } = await params;

  if (!CONTEXT_GRAPH_URL) {
    return NextResponse.json({ error: 'Context graph service not configured' }, { status: 503 });
  }

  const target = resolveTarget(path, request.nextUrl.searchParams);

  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  };

  // Inject shared secret for write operations (server-side only)
  if (SHARED_SECRET && (request.method === 'POST' || request.method === 'PUT' || request.method === 'DELETE')) {
    headers['Authorization'] = `Bearer ${SHARED_SECRET}`;
  }

  try {
    const fetchInit: RequestInit = {
      method: request.method,
      headers,
    };

    if (request.method !== 'GET' && request.method !== 'HEAD') {
      fetchInit.body = await request.text();
    }

    const res = await fetch(target, fetchInit);

    const body = await res.text();
    return new NextResponse(body, {
      status: res.status,
      headers: {
        'Content-Type': res.headers.get('Content-Type') || 'application/json',
      },
    });
  } catch (err) {
    console.error('[context-graph-proxy] Error:', err);
    return NextResponse.json({ error: 'Context graph service unavailable' }, { status: 502 });
  }
}

export const GET = proxyRequest;
export const POST = proxyRequest;
