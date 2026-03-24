'use client';

import { FC, ReactNode } from 'react';
import { MiniKitProvider } from '@coinbase/onchainkit/minikit';
import { base, baseSepolia } from 'wagmi/chains';
import { BASE_CHAIN_ID, BASE_RPC_ENDPOINT } from '@/lib/constants';

const CDP_API_KEY = process.env.NEXT_PUBLIC_CDP_CLIENT_API_KEY ?? '';
const PROJECT_ID = process.env.NEXT_PUBLIC_MINIKIT_PROJECT_ID ?? '';

interface MiniKitWrapperProps {
  children: ReactNode;
}

export const MiniKitWrapper: FC<MiniKitWrapperProps> = ({ children }) => {
  const chain = BASE_CHAIN_ID === baseSepolia.id ? baseSepolia : base;

  return (
    <MiniKitProvider
      apiKey={CDP_API_KEY}
      projectId={PROJECT_ID}
      chain={chain}
      rpcUrl={BASE_RPC_ENDPOINT}
      config={{ appearance: { mode: 'dark' } }}
    >
      {children}
    </MiniKitProvider>
  );
};
