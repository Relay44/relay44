import { NextResponse } from 'next/server';

import {
  ERC20_ABI,
  MARKET_CORE_ABI,
  ORDER_BOOK_ABI,
  RELAY_STAKING_ABI,
} from '@/lib/contracts';

type AbiJson = readonly Record<string, unknown>[];

const ABIS: Record<string, { name: string; abi: AbiJson }> = {
  'market-core': { name: 'MarketCore', abi: MARKET_CORE_ABI as unknown as AbiJson },
  'order-book': { name: 'OrderBook', abi: ORDER_BOOK_ABI as unknown as AbiJson },
  erc20: { name: 'ERC20', abi: ERC20_ABI as unknown as AbiJson },
  'relay-staking': { name: 'RelayStaking', abi: RELAY_STAKING_ABI as unknown as AbiJson },
};

export const dynamic = 'force-static';

export function generateStaticParams(): { name: string }[] {
  return Object.keys(ABIS).map((name) => ({ name }));
}

export async function GET(
  _request: Request,
  context: { params: Promise<{ name: string }> },
) {
  const { name } = await context.params;
  const entry = ABIS[name];

  if (!entry) {
    return NextResponse.json(
      {
        error: 'abi_not_found',
        message: `No ABI is published at this endpoint. Available: ${Object.keys(ABIS).join(', ')}.`,
      },
      { status: 404 },
    );
  }

  return NextResponse.json(
    {
      name: entry.name,
      abi: entry.abi,
    },
    {
      headers: {
        'cache-control': 'public, max-age=3600, s-maxage=3600',
        'content-type': 'application/json; charset=utf-8',
      },
    },
  );
}
