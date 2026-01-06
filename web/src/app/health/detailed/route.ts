import { NextResponse } from 'next/server';
import { readDetailedHealth, toApiErrorPayload } from '@/lib/server/baseReadApi';

export async function GET() {
  try {
    return NextResponse.json(await readDetailedHealth());
  } catch (error) {
    const mapped = toApiErrorPayload(error);
    return NextResponse.json(mapped.payload, { status: mapped.status });
  }
}
