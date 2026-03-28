'use client';

import { FC, ReactNode } from 'react';

interface MiniKitWrapperProps {
  children: ReactNode;
}

export const MiniKitWrapper: FC<MiniKitWrapperProps> = ({ children }) => {
  return <>{children}</>;
};
