import { NextResponse } from 'next/server';
import { readBaseTokenState, toApiErrorPayload } from '@/lib/server/baseReadApi';

export async function GET() {
  try {
    return NextResponse.json(await readBaseTokenState());
  } catch (error) {
    const mapped = toApiErrorPayload(error);
    return NextResponse.json(mapped.payload, { status: mapped.status });
  }
}
