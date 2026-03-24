import { NextResponse } from 'next/server';
import { getHomeLiveFeed } from '@/lib/server/homeLive';

export const runtime = 'nodejs';
export const dynamic = 'force-dynamic';

export async function GET() {
  const payload = await getHomeLiveFeed();
  return NextResponse.json(payload, {
    headers: {
      'Cache-Control': 'public, s-maxage=60, stale-while-revalidate=300',
    },
  });
}
