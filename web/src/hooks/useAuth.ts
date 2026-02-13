import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useSignMessage } from 'wagmi';

import { useBaseWallet } from '@/hooks/useBaseWallet';
import { useSolanaWallet } from '@/hooks/useSolanaWallet';
import { api } from '@/lib/api';
import { BASE_CHAIN_ID, CHAIN_MODE } from '@/lib/constants';

export function useAuth() {
  const baseWallet = useBaseWallet();
  const solanaWallet = useSolanaWallet();
  const { signMessageAsync } = useSignMessage();

  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sessionRestored, setSessionRestored] = useState(false);
  const previousWalletConnectedRef = useRef<boolean | null>(null);

  const activeFlow = useMemo<'siwe' | 'solana' | 'farcaster' | null>(() => {
    if (CHAIN_MODE === 'base') {
      return baseWallet.isConnected ? 'siwe' : null;
    }
    if (CHAIN_MODE === 'solana') {
      return solanaWallet.isConnected ? 'solana' : null;
    }
    if (baseWallet.isConnected) {
      return 'siwe';
    }
    if (solanaWallet.isConnected) {
      return 'solana';
    }
    return null;
  }, [baseWallet.isConnected, solanaWallet.isConnected]);

  const walletConnected = baseWallet.isConnected || solanaWallet.isConnected;
  const walletAddress = activeFlow === 'siwe' ? baseWallet.address : solanaWallet.address;

  useEffect(() => {
    let mounted = true;
    api.restoreSession().then((restored) => {
      if (mounted) {
        setIsAuthenticated(restored);
        setSessionRestored(true);
      }
    });
    return () => {
      mounted = false;
    };
  }, []);

  useEffect(() => {
    if (!sessionRestored) {
      return;
    }

    const previousWalletConnected = previousWalletConnectedRef.current;
    previousWalletConnectedRef.current = walletConnected;

    if (previousWalletConnected && !walletConnected) {
      api.logout().then(() => setIsAuthenticated(false));
    }
  }, [walletConnected, sessionRestored]);

  const login = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      if (activeFlow === 'siwe') {
        if (!baseWallet.isConnected || !baseWallet.address) {
          throw new Error('Base wallet not connected');
        }

        await baseWallet.ensureBaseChain();

        const nonce = await api.getSiweNonce();
        const issuedAt = new Date().toISOString();
        const domain = process.env.NEXT_PUBLIC_SIWE_DOMAIN || window.location.host;
        const uri = window.location.origin;
        const chainId = baseWallet.chainId ?? BASE_CHAIN_ID;
        const message = `${domain} wants you to sign in with your Ethereum account:\n${baseWallet.address}\n\nSign in to relay44\n\nURI: ${uri}\nVersion: 1\nChain ID: ${chainId}\nNonce: ${nonce}\nIssued At: ${issuedAt}`;

        const signature = await signMessageAsync({ message });
        await api.loginSiwe(baseWallet.address, signature, message);
      } else if (activeFlow === 'solana') {
        if (!solanaWallet.isConnected || !solanaWallet.address) {
          throw new Error('Solana wallet not connected');
        }

        const nonce = await api.getSolanaNonce();
        const issuedAt = new Date().toISOString();
        const domain = process.env.NEXT_PUBLIC_SIWE_DOMAIN || window.location.host;
        const uri = window.location.origin;
        const message = `${domain} wants you to sign in with your Solana account:\n${solanaWallet.address}\n\nSign in to relay44\n\nURI: ${uri}\nVersion: 1\nChain: solana\nNonce: ${nonce}\nIssued At: ${issuedAt}`;
        const signature = await solanaWallet.signMessage(message);
        await api.loginSolana(solanaWallet.address, signature, message);
      } else {
        throw new Error('Connect a wallet before logging in');
      }

      setIsAuthenticated(true);
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Authentication failed';
      setError(msg);
      console.error('Login error:', err);
    } finally {
      setIsLoading(false);
    }
  }, [activeFlow, baseWallet, signMessageAsync, solanaWallet]);

  const logout = useCallback(async () => {
    try {
      await api.logout();
    } catch {
      // Ignore logout errors
    }

    setIsAuthenticated(false);
    baseWallet.disconnect();
    if (solanaWallet.isConnected) {
      solanaWallet.disconnect().catch(() => {
      });
    }
  }, [baseWallet, solanaWallet]);

  return {
    isAuthenticated,
    isLoading,
    error,
    login,
    logout,
    walletConnected,
    walletAddress,
    activeFlow,
    baseWalletConnected: baseWallet.isConnected,
    solanaWalletConnected: solanaWallet.isConnected,
  };
}
