'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';

type SolanaEvent = 'connect' | 'disconnect' | 'accountChanged';

type SolanaProvider = {
  isPhantom?: boolean;
  isConnected?: boolean;
  publicKey?: { toBase58(): string };
  connect(opts?: { onlyIfTrusted?: boolean }): Promise<{ publicKey?: { toBase58(): string } }>;
  disconnect(): Promise<void>;
  signMessage(message: Uint8Array, encoding?: 'utf8' | 'hex'): Promise<{ signature: Uint8Array }>;
  on?(event: SolanaEvent, handler: (...args: unknown[]) => void): void;
  off?(event: SolanaEvent, handler: (...args: unknown[]) => void): void;
};

function toBase64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return window.btoa(binary);
}

function getProvider(): SolanaProvider | null {
  if (typeof window === 'undefined') return null;
  const maybe =
    (window as Window & { phantom?: { solana?: SolanaProvider } }).phantom?.solana
    || (window as Window & { solana?: SolanaProvider }).solana;
  return maybe ?? null;
}

export function useSolanaWallet() {
  const [provider, setProvider] = useState<SolanaProvider | null>(null);
  const [address, setAddress] = useState<string | undefined>();
  const [isConnected, setIsConnected] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);

  useEffect(() => {
    const walletProvider = getProvider();
    setProvider(walletProvider);

    if (!walletProvider) {
      setAddress(undefined);
      setIsConnected(false);
      return;
    }

    const sync = () => {
      const pubkey = walletProvider.publicKey?.toBase58();
      setAddress(pubkey);
      setIsConnected(Boolean(walletProvider.isConnected && pubkey));
    };

    sync();
    walletProvider.on?.('connect', sync);
    walletProvider.on?.('disconnect', sync);
    walletProvider.on?.('accountChanged', sync);

    return () => {
      walletProvider.off?.('connect', sync);
      walletProvider.off?.('disconnect', sync);
      walletProvider.off?.('accountChanged', sync);
    };
  }, []);

  const connect = useCallback(async () => {
    if (!provider) {
      throw new Error('No Solana wallet provider available');
    }

    setIsConnecting(true);
    try {
      const response = await provider.connect();
      const pubkey = response.publicKey?.toBase58() || provider.publicKey?.toBase58();
      setAddress(pubkey);
      setIsConnected(Boolean(pubkey));
    } finally {
      setIsConnecting(false);
    }
  }, [provider]);

  const disconnect = useCallback(async () => {
    if (!provider) {
      return;
    }
    await provider.disconnect();
    setAddress(undefined);
    setIsConnected(false);
  }, [provider]);

  const signMessage = useCallback(async (message: string) => {
    if (!provider) {
      throw new Error('No Solana wallet provider available');
    }

    const encoded = new TextEncoder().encode(message);
    const signed = await provider.signMessage(encoded, 'utf8');
    return toBase64(signed.signature);
  }, [provider]);

  const walletType = useMemo(() => {
    if (!provider) return 'none';
    if (provider.isPhantom) return 'phantom';
    return 'solana';
  }, [provider]);

  return {
    enabled: !!provider,
    walletType,
    address,
    isConnected,
    isConnecting,
    connect,
    disconnect,
    signMessage,
  };
}
