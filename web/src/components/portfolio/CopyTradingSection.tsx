'use client';

import Link from 'next/link';
import { Card } from '@/components/ui';
import { Button } from '@/components/ui';
import {
  useCopySubscriptions,
  useCopySubscriberCount,
  useStopCopyTrading,
  useUpdateCopySubscription,
} from '@/hooks';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { useSessionState } from '@/hooks';
import { formatCurrency } from '@/lib/utils';

function truncateAddress(address: string): string {
  if (!address || address.length < 10) return address || '';
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

export function CopyTradingSection() {
  const { isConnected } = useBaseWallet();
  const { hasSession, sessionRestored } = useSessionState();
  const enabled = isConnected && hasSession && sessionRestored;

  const { data: subsData, isLoading: subsLoading } = useCopySubscriptions(enabled);
  const { data: subCountData } = useCopySubscriberCount(enabled);
  const stopCopy = useStopCopyTrading();
  const updateSub = useUpdateCopySubscription();

  const subscriptions = subsData?.data || [];
  const subscriberCount = subCountData?.copySubscriberCount || 0;

  if (!enabled) return null;
  if (subsLoading) {
    return (
      <section className="mb-8">
        <h2 className="text-lg font-semibold mb-4">Copy Trading</h2>
        <Card>
          <div className="flex items-center justify-center h-24">
            <div className="animate-pulse text-text-secondary">Loading...</div>
          </div>
        </Card>
      </section>
    );
  }

  if (subscriptions.length === 0 && subscriberCount === 0) return null;

  return (
    <section className="mb-8">
      <h2 className="text-lg font-semibold mb-4">Copy Trading</h2>

      {subscriberCount > 0 && (
        <Card className="mb-4">
          <div className="flex items-center gap-3">
            <div className="text-text-secondary text-sm">Traders copying you</div>
            <div className="text-xl font-semibold">{subscriberCount}</div>
          </div>
        </Card>
      )}

      {subscriptions.length > 0 && (
        <div className="grid gap-3">
          {subscriptions.map((sub) => (
            <Card key={sub.id}>
              <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                <div className="flex-1 min-w-0">
                  <Link
                    href={`/profile/${sub.targetWallet}`}
                    className="text-sm font-medium text-text-primary hover:text-accent transition-colors"
                  >
                    Copying {truncateAddress(sub.targetWallet)}
                  </Link>
                  <div className="flex flex-wrap gap-4 mt-1 text-xs text-text-secondary">
                    <span>Allocation: {formatCurrency(sub.allocationUsdc)}</span>
                    <span>Max position: {formatCurrency(sub.maxPositionUsdc)}</span>
                    <span>Since {new Date(sub.createdAt).toLocaleDateString()}</span>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={updateSub.isPending}
                    onClick={() =>
                      updateSub.mutate({
                        subscriptionId: sub.id,
                        active: !sub.active,
                      })
                    }
                  >
                    {sub.active ? 'Pause' : 'Resume'}
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={stopCopy.isPending}
                    onClick={() => stopCopy.mutate(sub.id)}
                  >
                    Stop
                  </Button>
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}
    </section>
  );
}
