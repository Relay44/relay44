import { NextRequest, NextResponse } from 'next/server';
import { hasApiProxyTarget } from '@/lib/server/apiTarget';
import { proxyApiGet } from '@/lib/server/apiProxy';
import { buildLocalWeb4Capabilities } from '@/lib/server/web4Capabilities';

export async function GET(request: NextRequest) {
  if (hasApiProxyTarget()) {
    try {
      const response = await proxyApiGet(request, 'web4/capabilities');
      if (response.ok) {
        return response;
      }
    } catch {
      // Fall back to local capability inference when the backend is unavailable.
    }
  }

  return NextResponse.json(buildLocalWeb4Capabilities(request));
}
