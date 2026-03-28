'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import {
  capabilitiesAreReadOnly,
  getRuntimeCapabilities,
  isReadOnlyMode,
  readOnlyPreviewEnabled,
  setRuntimeCapabilities,
} from '@/lib/runtimeMode';

export function useRuntimeMode() {
  const initialCapabilities = getRuntimeCapabilities() ?? undefined;
  const capabilitiesQuery = useQuery({
    queryKey: ['web4-capabilities'],
    queryFn: async () => {
      const capabilities = await api.getWeb4Capabilities();
      setRuntimeCapabilities(capabilities);
      return capabilities;
    },
    enabled: !readOnlyPreviewEnabled,
    initialData: initialCapabilities,
    staleTime: 60_000,
    retry: 1,
  });
  const capabilities = capabilitiesQuery.data ?? null;
  const capabilitiesUnknown =
    !readOnlyPreviewEnabled &&
    !capabilities &&
    (capabilitiesQuery.isLoading || capabilitiesQuery.isError);

  return {
    capabilities,
    isLoadingCapabilities: capabilitiesQuery.isLoading,
    readOnly: readOnlyPreviewEnabled || capabilitiesUnknown || isReadOnlyMode(capabilities),
    forcedReadOnly: readOnlyPreviewEnabled,
    runtimeLockedReadOnly: capabilitiesAreReadOnly(capabilities),
  };
}
