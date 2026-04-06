'use client';

import { useEffect, useMemo } from 'react';
import { useAccount, useConnect, useDisconnect, useSwitchChain } from 'wagmi';

import { BASE_CHAIN_ID } from '@/lib/constants';
import { isMiniApp } from '@/lib/farcaster';
import { reownProjectId } from '@/lib/wagmi';

async function openAppKitModal() {
  // Dynamic import so the hook context is not required at module scope.
  // AppKitProvider must be in the tree — guarded by reownProjectId check.
  const { modal } = await import('@reown/appkit/react');
  await modal?.open();
}

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

  // Auto-connect in miniapp context (once only — stop if user dismisses)
  const triedAutoConnect = useMemo(() => ({ current: false }), []);
  useEffect(() => {
    if (!isMiniApp() || account.isConnected || connectPending || !preferredConnector) return;
    if (triedAutoConnect.current) return;
    triedAutoConnect.current = true;
    connectAsync({ connector: preferredConnector }).catch(() => {});
  }, [account.isConnected, connectPending, preferredConnector, connectAsync, triedAutoConnect]);

  const connect = async () => {
    // In miniapp context, connect directly with the preferred connector
    if (isMiniApp()) {
      const connector = preferredConnector;
      if (!connector) {
        throw new Error('No wallet connector available');
      }
      await connectAsync({ connector });
      return;
    }

    // On regular web, open the AppKit modal so the user can choose their wallet
    if (reownProjectId) {
      await openAppKitModal();
      return;
    }

    // Fallback when AppKit is not configured (no REOWN_PROJECT_ID)
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
