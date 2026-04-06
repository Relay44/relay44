'use client';

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { ContextGraphResult, NarrativeSnapshot } from './types';

const API_BASE = process.env.NEXT_PUBLIC_CONTEXT_GRAPH_URL || '/api/context-graph';

interface AnalyzeInput {
  marketUrl?: string;
  conditionId?: string;
  slug?: string;
  depth?: 'quick' | 'full';
}

async function analyzeMarket(input: AnalyzeInput): Promise<ContextGraphResult> {
  const res = await fetch(`${API_BASE}/analyze`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  });

  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    throw new Error(data.error || `HTTP ${res.status}`);
  }

  return res.json();
}

async function fetchNarrative(conditionId: string): Promise<NarrativeSnapshot[]> {
  const res = await fetch(`${API_BASE}/narrative/${encodeURIComponent(conditionId)}`);
  if (!res.ok) return [];
  const data = await res.json();
  return data.timeline || [];
}

export function useContextGraphAnalysis() {
  const queryClient = useQueryClient();

  const mutation = useMutation({
    mutationFn: analyzeMarket,
    onSuccess: (data) => {
      if (data.conditionId) {
        queryClient.setQueryData(['context-graph', 'analysis', data.conditionId], data);
      }
    },
  });

  return {
    result: mutation.data ?? null,
    loading: mutation.isPending,
    error: mutation.error?.message ?? null,
    analyze: mutation.mutateAsync,
    reset: mutation.reset,
  };
}

export function useNarrativeTimeline(conditionId: string | null) {
  return useQuery({
    queryKey: ['context-graph', 'narrative', conditionId],
    queryFn: () => fetchNarrative(conditionId!),
    enabled: !!conditionId,
    staleTime: 60_000,
    placeholderData: (prev) => prev,
  });
}
