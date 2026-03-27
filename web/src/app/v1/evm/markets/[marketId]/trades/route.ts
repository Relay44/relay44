import { NextRequest, NextResponse } from 'next/server';
import { toApiErrorPayload } from '@/lib/server/baseReadApi';
import { readUnifiedTrades } from '@/lib/server/unifiedMarketsApi';

export async function GET(
  request: NextRequest,
  context: { params: Promise<{ marketId: string }> }
) {
  try {
    const { marketId } = await context.params;
    return NextResponse.json(await readUnifiedTrades(marketId, request.nextUrl.searchParams));
  } catch (error) {
    const mapped = toApiErrorPayload(error);
    return NextResponse.json(mapped.payload, { status: mapped.status });
  }
}
