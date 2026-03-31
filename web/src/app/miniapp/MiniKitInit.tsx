'use client';

import { useEffect, useState } from 'react';
import { sdk } from '@farcaster/miniapp-sdk';
import { useFarcaster } from '@/components/farcaster';

export function MiniKitInit() {
  const [isSDKLoaded, setIsSDKLoaded] = useState(false);
  const { setUser, setIsReady } = useFarcaster();

  useEffect(() => {
    if (isSDKLoaded) return;
    setIsSDKLoaded(true);

    sdk.actions.ready({});

    sdk.context.then((ctx) => {
      if (ctx?.user) {
        setUser({
          fid: ctx.user.fid,
          username: ctx.user.username,
          displayName: ctx.user.displayName ?? undefined,
          pfpUrl: ctx.user.pfpUrl ?? undefined,
        });
      }
      setIsReady(true);
    }).catch(() => {
      setIsReady(true);
    });
  }, [isSDKLoaded, setUser, setIsReady]);

  return null;
}
