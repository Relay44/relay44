import { NextRequest, NextResponse } from 'next/server';
import { buildLocalWeb4Capabilities } from '@/lib/server/web4Capabilities';

export async function GET(request: NextRequest) {
  return NextResponse.json(buildLocalWeb4Capabilities(request));
}
