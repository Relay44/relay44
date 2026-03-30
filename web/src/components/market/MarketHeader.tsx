import { MarketArtwork } from '@/components/market/MarketArtwork';
import { Badge } from '@/components/ui';
import { MARKET_STATUS_LABELS } from '@/lib/constants';
import { isTradingViewUrl } from '@/lib/tradingView';
import type { Market } from '@/types';

function Linkify({
  text,
  suppressTradingViewLinks = false,
}: {
  text: string;
  suppressTradingViewLinks?: boolean;
}) {
  const urlRegex = /(https?:\/\/[^\s)]+)/g;
  const parts = text.split(urlRegex);
  let tradingViewNoticeRendered = false;

  return (
    <>
      {parts.map((part, i) => {
        if (!part.match(urlRegex)) {
          return <span key={i}>{part}</span>;
        }

        if (suppressTradingViewLinks && isTradingViewUrl(part)) {
          if (tradingViewNoticeRendered) {
            return null;
          }

          tradingViewNoticeRendered = true;
          return (
            <span key={i} className="text-text-secondary">
              TradingView reference chart embedded below.
            </span>
          );
        }

        return (
          <a
            key={i}
            href={part}
            target="_blank"
            rel="noopener noreferrer"
            className="text-accent hover:text-accent-hover underline underline-offset-2"
          >
            {part}
          </a>
        );
      })}
    </>
  );
}

export interface MarketHeaderProps {
  market: Market;
  suppressTradingViewLinks?: boolean;
}

export function MarketHeader({
  market,
  suppressTradingViewLinks = false,
}: MarketHeaderProps) {
  // Use accent for active, muted for others - no harsh green/red
  const statusVariant =
    market.status === 'active'
      ? 'accent'
      : market.status === 'resolved'
        ? 'default'
        : 'muted';

  return (
    <div className="mb-6 flex flex-col gap-4 sm:flex-row sm:items-start">
      <MarketArtwork
        market={market}
        className="h-24 w-24 shrink-0 sm:h-28 sm:w-28"
        sizes="112px"
        priority
      />
      <div className="min-w-0 flex-1">
        <div className="mb-3 flex flex-wrap items-center gap-2">
          <Badge variant="muted">{market.category}</Badge>
          <Badge variant={market.isExternal ? 'accent' : 'muted'}>
            {market.provider}
          </Badge>
          <Badge variant="muted">
            {market.chainId === 137 ? 'polygon' : market.chainId === 8453 ? 'base' : `chain-${market.chainId}`}
          </Badge>
          <Badge variant={statusVariant}>
            {MARKET_STATUS_LABELS[market.status]}
          </Badge>
          {market.liquidityMode === 'bootstrap_hybrid' ? (
            <Badge variant={market.bootstrapActive ? 'accent' : 'muted'}>
              {market.bootstrapActive ? 'bootstrap active' : 'bootstrap graduated'}
            </Badge>
          ) : null}
        </div>
        <h1 className="mb-2 text-xl font-bold sm:text-2xl">{market.question}</h1>
        <p className="text-sm leading-6 text-text-secondary">
          <Linkify
            text={market.description || ''}
            suppressTradingViewLinks={suppressTradingViewLinks}
          />
        </p>
      </div>
    </div>
  );
}
