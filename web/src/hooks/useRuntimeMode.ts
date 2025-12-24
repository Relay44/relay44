'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import {
  capabilitiesAreReadOnly,
  isReadOnlyMode,
  readOnlyPreviewEnabled,
  setRuntimeCapabilities,
} from '@/lib/runtimeMode';

export function useRuntimeMode() {
  const capabilitiesQuery = useQuery({
    queryKey: ['web4-capabilities'],
    queryFn: async () => {
      const capabilities = await api.getWeb4Capabilities();
      setRuntimeCapabilities(capabilities);
      return capabilities;
    },
    enabled: !readOnlyPreviewEnabled,
    staleTime: 60_000,
    retry: 1,
  });

  return {
    capabilities: capabilitiesQuery.data,
    isLoadingCapabilities: capabilitiesQuery.isLoading,
    readOnly: isReadOnlyMode(capabilitiesQuery.data),
    forcedReadOnly: readOnlyPreviewEnabled,
    runtimeLockedReadOnly: capabilitiesAreReadOnly(capabilitiesQuery.data),
  };
}
