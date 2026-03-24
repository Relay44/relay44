'use client';

import { createContext, FC, ReactNode, useContext } from 'react';
import { useFarcasterContext } from '@/hooks/useFarcasterContext';
import type { FarcasterContextState } from '@/hooks/useFarcasterContext';

const FarcasterContext = createContext<FarcasterContextState>({
  isMiniApp: false,
  isReady: false,
  user: null,
  setUser: () => {},
  setIsReady: () => {},
  close: async () => {},
  composeCast: async () => {},
  addMiniApp: async () => {},
});

export const useFarcaster = () => useContext(FarcasterContext);

interface FarcasterProviderProps {
  children: ReactNode;
}

export const FarcasterProvider: FC<FarcasterProviderProps> = ({ children }) => {
  const ctx = useFarcasterContext();
  return (
    <FarcasterContext.Provider value={ctx}>{children}</FarcasterContext.Provider>
  );
};
