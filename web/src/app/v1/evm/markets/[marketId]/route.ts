import { NextResponse } from 'next/server';
import { toApiErrorPayload } from '@/lib/server/baseReadApi';
import { readUnifiedMarket } from '@/lib/server/unifiedMarketsApi';

export async function GET(
  _request: Request,
  context: { params: Promise<{ marketId: string }> }
) {
  try {
    const { marketId } = await context.params;
    return NextResponse.json(await readUnifiedMarket(marketId));
  } catch (error) {
    const mapped = toApiErrorPayload(error);
    return NextResponse.json(mapped.payload, { status: mapped.status });
  }
}
