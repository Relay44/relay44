'use client';

import { useQuery } from '@tanstack/react-query';

import { api } from '@/lib/api';

interface QueryOptions {
  enabled?: boolean;
}

export function useCreatorEconomicsOverview(options?: QueryOptions) {
  return useQuery({
    queryKey: ['creator-economics', 'overview'],
    enabled: options?.enabled ?? true,
    staleTime: 30_000,
    retry: 1,
    queryFn: async () => api.getCreatorEconomicsOverview(),
  });
}

export function useCreatorEconomicsMarkets(
  options?: QueryOptions,
) {
  return useQuery({
    queryKey: ['creator-economics', 'markets'],
    enabled: options?.enabled ?? true,
    staleTime: 30_000,
    retry: 1,
    placeholderData: (previousData) => previousData,
    queryFn: async () => api.getCreatorEconomicsMarkets(),
  });
}

export function useCreatorEconomicsMarket(
  marketId: string | null,
  window: '7d' | '30d' | '90d' = '30d',
  options?: QueryOptions,
) {
  return useQuery({
    queryKey: ['creator-economics', 'market', marketId, window],
    enabled: (options?.enabled ?? true) && !!marketId,
    staleTime: 30_000,
    retry: 1,
    queryFn: async () => {
      if (!marketId) {
        throw new Error('Missing market id');
      }
      return api.getCreatorEconomicsMarket(marketId, window);
    },
  });
}
