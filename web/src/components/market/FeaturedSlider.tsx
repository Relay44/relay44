'use client';

import { useRef } from 'react';
import Link from 'next/link';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { Market } from '@/types';

export interface FeaturedSliderProps {
  markets: Market[];
  title?: string;
}

function FeaturedCard({ market }: { market: Market }) {
  const yesPrice = Math.round(market.yesPrice * 100);
  const noPrice = Math.round(market.noPrice * 100);

  return (
    <Link href={`/markets/${encodeURIComponent(market.id)}`} className="block group flex-shrink-0 w-[33.333vw] md:w-[33.333vw] max-w-[33.333%] h-[560px] md:h-[600px]">
      <div
        className={cn(
          'h-full overflow-hidden flex flex-col',
          'bg-bg-primary border border-border hover:border-text-muted',
          'p-6',
          'transition-colors duration-fast'
        )}
      >
        <div className="flex items-center gap-2 mb-4">
          <span className="px-2 py-0.5 text-[0.65rem] font-mono uppercase tracking-wider border border-border text-text-muted">
            {market.category}
          </span>
        </div>

        <h3 className="text-base font-medium text-text-primary mb-auto line-clamp-3 group-hover:underline transition-colors min-h-[72px]" style={{ fontFamily: 'var(--font-display)' }}>
          {market.question}
        </h3>

        <div className="flex gap-2 mt-4">
          <button
            type="button"
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
            }}
            className={cn(
              'flex-1 py-2 px-3 font-mono text-[0.8rem] font-bold',
              'border border-border text-text-primary',
              'hover:bg-text-primary hover:text-text-inverse',
              'transition-colors cursor-pointer',
              'flex items-center justify-between'
            )}
          >
            <span>YES</span>
            <span>{yesPrice}¢</span>
          </button>
          <button
            type="button"
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
            }}
            className={cn(
              'flex-1 py-2 px-3 font-mono text-[0.8rem] font-bold',
              'border border-border text-text-primary',
              'hover:bg-text-primary hover:text-text-inverse',
              'transition-colors cursor-pointer',
              'flex items-center justify-between'
            )}
          >
            <span>NO</span>
            <span>{noPrice}¢</span>
          </button>
        </div>

        <div className="mt-2 text-[0.7rem] font-mono text-text-muted uppercase">
          ${(market.volume24h / 1000).toFixed(0)}k vol
        </div>
      </div>
    </Link>
  );
}

export function FeaturedSlider({ markets, title }: FeaturedSliderProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  const scroll = (direction: 'left' | 'right') => {
    if (!scrollRef.current) return;
    const scrollAmount = 360;
    scrollRef.current.scrollBy({
      left: direction === 'left' ? -scrollAmount : scrollAmount,
      behavior: 'smooth',
    });
  };

  if (!markets || markets.length === 0) {
    return (
      <div className="relative">
        {title && (
          <div className="flex items-center justify-between mb-4 px-4 sm:px-6 md:px-8">
            <h2 className="text-[1.4rem] font-mono uppercase tracking-wider text-text-muted">{title}</h2>
          </div>
        )}
        <div className="flex gap-3 overflow-hidden px-4 sm:px-6 md:px-8">
          {[1, 2, 3].map((i) => (
            <div key={i} className="flex-shrink-0 w-[33.333vw] md:w-[33.333vw] max-w-[33.333%] h-[560px] md:h-[600px] bg-bg-secondary animate-pulse" />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="relative">
      {title && (
        <div className="flex items-center justify-between mb-3 px-4 sm:px-6 md:px-8">
          <h2 className="text-[0.7rem] font-mono uppercase tracking-wider text-text-muted">{title}</h2>
          <div className="flex gap-1">
            <button
              onClick={() => scroll('left')}
              className="p-1.5 border border-border text-text-muted hover:text-text-primary hover:border-text-muted transition-colors cursor-pointer"
            >
              <ChevronLeft className="w-4 h-4" />
            </button>
            <button
              onClick={() => scroll('right')}
              className="p-1.5 border border-border text-text-muted hover:text-text-primary hover:border-text-muted transition-colors cursor-pointer"
            >
              <ChevronRight className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      <div
        ref={scrollRef}
        className="flex gap-3 overflow-x-auto scrollbar-hide px-4 sm:px-6 md:px-8 pb-2"
        style={{ scrollSnapType: 'x mandatory' }}
      >
        {markets.map((market) => (
          <div key={market.id} style={{ scrollSnapAlign: 'start' }}>
            <FeaturedCard market={market} />
          </div>
        ))}
      </div>
    </div>
  );
}
