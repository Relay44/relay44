import Image from "next/image";
import { buildMarketArtworkDataUrl } from "@/lib/marketArtwork";
import { cn } from "@/lib/utils";
import type { Market } from "@/types";

interface MarketArtworkProps {
  market: Pick<Market, "category" | "id" | "imageUrl" | "provider" | "question">;
  className?: string;
  sizes: string;
  priority?: boolean;
}

export function MarketArtwork({
  market,
  className,
  sizes,
  priority = false,
}: MarketArtworkProps) {
  const src =
    market.imageUrl ||
    buildMarketArtworkDataUrl(
      [market.question, market.category, market.provider, market.id].join(" ")
    );

  return (
    <div
      className={cn(
        "relative overflow-hidden border border-border bg-bg-secondary",
        className
      )}
    >
      <Image
        src={src}
        alt={market.question}
        fill
        sizes={sizes}
        priority={priority}
        unoptimized={src.startsWith("data:")}
        className="object-cover [image-rendering:pixelated]"
      />
    </div>
  );
}
