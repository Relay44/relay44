import { useMemo } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { waitForTransactionReceipt } from '@wagmi/core';
import { parseEventLogs } from 'viem';
import { useConfig, usePublicClient, useWalletClient } from 'wagmi';

import { api } from '@/lib/api';
import { ORDER_BOOK_ABI, ORDER_BOOK_ADDRESS, ORDER_PLACED_EVENT_ABI, assertContractAddress } from '@/lib/contracts';
import { assertWritesEnabled } from '@/lib/runtimeMode';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import type { OrderFilters, PlaceOrderRequest, PlaceOrderResponse, PaginatedResponse, Order } from '@/types';

const ORDER_SCAN_LIMIT = 300;
const DEFAULT_EXPIRY_SECONDS = 24 * 60 * 60;

function toBaseUnits(value: number): bigint {
  return BigInt(Math.max(1, Math.round(value * 1_000_000)));
}

function orderStatus(
  canceled: boolean,
  remaining: bigint,
  size: bigint,
  expiry: bigint,
  nowSeconds: number
): Order['status'] {
  if (canceled) return 'cancelled';
  if (remaining === BigInt(0)) return 'filled';
  if (Number(expiry) <= nowSeconds) return 'expired';
  if (remaining < size) return 'partially_filled';
  return 'open';
}

export function useOrders(filters?: OrderFilters) {
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
    queryKey: ['orders', filters, baseWallet.address, orderBookAddress],
    enabled: !!publicClient && !!baseWallet.address && !!orderBookAddress,
    refetchInterval: 10000,
    queryFn: async (): Promise<PaginatedResponse<Order>> => {
      if (!publicClient || !baseWallet.address || !orderBookAddress) {
        return {
          data: [],
          total: 0,
          limit: filters?.limit || 50,
          offset: filters?.offset || 0,
          hasMore: false,
        };
      }

      const totalRaw = await publicClient.readContract({
        address: orderBookAddress,
        abi: ORDER_BOOK_ABI,
        functionName: 'orderCount',
      });

      const total = Number(totalRaw);
      if (total === 0) {
        return {
          data: [],
          total: 0,
          limit: filters?.limit || 50,
          offset: filters?.offset || 0,
          hasMore: false,
        };
      }

      const start = Math.max(1, total - ORDER_SCAN_LIMIT + 1);
      const orderIds = Array.from({ length: total - start + 1 }, (_, index) => BigInt(start + index)).reverse();
      const contracts = orderIds.map((orderId) => ({
        address: orderBookAddress,
        abi: ORDER_BOOK_ABI,
        functionName: 'orders' as const,
        args: [orderId] as const,
      }));

      const rawOrders = await publicClient.multicall({
        contracts,
        allowFailure: true,
      });

      const nowSeconds = Math.floor(Date.now() / 1000);
      const wallet = baseWallet.address.toLowerCase();

      let data: Order[] = [];
      for (let idx = 0; idx < rawOrders.length; idx += 1) {
        const raw = rawOrders[idx];
        if (raw.status !== 'success') continue;

        const [maker, marketId, isYes, priceBps, size, remaining, expiry, canceled] = raw.result;
        if (maker.toLowerCase() !== wallet) continue;

        const status = orderStatus(canceled, remaining, size, expiry, nowSeconds);
        const id = orderIds[idx].toString();
        const quantity = Number(size) / 1_000_000;
        const remainingQuantity = Number(remaining) / 1_000_000;
        const filledQuantity = Math.max(0, quantity - remainingQuantity);

        const createdAtGuess = new Date(Math.max(0, Number(expiry) - DEFAULT_EXPIRY_SECONDS) * 1000).toISOString();
        const order: Order = {
          id,
          orderId: Number(orderIds[idx]),
          marketId: marketId.toString(),
          owner: maker,
          side: 'buy',
          outcome: isYes ? 'yes' : 'no',
          orderType: 'limit',
          price: Number(priceBps) / 10_000,
          priceBps: Number(priceBps),
          quantity,
          filledQuantity,
          remainingQuantity,
          status,
          isPrivate: false,
          createdAt: createdAtGuess,
          updatedAt: new Date().toISOString(),
          expiresAt: new Date(Number(expiry) * 1000).toISOString(),
        };

        if (filters?.marketId && order.marketId !== filters.marketId) {
          continue;
        }
        if (filters?.status && order.status !== filters.status) {
          continue;
        }
        data.push(order);
      }

      const offset = filters?.offset || 0;
      const limit = filters?.limit || 50;
      const totalFiltered = data.length;
      data = data.slice(offset, offset + limit);

      return {
        data,
        total: totalFiltered,
        limit,
        offset,
        hasMore: offset + limit < totalFiltered,
      };
    },
  });
}

export function useOrder(orderId: string) {
  const { data } = useOrders();
  return useQuery({
    queryKey: ['order', orderId, data?.data.length || 0],
    enabled: !!orderId,
    queryFn: async () => {
      const order = data?.data.find((candidate) => candidate.id === orderId);
      if (!order) throw new Error('Order not found');
      return order;
    },
  });
}

export function usePlaceOrder() {
  const queryClient = useQueryClient();
  const baseWallet = useBaseWallet();
  const config = useConfig();
  const { data: walletClient } = useWalletClient();

  return useMutation({
    mutationFn: async (data: PlaceOrderRequest): Promise<PlaceOrderResponse> => {
      assertWritesEnabled('Trading');

      if (!baseWallet.address || !baseWallet.isConnected) {
        throw new Error('Connect your wallet before placing an order');
      }
      if (!walletClient) {
        throw new Error('Wallet client unavailable');
      }

      await baseWallet.ensureBaseChain();

      const marketId = Number(data.marketId);
      if (!Number.isInteger(marketId) || marketId < 1) {
        throw new Error('Invalid market id');
      }
      const priceBps = Math.max(1, Math.min(9_999, Math.round((data.price ?? 0.5) * 10_000)));
      const size = toBaseUnits(data.quantity);
      const expiry = Math.floor(Date.now() / 1000) + (data.expiresIn || DEFAULT_EXPIRY_SECONDS);

      const prepared = await api.prepareBasePlaceOrder({
        from: baseWallet.address,
        marketId,
        outcome: data.outcome,
        priceBps,
        size: size.toString(),
        expiry,
      });
      const hash = await walletClient.sendTransaction({
        account: baseWallet.address as `0x${string}`,
        to: prepared.to as `0x${string}`,
        data: prepared.data,
        value: BigInt(prepared.value),
      });

      const receipt = await waitForTransactionReceipt(config, { hash });
      const [event] = parseEventLogs({
        abi: ORDER_PLACED_EVENT_ABI,
        eventName: 'OrderPlaced',
        logs: receipt.logs,
      });

      return {
        orderId: event?.args.orderId?.toString() || hash,
        status: 'open',
        txSignature: hash,
      };
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['orders'] });
      queryClient.invalidateQueries({ queryKey: ['positions'] });
      queryClient.invalidateQueries({ queryKey: ['orderbook'] });
      queryClient.invalidateQueries({ queryKey: ['trades'] });
    },
  });
}

export function useCancelOrder() {
  const queryClient = useQueryClient();
  const baseWallet = useBaseWallet();
  const config = useConfig();
  const { data: walletClient } = useWalletClient();

  return useMutation({
    mutationFn: async (orderId: string) => {
      assertWritesEnabled('Order cancellation');

      if (!baseWallet.address || !baseWallet.isConnected) {
        throw new Error('Connect your wallet before cancelling an order');
      }
      if (!walletClient) {
        throw new Error('Wallet client unavailable');
      }

      await baseWallet.ensureBaseChain();

      const parsedOrderId = Number(orderId);
      if (!Number.isInteger(parsedOrderId) || parsedOrderId < 1) {
        throw new Error('Invalid order id');
      }

      const prepared = await api.prepareBaseCancelOrder({
        from: baseWallet.address,
        orderId: parsedOrderId,
      });
      const hash = await walletClient.sendTransaction({
        account: baseWallet.address as `0x${string}`,
        to: prepared.to as `0x${string}`,
        data: prepared.data,
        value: BigInt(prepared.value),
      });

      await waitForTransactionReceipt(config, { hash });
      return { success: true, txSignature: hash };
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['orders'] });
      queryClient.invalidateQueries({ queryKey: ['orderbook'] });
    },
  });
}

