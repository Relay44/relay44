'use client';

import { useState } from 'react';
import { Button, useToast } from '@/components/ui';
import {
  useCopySubscriptions,
  useStartCopyTrading,
  useStopCopyTrading,
  useCopyStatus,
} from '@/hooks';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { useSessionState } from '@/hooks';

interface CopyTraderButtonProps {
  wallet: string;
}

export function CopyTraderButton({ wallet }: CopyTraderButtonProps) {
  const { address, isConnected } = useBaseWallet();
  const { hasSession } = useSessionState();
  const { data: subsData } = useCopySubscriptions(isConnected && hasSession);
  const subscriptions = subsData?.data || [];
  const { isCopying, subscription } = useCopyStatus(wallet, subscriptions);

  const startCopy = useStartCopyTrading();
  const stopCopy = useStopCopyTrading();
  const { addToast } = useToast();
  const [showConfig, setShowConfig] = useState(false);
  const [allocation, setAllocation] = useState(50);
  const [maxPosition, setMaxPosition] = useState(20);

  // Don't show button on own profile
  if (address && address.toLowerCase() === wallet.toLowerCase()) {
    return null;
  }

  if (!isConnected || !hasSession) {
    return null;
  }

  const isPending = startCopy.isPending || stopCopy.isPending;

  const handleToggle = async () => {
    try {
      if (isCopying && subscription) {
        await stopCopy.mutateAsync(subscription.id);
        setShowConfig(false);
      } else if (showConfig) {
        await startCopy.mutateAsync({
          targetWallet: wallet,
          allocationUsdc: allocation,
          maxPositionUsdc: maxPosition,
        });
        setShowConfig(false);
      } else {
        setShowConfig(true);
      }
    } catch (err) {
      addToast((err as Error)?.message ?? 'Copy trading action failed', 'error');
    }
  };

  return (
    <div className="flex flex-col gap-2">
      {showConfig && !isCopying && (
        <div className="flex flex-col gap-2 p-3 bg-bg-secondary border border-border">
          <label className="text-sm text-text-secondary">
            Allocation (USDC)
            <input
              type="number"
              min={1}
              max={100000}
              value={allocation}
              onChange={(e) => setAllocation(Number(e.target.value))}
              className="mt-1 w-full px-2 py-1 bg-bg-primary border border-border text-text-primary text-sm"
            />
          </label>
          <label className="text-sm text-text-secondary">
            Max Position (USDC)
            <input
              type="number"
              min={1}
              max={50000}
              value={maxPosition}
              onChange={(e) => setMaxPosition(Number(e.target.value))}
              className="mt-1 w-full px-2 py-1 bg-bg-primary border border-border text-text-primary text-sm"
            />
          </label>
        </div>
      )}
      <Button
        onClick={handleToggle}
        disabled={isPending}
        variant={isCopying ? 'outline' : 'primary'}
        size="sm"
      >
        {isPending
          ? 'Processing...'
          : isCopying
            ? 'Stop Copying'
            : showConfig
              ? 'Confirm Copy'
              : 'Copy Trader'}
      </Button>
      {showConfig && !isCopying && (
        <button
          type="button"
          onClick={() => setShowConfig(false)}
          className="text-xs text-text-secondary hover:text-text-primary"
        >
          Cancel
        </button>
      )}
      {(startCopy.error || stopCopy.error) && (
        <p className="text-xs text-ask">
          {(startCopy.error || stopCopy.error)?.message || 'An error occurred'}
        </p>
      )}
    </div>
  );
}
