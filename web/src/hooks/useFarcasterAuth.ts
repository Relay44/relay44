'use client';

import { useState, useCallback } from 'react';
import { signInWithFarcaster } from '@/lib/farcaster';
import { api } from '@/lib/api';

interface FarcasterAuthState {
  isLoading: boolean;
  error: string | null;
  login: () => Promise<boolean>;
}

export function useFarcasterAuth(): FarcasterAuthState {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const login = useCallback(async (): Promise<boolean> => {
    setIsLoading(true);
    setError(null);

    try {
      const nonce = await api.getFarcasterNonce();
      let result;
      try {
        result = await signInWithFarcaster(nonce);
      } catch (signErr) {
        throw new Error(`Sign-in prompt failed: ${signErr instanceof Error ? signErr.message : String(signErr)}`);
      }

      if (!result) {
        throw new Error('Sign-in was dismissed or returned no result');
      }

      try {
        await api.loginFarcaster(result.message, result.signature, nonce);
      } catch (loginErr) {
        throw new Error(`Login API failed: ${loginErr instanceof Error ? loginErr.message : String(loginErr)}`);
      }

      return true;
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Farcaster sign-in failed';
      setError(msg);
      return false;
    } finally {
      setIsLoading(false);
    }
  }, []);

  return { isLoading, error, login };
}
