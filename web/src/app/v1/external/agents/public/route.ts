import { NextRequest } from 'next/server';
import {
  buildEmptyPublicExternalAgentsResponse,
  proxyPublicExternalAgentsGet,
} from '@/lib/server/publicExternalAgentsApi';

export async function GET(request: NextRequest) {
  return proxyPublicExternalAgentsGet(
    request,
    'external/agents/public',
    buildEmptyPublicExternalAgentsResponse(request),
  );
}
