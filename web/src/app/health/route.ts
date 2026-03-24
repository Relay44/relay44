import { NextResponse } from 'next/server';
import { readHealth, toApiErrorPayload } from '@/lib/server/baseReadApi';

export async function GET() {
  try {
    return NextResponse.json(await readHealth());
  } catch (error) {
    const mapped = toApiErrorPayload(error);
    return NextResponse.json(mapped.payload, { status: mapped.status });
  }
}
