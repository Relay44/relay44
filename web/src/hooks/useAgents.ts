import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { waitForTransactionReceipt } from '@wagmi/core';
import { useConfig, useWalletClient } from 'wagmi';

import { api } from '@/lib/api';
import { assertWritesEnabled } from '@/lib/runtimeMode';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import type { Agent, AgentFilters, PaginatedResponse } from '@/types';

function toBaseUnits(value: number): bigint {
  return BigInt(Math.max(1, Math.round(value * 1_000_000)));
}

export function useAgents(filters?: AgentFilters) {
  return useQuery({
    queryKey: ['agents', filters],
    queryFn: async (): Promise<PaginatedResponse<Agent>> => api.getBaseAgents(filters),
    refetchInterval: 10_000,
  });
}

export function useAgent(agentId: string) {
  return useQuery({
    queryKey: ['agent', agentId],
    enabled: !!agentId,
    queryFn: async () => api.getBaseAgent(agentId),
  });
}

export function useCreateAgent() {
  const queryClient = useQueryClient();
  const baseWallet = useBaseWallet();
  const config = useConfig();
  const { data: walletClient } = useWalletClient();

  return useMutation({
    mutationFn: async (data: {
      marketId: string;
      isYes: boolean;
      priceBps: number;
      size: number;
      cadence: number;
      expiryWindow: number;
      strategy: string;
    }) => {
      assertWritesEnabled('Agent launch');

      if (!baseWallet.address || !baseWallet.isConnected) {
        throw new Error('Connect your wallet before launching an agent');
      }
      if (!walletClient) {
        throw new Error('Wallet client unavailable');
      }

      await baseWallet.ensureBaseChain();

      const marketId = Number(data.marketId);
      if (!Number.isInteger(marketId) || marketId < 1) {
        throw new Error('Invalid market id');
      }

      const priceBps = Math.max(1, Math.min(9_999, Math.round(data.priceBps)));
      const size = toBaseUnits(data.size);
      const cadence = Math.max(1, Math.round(data.cadence));
      const expiryWindow = Math.max(1, Math.round(data.expiryWindow));
      const strategy = data.strategy.trim();
      if (!strategy) {
        throw new Error('Strategy is required');
      }

      const prepared = await api.prepareBaseCreateAgent({
        from: baseWallet.address,
        marketId,
        isYes: data.isYes,
        priceBps,
        size: size.toString(),
        cadence,
        expiryWindow,
        strategy,
      });

      const hash = await walletClient.sendTransaction({
        account: baseWallet.address as `0x${string}`,
        to: prepared.to as `0x${string}`,
        data: prepared.data,
        value: BigInt(prepared.value),
      });
      await waitForTransactionReceipt(config, { hash });
      return { txSignature: hash };
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['agents'] });
    },
  });
}

export function useExecuteAgent() {
  const queryClient = useQueryClient();
  const baseWallet = useBaseWallet();
  const config = useConfig();
  const { data: walletClient } = useWalletClient();

  return useMutation({
    mutationFn: async (agentId: string) => {
      assertWritesEnabled('Agent execution');

      if (!baseWallet.address || !baseWallet.isConnected) {
        throw new Error('Connect your wallet before executing an agent');
      }
      if (!walletClient) {
        throw new Error('Wallet client unavailable');
      }

      await baseWallet.ensureBaseChain();

      const parsedAgentId = Number(agentId);
      if (!Number.isInteger(parsedAgentId) || parsedAgentId < 1) {
        throw new Error('Invalid agent id');
      }

      const prepared = await api.prepareBaseExecuteAgent({
        from: baseWallet.address,
        agentId: parsedAgentId,
      });

      const hash = await walletClient.sendTransaction({
        account: baseWallet.address as `0x${string}`,
        to: prepared.to as `0x${string}`,
        data: prepared.data,
        value: BigInt(prepared.value),
      });
      await waitForTransactionReceipt(config, { hash });
      return { txSignature: hash };
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['agents'] });
      queryClient.invalidateQueries({ queryKey: ['orders'] });
      queryClient.invalidateQueries({ queryKey: ['orderbook'] });
      queryClient.invalidateQueries({ queryKey: ['trades'] });
    },
  });
}

export function useExternalAgents(filters?: {
  provider?: 'limitless' | 'polymarket' | 'aerodrome';
  active?: boolean;
  limit?: number;
  offset?: number;
  enabled?: boolean;
}) {
  const enabled = filters?.enabled ?? true;
  const requestFilters = {
    provider: filters?.provider,
    active: filters?.active,
    limit: filters?.limit,
    offset: filters?.offset,
  };

  return useQuery({
    queryKey: ['external-agents', requestFilters, enabled],
    enabled,
    queryFn: async () => {
      const response = await api.listExternalAgents(requestFilters);
      return {
        data: response.agents,
        total: response.total,
        limit: response.limit,
        offset: response.offset,
        hasMore: response.offset + response.limit < response.total,
      };
    },
    refetchInterval: 10000,
  });
}

export function usePublicExternalAgents(filters?: {
  provider?: 'limitless' | 'polymarket' | 'aerodrome';
  active?: boolean;
  limit?: number;
  offset?: number;
  enabled?: boolean;
}) {
  const enabled = filters?.enabled ?? true;
  const requestFilters = {
    provider: filters?.provider,
    active: filters?.active,
    limit: filters?.limit,
    offset: filters?.offset,
  };

  return useQuery({
    queryKey: ['external-agents-public', requestFilters, enabled],
    enabled,
    queryFn: async () => {
      const response = await api.listPublicExternalAgents(requestFilters);
      return {
        data: response.agents,
        total: response.total,
        limit: response.limit,
        offset: response.offset,
        hasMore: response.offset + response.limit < response.total,
      };
    },
    refetchInterval: 10000,
  });
}

export function usePublicExternalAgentsPerformance(enabled = true) {
  return useQuery({
    queryKey: ['external-agents-public-performance', enabled],
    enabled,
    queryFn: async () => api.getPublicExternalAgentsPerformance(),
    refetchInterval: 15000,
  });
}

export function useCreateExternalAgent() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: Parameters<typeof api.createExternalAgent>[0]) => {
      assertWritesEnabled('External agent launch');
      return api.createExternalAgent(data);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['external-agents'] });
      queryClient.invalidateQueries({ queryKey: ['external-agents-public'] });
      queryClient.invalidateQueries({ queryKey: ['external-agents-public-performance'] });
    },
  });
}

export function useExecuteExternalAgent() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: { agentId: string; force?: boolean }) => {
      assertWritesEnabled('External agent execution');
      return api.executeExternalAgent(params.agentId, { force: params.force ?? false });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['external-agents'] });
      queryClient.invalidateQueries({ queryKey: ['external-agents-public'] });
      queryClient.invalidateQueries({ queryKey: ['external-agents-public-performance'] });
      queryClient.invalidateQueries({ queryKey: ['trades'] });
    },
  });
}
