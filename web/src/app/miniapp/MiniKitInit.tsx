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

    sdk.actions.ready().catch(() => {});

    (async () => {
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
        // context not available outside miniapp
      }

      setIsReady(true);
    })();
  }, [setUser, setIsReady]);

  return null;
}
