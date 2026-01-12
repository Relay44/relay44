import { NextResponse } from 'next/server';
import { generateSiweNonce } from '@/lib/server/baseReadApi';

export async function GET() {
  return NextResponse.json({ nonce: generateSiweNonce() });
}
