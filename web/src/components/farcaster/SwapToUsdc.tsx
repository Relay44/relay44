'use client';

import { FC } from 'react';
import { ArrowRightLeft } from 'lucide-react';
import { swapToken } from '@/lib/farcaster';

const USDC_BASE_CAIP19 = 'eip155:8453/erc20:0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913';

interface SwapToUsdcProps {
  className?: string;
}

export const SwapToUsdc: FC<SwapToUsdcProps> = ({ className }) => {
  const handleSwap = async () => {
    try {
      await swapToken(USDC_BASE_CAIP19);
    } catch (err) {
      console.error('Swap failed:', err);
    }
  };

  return (
    <button
      onClick={handleSwap}
      className={`inline-flex items-center gap-1.5 rounded-md border border-accent/30 bg-accent/10 px-3 py-1.5 text-xs font-medium text-accent hover:bg-accent/20 transition-colors ${className ?? ''}`}
    >
      <ArrowRightLeft className="h-3.5 w-3.5" />
      Swap to USDC
    </button>
  );
};
