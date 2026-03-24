import { NextRequest } from 'next/server';
import { proxyApiGet } from '@/lib/server/apiProxy';

export async function GET(request: NextRequest) {
  return proxyApiGet(request, 'payments/x402/quote');
}
