'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';

export function useSessionState() {
  const sessionQuery = useQuery({
    queryKey: ['session-state'],
    queryFn: async () => {
      if (api.isAuthenticated()) {
        return true;
      }

      return api.restoreSession();
    },
    staleTime: 5 * 60_000,
    retry: 0,
  });

  const accessTokenPresent = api.isAuthenticated();

  return {
    hasSession:
      typeof sessionQuery.data === 'boolean'
        ? sessionQuery.data || accessTokenPresent
        : accessTokenPresent,
    sessionRestored: sessionQuery.status !== 'pending',
    isLoadingSession: sessionQuery.isLoading,
  };
}
