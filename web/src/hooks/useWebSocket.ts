import { useEffect, useRef, useCallback, useState, useSyncExternalStore } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { Outcome } from '@/types';

const WS_URL = process.env.NEXT_PUBLIC_WS_URL || 'ws://localhost:8080/ws';
const RECONNECT_MS = 3000;
const HEARTBEAT_MS = 25000;

type MessageHandler = (data: unknown) => void;

interface WebSocketMessage {
  type: string;
  data: unknown;
}

let sharedSocket: WebSocket | null = null;
let sharedConnected = false;
let refCount = 0;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
const handlers = new Map<string, Set<MessageHandler>>();
const listeners = new Set<() => void>();

function notify() {
  listeners.forEach((fn) => fn());
}

function startHeartbeat() {
  stopHeartbeat();
  heartbeatTimer = setInterval(() => {
    if (sharedSocket?.readyState === WebSocket.OPEN) {
      sharedSocket.send(JSON.stringify({ type: 'ping' }));
    }
  }, HEARTBEAT_MS);
}

function stopHeartbeat() {
  if (heartbeatTimer) {
    clearInterval(heartbeatTimer);
    heartbeatTimer = null;
  }
}

function connect() {
  if (sharedSocket?.readyState === WebSocket.OPEN || sharedSocket?.readyState === WebSocket.CONNECTING) return;

  try {
    sharedSocket = new WebSocket(WS_URL);

    sharedSocket.onopen = () => {
      sharedConnected = true;
      startHeartbeat();
      notify();
    };

    sharedSocket.onclose = () => {
      sharedConnected = false;
      stopHeartbeat();
      notify();
      if (refCount > 0) {
        reconnectTimer = setTimeout(connect, RECONNECT_MS);
      }
    };

    sharedSocket.onerror = () => {
      sharedConnected = false;
      notify();
    };

    sharedSocket.onmessage = (event) => {
      try {
        const message: WebSocketMessage = JSON.parse(event.data);
        const set = handlers.get(message.type);
        if (set) {
          set.forEach((handler) => handler(message.data));
        }
      } catch {}
    };
  } catch {}
}

function disconnect() {
  if (reconnectTimer) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  stopHeartbeat();
  sharedSocket?.close();
  sharedSocket = null;
  sharedConnected = false;
  notify();
}

function subscribe(type: string, handler: MessageHandler) {
  if (!handlers.has(type)) {
    handlers.set(type, new Set());
  }
  handlers.get(type)!.add(handler);
  return () => {
    handlers.get(type)?.delete(handler);
  };
}

function send(type: string, data: unknown) {
  if (sharedSocket?.readyState === WebSocket.OPEN) {
    sharedSocket.send(JSON.stringify({ type, data }));
  }
}

function getSnapshot() {
  return sharedConnected;
}

function subscribeToConnection(callback: () => void) {
  listeners.add(callback);
  return () => listeners.delete(callback);
}

export function useWebSocket() {
  const isConnected = useSyncExternalStore(subscribeToConnection, getSnapshot, () => false);

  useEffect(() => {
    refCount++;
    if (refCount === 1) connect();
    return () => {
      refCount--;
      if (refCount === 0) disconnect();
    };
  }, []);

  return { isConnected, subscribe, send };
}

export function useOrderBookSubscription(marketId: string, outcome: Outcome) {
  const queryClient = useQueryClient();
  const { subscribe: sub, send: wsSend, isConnected } = useWebSocket();

  useEffect(() => {
    if (!isConnected || !marketId) return;

    wsSend('subscribe', { channel: 'orderbook', marketId, outcome });

    const unsubscribe = sub('orderbook_update', (data) => {
      const update = data as { marketId: string; outcome: Outcome };
      if (update.marketId === marketId && update.outcome === outcome) {
        queryClient.invalidateQueries({ queryKey: ['orderbook', marketId, outcome] });
      }
    });

    return () => {
      wsSend('unsubscribe', { channel: 'orderbook', marketId, outcome });
      unsubscribe();
    };
  }, [isConnected, marketId, outcome, sub, wsSend, queryClient]);
}

export function useTradeSubscription(marketId: string) {
  const queryClient = useQueryClient();
  const { subscribe: sub, send: wsSend, isConnected } = useWebSocket();

  useEffect(() => {
    if (!isConnected || !marketId) return;

    wsSend('subscribe', { channel: 'trades', marketId });

    const unsubscribe = sub('trade', (data) => {
      const trade = data as { marketId: string };
      if (trade.marketId === marketId) {
        queryClient.invalidateQueries({ queryKey: ['trades', marketId] });
        queryClient.invalidateQueries({ queryKey: ['market', marketId] });
      }
    });

    return () => {
      wsSend('unsubscribe', { channel: 'trades', marketId });
      unsubscribe();
    };
  }, [isConnected, marketId, sub, wsSend, queryClient]);
}

export function usePriceSubscription(marketId: string) {
  const queryClient = useQueryClient();
  const { subscribe: sub, send: wsSend, isConnected } = useWebSocket();

  useEffect(() => {
    if (!isConnected || !marketId) return;

    wsSend('subscribe', { channel: 'prices', marketId });

    const unsubscribe = sub('price_update', (data) => {
      const update = data as { marketId: string };
      if (update.marketId === marketId) {
        queryClient.invalidateQueries({ queryKey: ['market', marketId] });
      }
    });

    return () => {
      wsSend('unsubscribe', { channel: 'prices', marketId });
      unsubscribe();
    };
  }, [isConnected, marketId, sub, wsSend, queryClient]);
}

export function useMarketLiveData(marketId: string, outcome: Outcome = 'yes') {
  useOrderBookSubscription(marketId, outcome);
  useTradeSubscription(marketId);
  usePriceSubscription(marketId);
}

/** Subscribe to distribution market live updates (aggregate mu/sigma, trades, resolution). */
export function useDistributionLiveData(marketId: string) {
  const queryClient = useQueryClient();
  const { subscribe: sub, send: wsSend, isConnected } = useWebSocket();

  useEffect(() => {
    if (!isConnected || !marketId) return;

    wsSend('subscribe', { channel: 'distribution', marketId });

    const unsubs = [
      sub('dist_market_update', (data) => {
        const update = data as { market_id: string };
        if (update.market_id === marketId) {
          queryClient.invalidateQueries({ queryKey: ['distribution-market', marketId] });
          queryClient.invalidateQueries({ queryKey: ['distribution-curve', marketId] });
        }
      }),
      sub('dist_trade', (data) => {
        const update = data as { market_id: string };
        if (update.market_id === marketId) {
          queryClient.invalidateQueries({ queryKey: ['distribution-market', marketId] });
          queryClient.invalidateQueries({ queryKey: ['distribution-positions'] });
          queryClient.invalidateQueries({ queryKey: ['distribution-curve', marketId] });
          queryClient.invalidateQueries({ queryKey: ['distribution-activity', marketId] });
        }
      }),
      sub('dist_resolve', (data) => {
        const update = data as { market_id: string };
        if (update.market_id === marketId) {
          queryClient.invalidateQueries({ queryKey: ['distribution-market', marketId] });
          queryClient.invalidateQueries({ queryKey: ['distribution-positions'] });
          queryClient.invalidateQueries({ queryKey: ['distribution-markets'] });
        }
      }),
    ];

    return () => {
      wsSend('unsubscribe', { channel: 'distribution', marketId });
      unsubs.forEach((fn) => fn());
    };
  }, [isConnected, marketId, sub, wsSend, queryClient]);
}
