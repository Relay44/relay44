import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { api } from '@/lib/api';
import type {
  DecisionCell,
  DecisionCellListItem,
  PaginatedResponse,
} from '@/types';
import type {
  CreateDecisionCellRequest,
  CreateDecisionNodeRequest,
  UpdateDecisionAutomationRequest,
  UpdateDecisionCellRequest,
  UpdateDecisionNodeRequest,
} from '@/lib/api';

export function useDecisionCells(filters?: {
  limit?: number;
  offset?: number;
  status?: string;
  enabled?: boolean;
}) {
  const enabled = filters?.enabled ?? true;
  return useQuery({
    queryKey: ['decision-cells', filters],
    enabled,
    queryFn: async (): Promise<PaginatedResponse<DecisionCellListItem>> =>
      api.listDecisionCells(filters),
    refetchInterval: enabled ? 30_000 : false,
  });
}

export function useDecisionCell(cellId: string, enabled = true) {
  return useQuery({
    queryKey: ['decision-cell', cellId],
    enabled: enabled && !!cellId,
    queryFn: async (): Promise<DecisionCell> => api.getDecisionCell(cellId),
    refetchInterval: enabled ? 15_000 : false,
  });
}

function invalidateDecisionQueries(queryClient: ReturnType<typeof useQueryClient>, cellId?: string) {
  queryClient.invalidateQueries({ queryKey: ['decision-cells'] });
  if (cellId) {
    queryClient.invalidateQueries({ queryKey: ['decision-cell', cellId] });
  }
}

export function useCreateDecisionCell() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (data: CreateDecisionCellRequest) => api.createDecisionCell(data),
    onSuccess: (cell) => {
      invalidateDecisionQueries(queryClient, cell.id);
    },
  });
}

export function useUpdateDecisionCell(cellId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (data: UpdateDecisionCellRequest) => api.updateDecisionCell(cellId, data),
    onSuccess: () => invalidateDecisionQueries(queryClient, cellId),
  });
}

export function useAddDecisionAction(cellId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (label: string) => api.addDecisionAction(cellId, label),
    onSuccess: () => invalidateDecisionQueries(queryClient, cellId),
  });
}

export function useAddDecisionNode(cellId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (data: CreateDecisionNodeRequest) => api.addDecisionNode(cellId, data),
    onSuccess: () => invalidateDecisionQueries(queryClient, cellId),
  });
}

export function useUpdateDecisionNode(cellId: string, nodeId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (data: UpdateDecisionNodeRequest) =>
      api.updateDecisionNode(cellId, nodeId, data),
    onSuccess: () => invalidateDecisionQueries(queryClient, cellId),
  });
}

export function useAttachDecisionMarket(cellId: string, nodeId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (data: { sourceType: 'internal_market' | 'external_market'; sourceRef: string }) =>
      api.attachDecisionMarket(cellId, nodeId, data),
    onSuccess: () => invalidateDecisionQueries(queryClient, cellId),
  });
}

export function useAttachDecisionAgent(cellId: string, nodeId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (data: {
      externalAgentId: string;
      triggerMode: 'on_recommendation_gain' | 'on_threshold_cross' | 'on_confidence_gain';
      active?: boolean;
    }) => api.attachDecisionAgent(cellId, nodeId, data),
    onSuccess: () => invalidateDecisionQueries(queryClient, cellId),
  });
}

export function useRecalculateDecisionCell(cellId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () => api.recalculateDecisionCell(cellId),
    onSuccess: () => invalidateDecisionQueries(queryClient, cellId),
  });
}

export function useUpdateDecisionAutomation(cellId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (data: UpdateDecisionAutomationRequest) =>
      api.updateDecisionAutomation(cellId, data),
    onSuccess: () => invalidateDecisionQueries(queryClient, cellId),
  });
}

export function useUpsertDecisionAlert(cellId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (data: {
      kind: string;
      threshold?: Record<string, unknown>;
      active?: boolean;
    }) => api.upsertDecisionAlert(cellId, data),
    onSuccess: () => invalidateDecisionQueries(queryClient, cellId),
  });
}

