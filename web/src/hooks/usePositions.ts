import { useMemo } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { waitForTransactionReceipt } from '@wagmi/core';
import { useConfig, usePublicClient, useWalletClient } from 'wagmi';

import { api } from '@/lib/api';
import { ORDER_BOOK_ABI, ORDER_BOOK_ADDRESS, assertContractAddress } from '@/lib/contracts';
import { assertWritesEnabled } from '@/lib/runtimeMode';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import type { PaginatedResponse, Position } from '@/types';

function toUnits(value: bigint): number {
  return Number(value) / 1_000_000;
}

export function usePositions() {
  const publicClient = usePublicClient();
  const baseWallet = useBaseWallet();

  const orderBookAddress = useMemo(() => {
    try {
      return assertContractAddress(ORDER_BOOK_ADDRESS, 'NEXT_PUBLIC_ORDER_BOOK_ADDRESS');
    } catch {
      return null;
    }
  }, []);

  return useQuery({
    queryKey: ['positions', baseWallet.address, orderBookAddress],
    enabled: !!publicClient && !!baseWallet.address && !!orderBookAddress,
    refetchInterval: 15000,
    queryFn: async (): Promise<PaginatedResponse<Position>> => {
      if (!publicClient || !baseWallet.address) {
        return { data: [], total: 0, limit: 0, offset: 0, hasMore: false };
      }

      if (!orderBookAddress) {
        return { data: [], total: 0, limit: 200, offset: 0, hasMore: false };
      }

      const markets = await api.getBaseMarkets({ limit: 200, offset: 0, source: 'internal' });
      if (markets.data.length === 0) {
        return { data: [], total: 0, limit: 200, offset: 0, hasMore: false };
      }

      const now = new Date().toISOString();
      const positions: Position[] = [];
      for (let idx = 0; idx < markets.data.length; idx += 1) {
        const market = markets.data[idx];
        const marketId = BigInt(market.id);
        let positionRead: readonly [bigint, bigint, boolean];
        let claimableRaw: bigint;
        try {
          positionRead = await publicClient.readContract({
            address: orderBookAddress,
            abi: ORDER_BOOK_ABI,
            functionName: 'positions',
            args: [marketId, baseWallet.address as `0x${string}`],
          });
          claimableRaw = await publicClient.readContract({
            address: orderBookAddress,
            abi: ORDER_BOOK_ABI,
            functionName: 'claimable',
            args: [marketId, baseWallet.address as `0x${string}`],
          });
        } catch {
          continue;
        }

        const [yesSharesRaw, noSharesRaw] = positionRead;
        const yesBalance = toUnits(yesSharesRaw);
        const noBalance = toUnits(noSharesRaw);
        if (yesBalance === 0 && noBalance === 0) continue;

        const claimable = toUnits(claimableRaw);
        const totalDeposited = yesBalance + noBalance;
        const currentYesPrice = market.yesPrice || 0.5;
        const currentNoPrice = market.noPrice || 0.5;
        const markValue = yesBalance * currentYesPrice + noBalance * currentNoPrice;
        const unrealizedPnl = claimable > 0 ? claimable - totalDeposited : markValue - totalDeposited;

        positions.push({
          marketId: market.id,
          marketQuestion: market.question,
          owner: baseWallet.address,
          yesBalance,
          noBalance,
          claimable,
          avgYesCost: currentYesPrice,
          avgNoCost: currentNoPrice,
          currentYesPrice,
          currentNoPrice,
          unrealizedPnl,
          realizedPnl: 0,
          totalDeposited,
          totalWithdrawn: 0,
          openOrderCount: 0,
          totalTrades: 0,
          createdAt: now,
        });
      }

      return {
        data: positions,
        total: positions.length,
        limit: positions.length,
        offset: 0,
        hasMore: false,
      };
    },
  });
}

export function usePosition(marketId: string) {
  const positions = usePositions();
  return useQuery({
    queryKey: ['position', marketId, positions.data?.data.length || 0],
    enabled: !!marketId,
    queryFn: async () => {
      const position = positions.data?.data.find((candidate) => candidate.marketId === marketId);
      if (!position) throw new Error('Position not found');
      return position;
    },
  });
}

export function useClaimWinnings() {
  const queryClient = useQueryClient();
  const baseWallet = useBaseWallet();
  const config = useConfig();
  const { data: walletClient } = useWalletClient();

  return useMutation({
    mutationFn: async (marketId: string) => {
      assertWritesEnabled('Claiming winnings');

      if (!baseWallet.address || !baseWallet.isConnected) {
        throw new Error('Connect your wallet before claiming');
      }
      if (!walletClient) {
        throw new Error('Wallet client unavailable');
      }

      const parsedMarketId = Number(marketId);
      if (!Number.isInteger(parsedMarketId) || parsedMarketId < 1) {
        throw new Error('Invalid market id');
      }

      await baseWallet.ensureBaseChain();
      const prepared = await api.prepareBaseClaim({
        from: baseWallet.address,
        marketId: parsedMarketId,
      });
      const hash = await walletClient.sendTransaction({
        account: baseWallet.address as `0x${string}`,
        to: prepared.to as `0x${string}`,
        data: prepared.data,
        value: BigInt(prepared.value),
      });

      const receipt = await waitForTransactionReceipt(config, { hash });
      return {
        amount: 0,
        txSignature: receipt.transactionHash,
      };
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['positions'] });
      queryClient.invalidateQueries({ queryKey: ['orders'] });
    },
  });
}
