const URL_PATTERN = /(https?:\/\/[^\s)]+)/g;
const TRADINGVIEW_HOST_PATTERN = /(^|\.)tradingview\.com$/i;

function normalizeUrl(candidate: string): string {
  return candidate.replace(/[.,]+$/, "");
}

export interface TradingViewReference {
  symbol: string;
  url: string;
}

export function extractTradingViewReference(
  text: string | null | undefined
): TradingViewReference | null {
  const matches = text?.match(URL_PATTERN) || [];

  for (const match of matches) {
    const urlValue = normalizeUrl(match);

    try {
      const url = new URL(urlValue);
      if (!TRADINGVIEW_HOST_PATTERN.test(url.hostname)) {
        continue;
      }

      const symbol = url.searchParams.get("symbol")?.trim();
      if (!symbol) {
        continue;
      }

      return {
        symbol: decodeURIComponent(symbol),
        url: urlValue,
      };
    } catch {
      continue;
    }
  }

  return null;
}

export function isTradingViewUrl(candidate: string): boolean {
  try {
    const url = new URL(normalizeUrl(candidate));
    return TRADINGVIEW_HOST_PATTERN.test(url.hostname);
  } catch {
    return false;
  }
}
