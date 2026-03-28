import { NextRequest, NextResponse } from 'next/server';
import { hasApiProxyTarget } from '@/lib/server/apiTarget';
import { proxyApiGet } from '@/lib/server/apiProxy';

export async function GET(
  request: NextRequest,
  context: { params: Promise<{ agentId: string }> }
) {
  const { agentId } = await context.params;

  if (hasApiProxyTarget()) {
    try {
      const response = await proxyApiGet(request, `evm/agents/${encodeURIComponent(agentId)}`);
      if (response.ok) {
        return response;
      }
    } catch {
      // Fall through to a factual not-found response when no backend is reachable.
    }
  }

  return NextResponse.json(
    { code: 'AGENT_NOT_FOUND', error: 'Agent runtime is unavailable in this environment' },
    { status: 404 }
  );
}
