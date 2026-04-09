import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { assertWritesEnabled } from '@/lib/runtimeMode';
import type {
  AgentTemplate,
  ManagedAgent,
  ManagedAgentTrade,
  DeployAgentRequest,
  DeployAgentResponse,
} from '@/types';

export function useAgentTemplates() {
  return useQuery<AgentTemplate[]>({
    queryKey: ['agent-templates'],
    queryFn: async () => {
      const res = await api.listAgentTemplates();
      return res.templates;
    },
    staleTime: 60_000,
  });
}

export function useAgentTemplate(templateId: string | undefined) {
  const { data: templates, ...rest } = useAgentTemplates();
  const template = templates?.find((t) => t.id === templateId);
  return { data: template, templates, ...rest };
}

export function useManagedAgents(filters?: { status?: string; limit?: number }) {
  return useQuery<ManagedAgent[]>({
    queryKey: ['managed-agents', filters],
    queryFn: async () => {
      const res = await api.listManagedAgents(filters);
      return res.agents;
    },
    refetchInterval: 15_000,
  });
}

export function useManagedAgentTrades(agentId: string | undefined) {
  return useQuery<ManagedAgentTrade[]>({
    queryKey: ['managed-agent-trades', agentId],
    enabled: !!agentId,
    queryFn: async () => {
      const res = await api.getManagedAgentTrades(agentId!);
      return res.trades;
    },
    staleTime: 30_000,
  });
}

export function useDeployManagedAgent() {
  const queryClient = useQueryClient();

  return useMutation<DeployAgentResponse, Error, DeployAgentRequest>({
    mutationFn: async (data) => {
      assertWritesEnabled('Deploy managed agent');
      return api.deployManagedAgent(data);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['managed-agents'] });
    },
  });
}

export function useUpdateManagedAgent() {
  const queryClient = useQueryClient();

  return useMutation<
    { ok: boolean; status: string },
    Error,
    { agentId: string; status: string }
  >({
    mutationFn: async ({ agentId, status }) => {
      assertWritesEnabled('Update managed agent');
      return api.updateManagedAgent(agentId, { status });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['managed-agents'] });
    },
  });
}
