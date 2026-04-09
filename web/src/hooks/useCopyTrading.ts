import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { assertWritesEnabled } from '@/lib/runtimeMode';
import type { CopyTradingSubscription } from '@/types';

const COPY_SUBS_KEY = ['copy-trading', 'subscriptions'];
const COPY_SUBSCRIBERS_KEY = ['copy-trading', 'subscribers'];

export function useCopySubscriptions(enabled = true) {
  return useQuery({
    queryKey: COPY_SUBS_KEY,
    queryFn: async () => api.getCopySubscriptions(),
    enabled,
    staleTime: 15_000,
    refetchInterval: 30_000,
  });
}

export function useCopySubscriberCount(enabled = true) {
  return useQuery({
    queryKey: COPY_SUBSCRIBERS_KEY,
    queryFn: async () => api.getCopySubscriberCount(),
    enabled,
    staleTime: 30_000,
  });
}

export function useCopySubscriptionHistory(subscriptionId: string, enabled = true) {
  return useQuery({
    queryKey: ['copy-trading', 'history', subscriptionId],
    queryFn: async () => api.getCopySubscriptionHistory(subscriptionId),
    enabled: enabled && !!subscriptionId,
    staleTime: 30_000,
  });
}

export function useStartCopyTrading() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: {
      targetWallet: string;
      allocationUsdc?: number;
      maxPositionUsdc?: number;
    }) => {
      assertWritesEnabled('Copy trading');
      return api.startCopyTrading(data.targetWallet, {
        allocationUsdc: data.allocationUsdc,
        maxPositionUsdc: data.maxPositionUsdc,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: COPY_SUBS_KEY });
    },
  });
}

export function useStopCopyTrading() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (subscriptionId: string) => {
      assertWritesEnabled('Copy trading');
      return api.stopCopyTrading(subscriptionId);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: COPY_SUBS_KEY });
      queryClient.invalidateQueries({ queryKey: ['copy-trading', 'history'] });
    },
  });
}

export function useUpdateCopySubscription() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: {
      subscriptionId: string;
      active?: boolean;
      allocationUsdc?: number;
      maxPositionUsdc?: number;
    }) => {
      assertWritesEnabled('Copy trading');
      const { subscriptionId, ...update } = data;
      return api.updateCopySubscription(subscriptionId, update);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: COPY_SUBS_KEY });
      queryClient.invalidateQueries({ queryKey: ['copy-trading', 'history'] });
    },
  });
}

/** Check if the current user is already copying a given wallet */
export function useCopyStatus(targetWallet: string, subscriptions?: CopyTradingSubscription[]) {
  if (!subscriptions || !targetWallet) return { isCopying: false, subscription: undefined };
  const target = targetWallet.toLowerCase();
  const match = subscriptions.find(
    (s) => s.targetWallet.toLowerCase() === target && s.active,
  );
  return { isCopying: !!match, subscription: match };
}
