import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { assertWritesEnabled } from '@/lib/runtimeMode';
import type {
  SignalProvider,
  SignalProviderFilters,
  SignalEmission,
  CreateSignalProviderRequest,
} from '@/types';

export function useSignalProviders(filters?: SignalProviderFilters) {
  return useQuery({
    queryKey: ['signal-providers', filters],
    queryFn: async (): Promise<{ providers: SignalProvider[] }> =>
      api.getSignalProviders(filters),
    staleTime: 15_000,
    refetchInterval: 15_000,
  });
}

export function useSignalProviderEmissions(marketSlug: string | undefined) {
  return useQuery({
    queryKey: ['signal-emissions', marketSlug],
    enabled: !!marketSlug,
    queryFn: async (): Promise<{ signals: SignalEmission[] }> =>
      api.getSignalProviderEmissions(marketSlug!),
    staleTime: 30_000,
    refetchInterval: 30_000,
  });
}

export function useCreateSignalProvider() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: CreateSignalProviderRequest) => {
      assertWritesEnabled('Signal provider creation');
      return api.createSignalProvider(data);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['signal-providers'] });
    },
  });
}
