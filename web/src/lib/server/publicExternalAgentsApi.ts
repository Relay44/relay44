import { NextRequest, NextResponse } from 'next/server';

const DEFAULT_PUBLIC_API_BASE = 'https://relay44-api.onrender.com/v1';

function normalizeBase(raw: string | undefined): string {
  const trimmed = String(raw || '').trim().replace(/\/$/, '');
  if (!trimmed) {
    return '';
  }

  if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
    return trimmed;
  }

  return `http://${trimmed}`;
}

function getApiBases(): string[] {
  const primary =
    process.env.API_PROXY_TARGET?.trim()
    || process.env.NEXT_PUBLIC_API_URL?.trim()
    || 'http://localhost:8080/v1';
  const fallback =
    process.env.NEXT_PUBLIC_API_FALLBACK_URL?.trim()
    || DEFAULT_PUBLIC_API_BASE;

  return [...new Set([primary, fallback].map(normalizeBase).filter(Boolean))];
}

function buildHeaders(request: NextRequest) {
  const headers = new Headers();
  const accept = request.headers.get('accept');
  const country = request.headers.get('x-country-code');

  if (accept) {
    headers.set('accept', accept);
  }

  if (country) {
    headers.set('x-country-code', country);
  }

  headers.set('x-forwarded-host', request.headers.get('host') || 'localhost:3000');
  headers.set('x-forwarded-proto', request.nextUrl.protocol.replace(':', ''));
  return headers;
}

function toNextResponse(response: Response) {
  const headers = new Headers(response.headers);
  headers.delete('content-encoding');
  headers.delete('content-length');

  return new NextResponse(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}

export function buildEmptyPublicExternalAgentsResponse(request: NextRequest) {
  const limit = Number(request.nextUrl.searchParams.get('limit') || 50);
  const offset = Number(request.nextUrl.searchParams.get('offset') || 0);

  return {
    agents: [],
    total: 0,
    limit: Number.isFinite(limit) && limit >= 0 ? limit : 50,
    offset: Number.isFinite(offset) && offset >= 0 ? offset : 0,
  };
}

export function buildEmptyPublicExternalAgentsPerformanceResponse() {
  return {
    scope: 'public',
    owner: null,
    totals: {
      agents: 0,
      activeAgents: 0,
      openPositions: 0,
      closedPositions: 0,
      fills: 0,
      volumeUsdc: 0,
      feesUsdc: 0,
      realizedPnlUsdc: 0,
      unrealizedPnlUsdc: 0,
      netPnlUsdc: 0,
    },
    strategies: [],
    timeline: [],
  };
}

export async function proxyPublicExternalAgentsGet(
  request: NextRequest,
  path: string,
  emptyPayload: Record<string, unknown>,
) {
  let lastResponse: Response | null = null;

  for (const base of getApiBases()) {
    const target = new URL(`${base}/${path}`);
    request.nextUrl.searchParams.forEach((value, key) => {
      target.searchParams.append(key, value);
    });

    try {
      const response = await fetch(target, {
        method: 'GET',
        headers: buildHeaders(request),
        cache: 'no-store',
        redirect: 'manual',
      });

      if (response.ok) {
        return toNextResponse(response);
      }

      lastResponse = response;
      if (response.status !== 404 && response.status < 500) {
        return toNextResponse(response);
      }
    } catch {
      // Try the next base.
    }
  }

  if (process.env.NODE_ENV !== 'production') {
    return NextResponse.json(emptyPayload);
  }

  if (lastResponse) {
    return toNextResponse(lastResponse);
  }

  return NextResponse.json(
    { error: 'External agent feed is unavailable' },
    { status: 503 },
  );
}
