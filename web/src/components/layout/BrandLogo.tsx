'use client';

import Image from 'next/image';
import { useTheme } from '@/components/ThemeProvider';

interface BrandLogoProps {
  compact?: boolean;
}

export function BrandLogo({ compact = false }: BrandLogoProps) {
  const { resolvedTheme } = useTheme();
  const logoSrc = resolvedTheme === 'dark' ? '/relay44-logo-w.svg' : '/relay44-logo-b.svg';

  return (
    <Image
      src={logoSrc}
      alt="Relay44"
      width={compact ? 188 : 313}
      height={compact ? 125 : 208}
      priority
      className={compact ? 'h-[50px] w-auto' : 'h-[75px] w-auto'}
    />
  );
}
