import { NextResponse } from 'next/server';
import { hasApiProxyTarget, resolveApiProxyTarget } from '@/lib/server/apiTarget';
import { readDetailedHealth, toApiErrorPayload } from '@/lib/server/baseReadApi';

export async function GET() {
  try {
    if (hasApiProxyTarget()) {
      const target = `${resolveApiProxyTarget().replace(/\/v1\/?$/, '')}/health/detailed`;
      try {
        const response = await fetch(target, {
          cache: 'no-store',
          signal: AbortSignal.timeout(5_000),
        });
        if (response.ok) {
          return new NextResponse(response.body, {
            status: response.status,
            headers: response.headers,
          });
        }
      } catch {
        // Fall back to local health when the API detailed check is slow or unavailable.
      }
    }
    return NextResponse.json(await readDetailedHealth());
  } catch (error) {
    const mapped = toApiErrorPayload(error);
    return NextResponse.json(mapped.payload, { status: mapped.status });
  }
}
