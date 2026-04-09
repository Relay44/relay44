import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';
import type {
  DistributionMarket,
  DistributionPosition,
  DistributionQuote,
  CurvePoint,
  CurveSnapshot,
} from '@/types/distribution';

interface DistributionMarketFilters {
  status?: string;
  category?: string;
  limit?: number;
  offset?: number;
}

export function useDistributionMarkets(filters?: DistributionMarketFilters) {
  return useQuery({
    queryKey: ['distribution-markets', filters],
    queryFn: async (): Promise<{
      data: DistributionMarket[];
      total: number;
      hasMore: boolean;
    }> => {
      return api.getDistributionMarkets(filters);
    },
    staleTime: 15000,
    refetchOnWindowFocus: false,
  });
}

export function useDistributionMarket(id: string) {
  return useQuery({
    queryKey: ['distribution-market', id],
    queryFn: async (): Promise<DistributionMarket> => {
      return api.getDistributionMarket(id);
    },
    enabled: !!id,
    staleTime: 30000,
  });
}

export function useDistributionQuote(
  marketId: string,
  mu: number | null,
  sigma: number | null,
  size: number | null
) {
  return useQuery({
    queryKey: ['distribution-quote', marketId, mu, sigma, size],
    queryFn: async (): Promise<DistributionQuote> => {
      return api.getDistributionQuote(marketId, mu!, sigma!, size!);
    },
    enabled: !!marketId && mu !== null && sigma !== null && size !== null && size > 0,
    staleTime: 5000,
    refetchOnWindowFocus: false,
  });
}

export function useDistributionCurve(
  marketId: string,
  proposalMu?: number,
  proposalSigma?: number
) {
  return useQuery({
    queryKey: ['distribution-curve', marketId, proposalMu, proposalSigma],
    queryFn: async (): Promise<CurvePoint[]> => {
      return api.getDistributionCurve(marketId, proposalMu, proposalSigma);
    },
    enabled: !!marketId,
    staleTime: 10000,
  });
}

export function useDistributionTrade() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      marketId,
      mu,
      sigma,
      size,
    }: {
      marketId: string;
      mu: number;
      sigma: number;
      size: number;
    }) => {
      return api.executeDistributionTrade(marketId, { mu, sigma, size });
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['distribution-market', variables.marketId] });
      queryClient.invalidateQueries({ queryKey: ['distribution-markets'] });
      queryClient.invalidateQueries({ queryKey: ['distribution-curve', variables.marketId] });
      queryClient.invalidateQueries({ queryKey: ['distribution-positions'] });
    },
  });
}

export function useDistributionPositions() {
  return useQuery({
    queryKey: ['distribution-positions'],
    queryFn: async (): Promise<DistributionPosition[]> => {
      return api.getDistributionPositions();
    },
    staleTime: 15000,
  });
}

export function useCloseDistPosition() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (positionId: number) => {
      return api.closeDistributionPosition(positionId);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['distribution-positions'] });
      queryClient.invalidateQueries({ queryKey: ['distribution-markets'] });
    },
  });
}

export function useDistributionCurveHistory(
  marketId: string,
  params?: { limit?: number; since?: string }
) {
  return useQuery({
    queryKey: ['distribution-curve-history', marketId, params],
    queryFn: async (): Promise<CurveSnapshot[]> => {
      return api.getDistributionCurveHistory(marketId, params);
    },
    enabled: !!marketId,
    staleTime: 60000,
    refetchOnWindowFocus: false,
  });
}

export function useCreateDistributionMarket() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: {
      marketId: string;
      question: string;
      description?: string;
      category?: string;
      outcomeMin: number;
      outcomeMax: number;
      outcomeUnit?: string;
      liquidityParam: number;
      collateralToken: string;
      feeBps?: number;
      resolver?: string;
      useOracle?: boolean;
      oracleFeedId?: string;
      tradingEnd?: string;
      resolutionDeadline?: string;
    }) => {
      return api.createDistributionMarket(data);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['distribution-markets'] });
    },
  });
}

export function useResolveDistributionMarket() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ marketId, value }: { marketId: string; value: number }) => {
      return api.resolveDistributionMarket(marketId, value);
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['distribution-market', variables.marketId] });
      queryClient.invalidateQueries({ queryKey: ['distribution-markets'] });
      queryClient.invalidateQueries({ queryKey: ['distribution-positions'] });
    },
  });
}

export function useClaimDistPayout() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (positionId: number) => {
      return api.claimDistributionPayout(positionId);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['distribution-positions'] });
    },
  });
}
