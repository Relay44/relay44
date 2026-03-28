'use client';

import Image from 'next/image';
import { useTheme } from '@/components/ThemeProvider';

interface BrandLogoProps {
  compact?: boolean;
}

export function BrandLogo({ compact = false }: BrandLogoProps) {
  const { resolvedTheme } = useTheme();
  const logoSrc = resolvedTheme === 'dark' ? '/relay44-logo-w.svg' : '/relay44-logo-b.svg';
  const logoWidth = 644;
  const logoHeight = 401;

  return (
    <Image
      src={logoSrc}
      alt="Relay44"
      width={logoWidth}
      height={logoHeight}
      priority
      className={compact ? 'h-[50px] w-auto' : 'h-[75px] w-auto'}
    />
  );
}
