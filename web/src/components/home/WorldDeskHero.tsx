'use client';

import Link from 'next/link';
import { useEffect, useMemo, useState } from 'react';
import { ArrowLeft, ArrowRight } from 'lucide-react';
import { Button } from '@/components/ui';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import type { NewsSlide } from '@/lib/server/homeLive';
import { cn } from '@/lib/utils';

interface WorldDeskHeroProps {
  slides: NewsSlide[];
}

function truncateDescription(value: string, limit = 108): string {
  const clean = value.replace(/\s+/g, ' ').trim();
  if (clean.length <= limit) {
    return clean;
  }

  return `${clean.slice(0, limit).trimEnd()}...`;
}

function buildDraftHref(slide: NewsSlide): string {
  const primaryDraft = slide.marketDrafts[0];
  const query = new URLSearchParams({
    story: slide.id,
    draft: primaryDraft.id,
    question: primaryDraft.question,
    description: primaryDraft.description,
    category: primaryDraft.category,
    resolutionSource: primaryDraft.resolutionSource,
    tradingEnd: primaryDraft.tradingEnd,
  });

  if (primaryDraft.customSource) {
    query.set('customSource', primaryDraft.customSource);
  }

  return `/markets/create?${query.toString()}`;
}

export function WorldDeskHero({ slides }: WorldDeskHeroProps) {
  const safeSlides = useMemo(() => slides.slice(0, 6), [slides]);
  const [activeIndex, setActiveIndex] = useState(0);
  const [isStoryOpen, setIsStoryOpen] = useState(false);
  const [openSlide, setOpenSlide] = useState<NewsSlide | null>(null);

  useEffect(() => {
    if (safeSlides.length <= 1 || isStoryOpen) {
      return;
    }

    const timer = window.setInterval(() => {
      setActiveIndex((current) => (current + 1) % safeSlides.length);
    }, 8000);

    return () => {
      window.clearInterval(timer);
    };
  }, [isStoryOpen, safeSlides.length]);

  useEffect(() => {
    setActiveIndex(0);
  }, [safeSlides]);

  useEffect(() => {
    if (!isStoryOpen) {
      setOpenSlide(null);
    }
  }, [isStoryOpen]);

  if (safeSlides.length === 0) {
    return null;
  }

  const activeSlide = safeSlides[activeIndex] || safeSlides[0];
  const modalSlide = openSlide || activeSlide;

  return (
    <section className="border-b border-border pb-8 pt-8">
      <div className="grid gap-5 lg:h-[500px] lg:max-h-[500px] lg:grid-cols-[minmax(0,1fr)_280px]">
        <div className="min-w-0 max-h-[500px] overflow-hidden border border-border bg-bg-primary px-5 py-5 brutal-shadow sm:px-6 sm:py-6 lg:h-full">
          <div className="flex h-full flex-col">
            <div className="flex flex-wrap items-center gap-3">
              <span className="border border-accent/30 bg-accent/10 px-3 py-1 text-[11px] uppercase tracking-[0.22em] text-accent">
                Current coverage
              </span>
            </div>

            <div className="mt-4 min-h-[7rem] sm:min-h-[8rem] lg:min-h-[8.5rem]">
              <p className="text-[11px] uppercase tracking-[0.18em] text-text-secondary">
                {activeSlide.kicker}
              </p>
              <h1 className="mt-3 max-w-4xl text-[clamp(2.3rem,5vw,4.6rem)] font-semibold uppercase leading-[0.92] tracking-[-0.04em] text-text-primary line-clamp-2">
                {activeSlide.headline}
              </h1>
            </div>

            <div className="mt-7 space-y-3 text-sm text-text-secondary sm:max-w-3xl">
              <div className="min-h-[4.9rem] border-l border-accent/30 pl-3">
                <p className="leading-6 line-clamp-2">
                  {truncateDescription(activeSlide.body)}
                </p>
                <button
                  type="button"
                  onClick={() => {
                    setOpenSlide(activeSlide);
                    setIsStoryOpen(true);
                  }}
                  className="mt-1 text-[11px] uppercase tracking-[0.16em] text-accent transition-colors hover:text-accent/80"
                >
                  Read article
                </button>
              </div>
              <p className="border-l border-accent/30 pl-3 leading-6 truncate">
                {activeSlide.lines[1]}
              </p>
              <p className="border-l border-accent/30 pl-3 leading-6 truncate">
                {activeSlide.lines[2]}
              </p>
            </div>

            <div className="mt-auto pt-5">
              <div className="flex flex-wrap items-center gap-3">
                <Link href={buildDraftHref(activeSlide)}>
                  <Button variant="outline" className="border-accent text-accent hover:bg-accent/10">
                    Draft market
                  </Button>
                </Link>
                <a
                  href={activeSlide.sourceUrl}
                  target="_blank"
                  rel="noreferrer"
                  className="inline-flex h-10 items-center border border-border px-4 text-sm uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
                >
                  Original source
                </a>
                <span className="text-[11px] uppercase tracking-[0.16em] text-text-muted">
                  Three draft questions available
                </span>
              </div>

              <div className="mt-5 flex items-center gap-3 lg:hidden">
                <button
                  type="button"
                  onClick={() => setActiveIndex((current) => (current - 1 + safeSlides.length) % safeSlides.length)}
                  className="inline-flex h-11 w-11 items-center justify-center border border-border text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
                  aria-label="Previous story"
                >
                  <ArrowLeft className="h-4 w-4" />
                </button>
                <button
                  type="button"
                  onClick={() => setActiveIndex((current) => (current + 1) % safeSlides.length)}
                  className="inline-flex h-11 w-11 items-center justify-center border border-border text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
                  aria-label="Next story"
                >
                  <ArrowRight className="h-4 w-4" />
                </button>
              </div>
            </div>
          </div>
        </div>

        <aside className="hidden h-full max-h-[500px] overflow-hidden border border-border bg-bg-primary px-5 py-5 brutal-shadow lg:flex lg:flex-col sm:px-6 sm:py-6">
          <div className="shrink-0">
            <p className="text-[11px] uppercase tracking-[0.18em] text-text-muted">Coverage</p>
            <div className="mt-4 flex items-center gap-3">
              <button
                type="button"
                onClick={() => setActiveIndex((current) => (current - 1 + safeSlides.length) % safeSlides.length)}
                className="inline-flex h-11 w-11 items-center justify-center border border-border text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
                aria-label="Previous story"
              >
                <ArrowLeft className="h-4 w-4" />
              </button>
              <button
                type="button"
                onClick={() => setActiveIndex((current) => (current + 1) % safeSlides.length)}
                className="inline-flex h-11 w-11 items-center justify-center border border-border text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
                aria-label="Next story"
              >
                <ArrowRight className="h-4 w-4" />
              </button>
            </div>
          </div>

          <div className="mt-6 flex-1 space-y-2 overflow-y-auto pr-1">
            {safeSlides.map((slide, index) => (
              <button
                key={slide.id}
                type="button"
                onClick={() => setActiveIndex(index)}
                className={cn(
                  'block w-full border px-3 py-3 text-left transition-colors',
                  index === activeIndex
                    ? 'border-accent bg-accent/10 text-text-primary'
                    : 'border-border text-text-secondary hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary'
                )}
              >
                <p className="text-[11px] uppercase tracking-[0.16em] text-text-muted">{slide.kicker}</p>
                <p className="mt-2 text-sm uppercase tracking-[0.08em] line-clamp-2">{slide.headline}</p>
              </button>
            ))}
          </div>
        </aside>
      </div>

      <Dialog
        open={isStoryOpen}
        onOpenChange={(nextOpen) => {
          setIsStoryOpen(nextOpen);
          if (!nextOpen) {
            setOpenSlide(null);
          }
        }}
      >
        <DialogContent className="max-h-[85vh] max-w-3xl overflow-hidden border border-border bg-bg-primary p-0 brutal-shadow">
          <div className="flex max-h-[85vh] flex-col">
            <DialogHeader className="border-b border-border px-6 py-5 text-left">
              <p className="text-[11px] uppercase tracking-[0.18em] text-text-muted">
                {modalSlide.kicker}
              </p>
              <DialogTitle className="mt-3 text-2xl font-semibold uppercase leading-tight tracking-[-0.03em] text-text-primary sm:text-3xl">
                {modalSlide.headline}
              </DialogTitle>
            </DialogHeader>

            <div className="overflow-y-auto px-6 py-5">
              <p className="whitespace-pre-wrap text-base leading-7 text-text-secondary">
                {modalSlide.body}
              </p>

              <div className="mt-5 space-y-3 border-t border-border pt-5 text-sm text-text-secondary">
                <p>{modalSlide.lines[1]}</p>
                <p>{modalSlide.lines[2]}</p>
              </div>
            </div>

            <div className="flex flex-wrap items-center gap-3 border-t border-border px-6 py-4">
              <Link href={buildDraftHref(modalSlide)}>
                <Button variant="outline" className="border-accent text-accent hover:bg-accent/10">
                  Draft market
                </Button>
              </Link>
              <a
                href={modalSlide.sourceUrl}
                target="_blank"
                rel="noreferrer"
                className="inline-flex h-10 items-center border border-border px-4 text-sm uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
              >
                Original source
              </a>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </section>
  );
}
