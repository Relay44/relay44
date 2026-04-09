import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';
import type { ReferralStats } from '@/lib/api';

export function useReferralCode() {
  return useQuery<{ code: string | null }>({
    queryKey: ['referral-code'],
    queryFn: () => api.getReferralCode(),
    staleTime: 60_000,
  });
}

export function useReferralStats() {
  return useQuery<ReferralStats>({
    queryKey: ['referral-stats'],
    queryFn: () => api.getReferralStats(),
    staleTime: 30_000,
  });
}

export function useGenerateReferralCode() {
  const queryClient = useQueryClient();

  return useMutation<{ code: string; created: boolean }, Error>({
    mutationFn: () => api.generateReferralCode(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['referral-code'] });
      queryClient.invalidateQueries({ queryKey: ['referral-stats'] });
    },
  });
}

export function useApplyReferralCode() {
  const queryClient = useQueryClient();

  return useMutation<{ ok: boolean; referrer: string }, Error, string>({
    mutationFn: (code: string) => api.applyReferralCode(code),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['referral-stats'] });
    },
  });
}
