'use client';

import { useEffect, useMemo } from 'react';
import { useAccount, useConnect, useDisconnect, useSwitchChain } from 'wagmi';

import { BASE_CHAIN_ID } from '@/lib/constants';
import { isMiniApp } from '@/lib/farcaster';

export function useBaseWallet() {
  const account = useAccount();
  const { connectAsync, connectors, isPending: connectPending } = useConnect();
  const { disconnect } = useDisconnect();
  const { switchChainAsync, isPending: switchPending } = useSwitchChain();

  const preferredConnector = useMemo(() => {
    if (connectors.length === 0) return undefined;

    // In miniapp context, prefer the Farcaster or Coinbase connector
    if (isMiniApp()) {
      const miniAppConnector = connectors.find((c) => {
        const name = c.name.toLowerCase();
        return name.includes('farcaster') || name.includes('coinbase');
      });
      if (miniAppConnector) return miniAppConnector;
    }

    const coinbase = connectors.find((c) =>
      c.name.toLowerCase().includes('coinbase')
    );
    if (coinbase) return coinbase;

    return (
      connectors.find((connector) =>
        connector.name.toLowerCase().includes('metamask')
      ) || connectors[0]
    );
  }, [connectors]);

  // Auto-connect in miniapp context
  useEffect(() => {
    if (!isMiniApp() || account.isConnected || connectPending || !preferredConnector) return;
    connectAsync({ connector: preferredConnector }).catch(() => {});
  }, [account.isConnected, connectPending, preferredConnector, connectAsync]);

  const connect = async () => {
    const connector = preferredConnector;
    if (!connector) {
      throw new Error('No wallet connector available');
    }
    await connectAsync({ connector });
  };

  const ensureBaseChain = async () => {
    if (account.chainId === BASE_CHAIN_ID) {
      return;
    }
    await switchChainAsync({ chainId: BASE_CHAIN_ID });
  };

  return {
    enabled: true,
    address: account.address,
    isConnected: account.isConnected,
    chainId: account.chainId,
    isConnecting: connectPending,
    isSwitchingChain: switchPending,
    connect,
    disconnect,
    ensureBaseChain,
  };
}
