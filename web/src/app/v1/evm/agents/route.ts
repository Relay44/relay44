import { NextRequest, NextResponse } from 'next/server';
import { hasApiProxyTarget } from '@/lib/server/apiTarget';
import { proxyApiGet } from '@/lib/server/apiProxy';

function parseInteger(raw: string | null, fallback: number): number {
  if (!raw) {
    return fallback;
  }

  const parsed = Number(raw);
  if (!Number.isInteger(parsed) || parsed < 0) {
    return fallback;
  }

  return parsed;
}

export async function GET(request: NextRequest) {
  if (hasApiProxyTarget()) {
    try {
      const response = await proxyApiGet(request, 'evm/agents');
      if (response.ok) {
        return response;
      }
    } catch {
      // Return a factual empty runtime set when no live backend is reachable.
    }
  }

  const limit = parseInteger(request.nextUrl.searchParams.get('limit'), 50);
  const offset = parseInteger(request.nextUrl.searchParams.get('offset'), 0);

  return NextResponse.json({
    agents: [],
    total: 0,
    limit,
    offset,
  });
}
