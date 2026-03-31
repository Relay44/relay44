'use client';

import { useEffect, useRef } from 'react';
import { sdk } from '@farcaster/miniapp-sdk';
import { useFarcaster } from '@/components/farcaster';

export function MiniKitInit() {
  const called = useRef(false);
  const { setUser, setIsReady } = useFarcaster();

  useEffect(() => {
    if (called.current) return;
    called.current = true;

    (async () => {
      try {
        await sdk.actions.ready({ disableNativeGestures: false });
      } catch {
        // ignore
      }

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
  }, [setUser, setIsReady]);

  return null;
}
