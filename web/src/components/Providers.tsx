'use client';

import { FC, ReactNode, useEffect, useState } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { WagmiProvider } from 'wagmi';
import { AppKitProvider } from '@reown/appkit/react';

import { ErrorBoundary } from '@/components/ErrorBoundary';
import { ThemeProvider } from '@/components/ThemeProvider';
import { NotificationProvider } from '@/components/notifications';
import { ToastProvider } from '@/components/ui';
import { FarcasterProvider } from '@/components/farcaster';
import { MiniKitWrapper } from '@/components/minikit';
import { config, appKitConfig } from '@/lib/wagmi';

interface ProvidersProps {
  children: ReactNode;
}

export const Providers: FC<ProvidersProps> = ({ children }) => {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime: 10 * 1000,
            refetchOnWindowFocus: false,
          },
        },
      })
  );

  useEffect(() => {
    const disabled = ['1', 'true', 'yes', 'on'].includes(
      String(process.env.NEXT_PUBLIC_DISABLE_PWA || '')
        .trim()
        .toLowerCase()
    );
    if (!disabled || typeof window === 'undefined') return;

    void (async () => {
      if ('serviceWorker' in navigator) {
        const registrations = await navigator.serviceWorker.getRegistrations();
        await Promise.all(registrations.map((registration) => registration.unregister()));
      }

      if ('caches' in window) {
        const keys = await caches.keys();
        await Promise.all(
          keys
            .filter((key) => key.includes('workbox') || key.includes('next-pwa'))
            .map((key) => caches.delete(key))
        );
      }
    })();
  }, []);

  return (
    <ErrorBoundary>
      <ThemeProvider>
        <WagmiProvider config={config}>
          <QueryClientProvider client={queryClient}>
            {appKitConfig ? (
              <AppKitProvider {...appKitConfig}>
                <MiniKitWrapper>
                  <FarcasterProvider>
                    <NotificationProvider>
                      <ToastProvider>{children}</ToastProvider>
                    </NotificationProvider>
                  </FarcasterProvider>
                </MiniKitWrapper>
              </AppKitProvider>
            ) : (
              <MiniKitWrapper>
                <FarcasterProvider>
                  <NotificationProvider>
                    <ToastProvider>{children}</ToastProvider>
                  </NotificationProvider>
                </FarcasterProvider>
              </MiniKitWrapper>
            )}
          </QueryClientProvider>
        </WagmiProvider>
      </ThemeProvider>
    </ErrorBoundary>
  );
};
