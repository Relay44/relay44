import { NextRequest } from 'next/server';
import { proxyApiGet, proxyApiPost } from '@/lib/server/apiProxy';

export async function GET(request: NextRequest) {
  return proxyApiGet(request, 'web4/mcp');
}

export async function POST(request: NextRequest) {
  return proxyApiPost(request, 'web4/mcp');
}
