import { NextRequest, NextResponse } from 'next/server';
import { proxyApiGet } from '@/lib/server/apiProxy';

export async function GET(
  request: NextRequest,
  context: { params: Promise<{ marketId: string }> }
) {
  const { marketId } = await context.params;
  return proxyApiGet(request, `evm/markets/${encodeURIComponent(marketId)}/orderbook`);
}
