'use client';

import { useEffect, useRef } from 'react';
import { useMiniKit } from '@coinbase/onchainkit/minikit';
import sdk from '@farcaster/miniapp-sdk';
import { useFarcaster } from '@/components/farcaster';

export function MiniKitInit() {
  const called = useRef(false);
  const { setFrameReady } = useMiniKit();
  const { setUser, setIsReady } = useFarcaster();

  useEffect(() => {
    if (called.current) return;
    called.current = true;

    (async () => {
      // 1. Signal ready to host
      try {
        setFrameReady();
      } catch {
        // ignore if provider isn't ready
      }

      try {
        await sdk.actions.ready({ disableNativeGestures: false });
      } catch {
        // ignore
      }

      // 2. Now that ready handshake is done, fetch user context
      try {
        const ctx = await sdk.context;
        if (ctx?.user) {
          setUser({
            fid: ctx.user.fid,
            username: ctx.user.username,
            displayName: ctx.user.displayName ?? undefined,
            pfpUrl: ctx.user.pfpUrl ?? undefined,
          });
        }
      } catch {
        // ignore
      }

      setIsReady(true);
    })();
  }, [setFrameReady, setUser, setIsReady]);

  return null;
}
