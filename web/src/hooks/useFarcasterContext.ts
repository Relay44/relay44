'use client';

import { useState, useCallback } from 'react';
import { isMiniApp, close, composeCast, addMiniApp } from '@/lib/farcaster';

export interface FarcasterUser {
  fid: number;
  username?: string;
  displayName?: string;
  pfpUrl?: string;
}

export interface FarcasterContextState {
  isMiniApp: boolean;
  isReady: boolean;
  user: FarcasterUser | null;
  setUser: (user: FarcasterUser | null) => void;
  setIsReady: (ready: boolean) => void;
  close: () => Promise<void>;
  composeCast: (text?: string, embeds?: [] | [string] | [string, string]) => Promise<void>;
  addMiniApp: () => Promise<void>;
}

export function useFarcasterContext(): FarcasterContextState {
  const [isInMiniApp] = useState(() => isMiniApp());
  const [user, setUser] = useState<FarcasterUser | null>(null);
  const [isReady, setIsReady] = useState(!isInMiniApp);

  const handleComposeCast = useCallback(
    async (text?: string, embeds?: [] | [string] | [string, string]) => {
      await composeCast({ text, embeds });
    },
    [],
  );

  return {
    isMiniApp: isInMiniApp,
    isReady,
    user,
    setUser,
    setIsReady,
    close,
    composeCast: handleComposeCast,
    addMiniApp,
  };
}
