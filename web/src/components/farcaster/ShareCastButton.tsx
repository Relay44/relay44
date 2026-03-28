'use client';

import { FC } from 'react';
import { Share2 } from 'lucide-react';
import { isMiniApp, composeCast as fcComposeCast } from '@/lib/farcaster';

interface ShareCastButtonProps {
  text: string;
  embedUrl: string;
  className?: string;
}

export const ShareCastButton: FC<ShareCastButtonProps> = ({
  text,
  embedUrl,
  className,
}) => {
  const handleShare = async () => {
    if (isMiniApp()) {
      await fcComposeCast({ text, embeds: [embedUrl] });
    } else {
      const encoded = encodeURIComponent(text);
      const encodedUrl = encodeURIComponent(embedUrl);
      window.open(
        `https://warpcast.com/~/compose?text=${encoded}&embeds[]=${encodedUrl}`,
        '_blank',
      );
    }
  };

  return (
    <button
      onClick={handleShare}
      className={`inline-flex items-center gap-1.5 rounded-md border border-border/50 bg-bg-secondary px-3 py-1.5 text-xs font-medium text-text-secondary ${className ?? ''}`}
    >
      <Share2 className="h-3.5 w-3.5" />
      Share on Farcaster
    </button>
  );
};
