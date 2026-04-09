import { useQuery } from '@tanstack/react-query';

import { api } from '@/lib/api';
import type { ScannedOpportunity, CalibrationBucket, ScanRun } from '@/types';

export function useScannerOpportunities(params?: {
  opportunityType?: string;
  category?: string;
  minScore?: number;
  limit?: number;
  enabled?: boolean;
}) {
  const enabled = params?.enabled ?? true;
  return useQuery({
    queryKey: ['scanner-opportunities', params],
    enabled,
    queryFn: async (): Promise<{ opportunities: ScannedOpportunity[]; count: number }> =>
      api.getScannerOpportunities({
        opportunityType: params?.opportunityType,
        category: params?.category,
        minScore: params?.minScore,
        limit: params?.limit,
      }),
    staleTime: 60_000,
    refetchInterval: enabled ? 60_000 : false,
    refetchOnWindowFocus: false,
    retry: 1,
  });
}

export function useScannerCalibration(enabled = true) {
  return useQuery({
    queryKey: ['scanner-calibration'],
    enabled,
    queryFn: async (): Promise<{ calibrationBuckets: CalibrationBucket[]; count: number }> =>
      api.getScannerCalibration(),
    staleTime: 300_000,
    refetchOnWindowFocus: false,
    retry: 1,
  });
}

export function useScannerRuns(enabled = true) {
  return useQuery({
    queryKey: ['scanner-runs'],
    enabled,
    queryFn: async (): Promise<{ runs: ScanRun[]; count: number }> =>
      api.getScannerRuns(),
    staleTime: 60_000,
    refetchInterval: enabled ? 60_000 : false,
    refetchOnWindowFocus: false,
    retry: 1,
  });
}
