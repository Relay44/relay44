import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { waitForTransactionReceipt } from '@wagmi/core';
import { useConfig, useWalletClient } from 'wagmi';
import { api, ApiError } from '@/lib/api';
import { assertWritesEnabled } from '@/lib/runtimeMode';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import type { MarketFilters, Outcome, PaginatedResponse, Market } from '@/types';

interface UseMarketsOptions {
  initialData?: PaginatedResponse<Market>;
  enabled?: boolean;
}

export function useMarkets(filters?: MarketFilters, options?: UseMarketsOptions) {
  return useQuery({
    queryKey: ['markets', filters, 'base-api'],
    enabled: options?.enabled ?? true,
    initialData: options?.initialData,
    placeholderData: (previousData) => previousData,
    staleTime: options?.initialData ? 15000 : 10000,
    refetchOnMount: options?.initialData ? false : true,
    refetchOnWindowFocus: false,
    queryFn: async (): Promise<PaginatedResponse<Market>> => {
      const response = await api.getBaseMarkets({
        limit: filters?.limit || 50,
        offset: filters?.offset || 0,
        source: filters?.source || 'all',
        tradable: filters?.tradable || 'all',
        includeLowLiquidity: filters?.includeLowLiquidity,
      });

      let data = [...response.data];
      const requestedCategory = filters?.category?.toLowerCase();
      if (requestedCategory) {
        data = data.filter(
          (market) => market.category.toLowerCase() === requestedCategory
        );
      }

      if (filters?.sort === 'volume') {
        data.sort((a, b) => b.volume24h - a.volume24h);
      } else if (filters?.sort === 'newest') {
        data.sort(
          (a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
        );
      } else if (filters?.sort === 'ending') {
        data.sort((a, b) => new Date(a.tradingEnd).getTime() - new Date(b.tradingEnd).getTime());
      }

      const filteredTotal = data.length;
      const offset = filters?.offset || 0;
      const limit = filters?.limit || 50;
      const paged = data.slice(offset, offset + limit);

      return {
        ...response,
        data: paged,
        total: filteredTotal,
        limit,
        offset,
        hasMore: offset + limit < filteredTotal,
      };
    },
    retry: 1,
  });
}

export function useMarket(id: string) {
  return useQuery({
    queryKey: ['market', id, 'base-api'],
    queryFn: async () => api.getBaseMarket(id),
    enabled: !!id,
    retry: 1,
    staleTime: 30000,
  });
}

export function useOrderBook(marketId: string, outcome: Outcome) {
  return useQuery({
    queryKey: ['orderbook', marketId, outcome, 'base-api'],
    queryFn: async () => api.getBaseOrderBook(marketId, outcome),
    enabled: !!marketId,
    retry: (failureCount, error) => {
      if (error instanceof ApiError && error.status === 402) {
        return false;
      }
      return failureCount < 1;
    },
    refetchInterval: (query) => {
      const error = query.state.error;
      if (error instanceof ApiError && error.status === 402) {
        return false;
      }
      return 5000;
    },
  });
}

export function useTrades(
  marketId: string,
  params?: { outcome?: Outcome; limit?: number }
) {
  return useQuery({
    queryKey: ['trades', marketId, params, 'base-api'],
    queryFn: async () => api.getBaseTrades(marketId, params),
    enabled: !!marketId,
  });
}

export function useResolveMarket() {
  const queryClient = useQueryClient();
  const baseWallet = useBaseWallet();
  const config = useConfig();
  const { data: walletClient } = useWalletClient();

  return useMutation({
    mutationFn: async ({
      marketId,
      outcome,
    }: {
      marketId: string;
      outcome: Outcome;
    }) => {
      assertWritesEnabled('Resolving markets');

      if (!baseWallet.address || !baseWallet.isConnected) {
        throw new Error('Connect your wallet before resolving a market');
      }
      if (!walletClient) {
        throw new Error('Wallet client unavailable');
      }

      const parsedMarketId = Number(marketId);
      if (!Number.isInteger(parsedMarketId) || parsedMarketId < 1) {
        throw new Error('Invalid market id');
      }

      await baseWallet.ensureBaseChain();
      const prepared = await api.prepareBaseResolveMarket({
        from: baseWallet.address,
        marketId: parsedMarketId,
        outcome: outcome === 'yes',
      });
      const hash = await walletClient.sendTransaction({
        account: baseWallet.address as `0x${string}`,
        to: prepared.to as `0x${string}`,
        data: prepared.data,
        value: BigInt(prepared.value),
      });

      const receipt = await waitForTransactionReceipt(config, { hash });
      return {
        outcome,
        txSignature: receipt.transactionHash,
      };
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['market', variables.marketId] });
      queryClient.invalidateQueries({ queryKey: ['markets'] });
      queryClient.invalidateQueries({ queryKey: ['positions'] });
      queryClient.invalidateQueries({ queryKey: ['orders'] });
    },
  });
}
