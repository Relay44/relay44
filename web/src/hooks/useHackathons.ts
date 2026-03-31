import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';
import type {
  HackathonLeaderboard,
  HackathonRegistration,
  HackathonSnapshot,
} from '@/types';

export function useHackathons(params?: { status?: string }) {
  return useQuery({
    queryKey: ['hackathons', params?.status],
    queryFn: () => api.getHackathons(params),
    refetchInterval: 30_000,
    refetchIntervalInBackground: false,
    staleTime: 10_000,
  });
}

export function useHackathon(id: string | undefined) {
  return useQuery({
    queryKey: ['hackathon', id],
    enabled: !!id,
    queryFn: () => api.getHackathon(id!),
    refetchInterval: 30_000,
    refetchIntervalInBackground: false,
    staleTime: 10_000,
  });
}

export function useHackathonLeaderboard(id: string | undefined) {
  return useQuery<HackathonLeaderboard>({
    queryKey: ['hackathon-leaderboard', id],
    enabled: !!id,
    queryFn: () => api.getHackathonLeaderboard(id!, { limit: 100 }),
    refetchInterval: 30_000,
    refetchIntervalInBackground: false,
    staleTime: 10_000,
  });
}

export function useHackathonSnapshots(
  id: string | undefined,
  walletAddress?: string,
) {
  return useQuery<{ snapshots: HackathonSnapshot[] }>({
    queryKey: ['hackathon-snapshots', id, walletAddress],
    enabled: !!id,
    queryFn: () => api.getHackathonSnapshots(id!, { walletAddress, limit: 200 }),
    refetchInterval: 30_000,
    refetchIntervalInBackground: false,
    staleTime: 10_000,
  });
}

export function useHackathonRegistrations(id: string | undefined) {
  return useQuery<{ registrations: HackathonRegistration[]; total: number }>({
    queryKey: ['hackathon-registrations', id],
    enabled: !!id,
    queryFn: () => api.getHackathonRegistrations(id!),
    refetchInterval: 30_000,
    refetchIntervalInBackground: false,
    staleTime: 10_000,
  });
}

export function useRegisterForHackathon() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: { hackathonId: string; identityId?: string }) =>
      api.registerForHackathon(params.hackathonId, {
        identityId: params.identityId,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['hackathon'] });
      queryClient.invalidateQueries({ queryKey: ['hackathon-registrations'] });
    },
  });
}

export function useLinkAgentToHackathon() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: { hackathonId: string; agentId: string }) =>
      api.linkAgentToHackathon(params.hackathonId, params.agentId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['hackathon'] });
    },
  });
}
