'use client';

import { useEffect, useState } from 'react';
import sdk from '@farcaster/miniapp-sdk';
import { useFarcaster } from '@/components/farcaster';

export function MiniKitInit() {
  const [isSDKLoaded, setIsSDKLoaded] = useState(false);
  const { setUser, setIsReady } = useFarcaster();

  useEffect(() => {
    if (sdk && !isSDKLoaded) {
      setIsSDKLoaded(true);

      const load = async () => {
        const context = await sdk.context;
        if (context?.user) {
          setUser({
            fid: context.user.fid,
            username: context.user.username,
            displayName: context.user.displayName ?? undefined,
            pfpUrl: context.user.pfpUrl ?? undefined,
          });
        }

        sdk.actions.ready({});
        setIsReady(true);
      };

      load().catch(() => {
        sdk.actions.ready({});
        setIsReady(true);
      });
    }
  }, [isSDKLoaded, setUser, setIsReady]);

  return null;
}
