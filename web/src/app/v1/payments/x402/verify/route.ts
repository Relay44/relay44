import { NextRequest } from 'next/server';
import { proxyApiPost } from '@/lib/server/apiProxy';

export async function POST(request: NextRequest) {
  return proxyApiPost(request, 'payments/x402/verify');
}
