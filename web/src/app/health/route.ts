import { NextResponse } from 'next/server';
import { hasApiProxyTarget, resolveApiProxyTarget } from '@/lib/server/apiTarget';
import { readHealth, toApiErrorPayload } from '@/lib/server/baseReadApi';

export async function GET() {
  try {
    if (hasApiProxyTarget()) {
      const target = `${resolveApiProxyTarget().replace(/\/v1\/?$/, '')}/health`;
      const response = await fetch(target, { cache: 'no-store' });
      if (response.ok) {
        return new NextResponse(response.body, {
          status: response.status,
          headers: response.headers,
        });
      }
    }
    return NextResponse.json(await readHealth());
  } catch (error) {
    const mapped = toApiErrorPayload(error);
    return NextResponse.json(mapped.payload, { status: mapped.status });
  }
}
