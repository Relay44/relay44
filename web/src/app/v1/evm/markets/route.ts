import { NextRequest, NextResponse } from 'next/server';
import { toApiErrorPayload } from '@/lib/server/baseReadApi';
import { readUnifiedMarkets } from '@/lib/server/unifiedMarketsApi';

export async function GET(request: NextRequest) {
  try {
    return NextResponse.json(await readUnifiedMarkets(request.nextUrl.searchParams));
  } catch (error) {
    const mapped = toApiErrorPayload(error);
    return NextResponse.json(mapped.payload, { status: mapped.status });
  }
}
