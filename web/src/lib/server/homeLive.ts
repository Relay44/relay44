import {
  normalizeBaseMarketsResponse,
  type BaseMarketsResponse,
} from '@/lib/api';
import type { Market, PaginatedResponse } from '@/types';

export interface MarketDraftOption {
  id: string;
  label: string;
  summary: string;
  question: string;
  description: string;
  resolutionSource: 'official' | 'news' | 'custom';
  customSource?: string;
  tradingEnd: string;
  category: string;
}

export interface NewsSlide {
  id: string;
  kicker: string;
  headline: string;
  body: string;
  lines: [string, string, string];
  sourceUrl: string;
  marketDrafts: MarketDraftOption[];
}

export interface SignalSnapshot {
  label: string;
  source: string;
  latencyMs: number;
  marketsTracked: number;
  feedsLive: number;
  feedsExpected: number;
  stageLabel: string;
  updatedAt: string;
  points: number[];
}

export interface HomeLiveFeed {
  news: NewsSlide[];
  signal: SignalSnapshot;
  fetchedAt: string;
}

interface CacheEntry<T> {
  expiresAt: number;
  value: T;
}

interface RssFeedConfig {
  url: string;
  source: string;
  section: string;
  weight: number;
}

interface FeedStory {
  title: string;
  description: string;
  link: string;
  source: string;
  section: string;
  publishedAt: Date | null;
  weight: number;
}

interface SignalCapabilities {
  runtime: {
    limitless_enabled: boolean;
    polymarket_enabled: boolean;
  };
  launch?: {
    beta: boolean;
    limitless_trading_ready: boolean;
    polymarket_trading_ready: boolean;
  };
}

const NEWS_CACHE_TTL_MS = 15 * 60 * 1000;
const SIGNAL_CACHE_TTL_MS = 60 * 1000;
const DEFAULT_API_BASE = 'http://localhost:8080/v1';
const FEED_TIMEOUT_MS = 4_000;
const RSS_FEEDS: RssFeedConfig[] = [
  {
    url: 'https://feeds.bbci.co.uk/news/world/rss.xml',
    source: 'BBC',
    section: 'World',
    weight: 12,
  },
  {
    url: 'https://feeds.bbci.co.uk/news/business/rss.xml',
    source: 'BBC',
    section: 'Business',
    weight: 9,
  },
  {
    url: 'https://feeds.bbci.co.uk/news/technology/rss.xml',
    source: 'BBC',
    section: 'Technology',
    weight: 8,
  },
  {
    url: 'https://feeds.npr.org/1004/rss.xml',
    source: 'NPR',
    section: 'World',
    weight: 10,
  },
  {
    url: 'https://rss.nytimes.com/services/xml/rss/nyt/World.xml',
    source: 'NYT',
    section: 'World',
    weight: 10,
  },
];
const IMPACT_PATTERNS: Array<[RegExp, number]> = [
  [/\b(ai|artificial intelligence|openai|chip|chips)\b/i, 10],
  [/\b(iran|ukraine|russia|china|israel|gaza|taiwan)\b/i, 9],
  [/\b(war|strike|ceasefire|drone|missile|attack)\b/i, 9],
  [/\b(election|vote|court|supreme|tariff|sanction|policy)\b/i, 8],
  [/\b(oil|gas|energy|inflation|rates?|fed|market|economy)\b/i, 7],
  [/\b(outage|earthquake|storm|flood|wildfire|crash)\b/i, 7],
];
const SENSITIVE_TRAGEDY_PATTERNS = [
  /\b(school shooting|mass shooting|terror attack|suicide bombing|ethnic cleansing|genocide)\b/i,
  /\b(rape|sexual assault|child abuse|human trafficking|child exploitation|grooming)\b/i,
  /\b(epstein|sex offender|pedoph\w*)\b/i,
  /\b(morning rundown|morning edition|newsletter|daily brief|up first)\b/i,
  /\.\s+And,\s+/i,
  /\b(family|families|parent|parents|mother|father|child|children|student|students|civilian|civilians|hostage|hostages|victim|victims|survivor|survivors|bereaved|migrant workers)\b.*\b(killed|dead|death|deaths|fatal|fatalities|shot|shooting|injured|wounded|slain|murdered|massacred|abducted|grieving|mourning)\b/i,
  /\b(killed|dead|death|deaths|fatal|fatalities|shot|shooting|injured|wounded|slain|murdered|massacred|abducted|grieving|mourning)\b.*\b(family|families|parent|parents|mother|father|child|children|student|students|civilian|civilians|hostage|hostages|victim|victims|survivor|survivors|bereaved|migrant workers)\b/i,
  /\b(funeral|memorial|mourning|grief|grieving|survivors?|bereaved)\b.*\b(school shooting|mass shooting|terror attack|bombing|massacre|killed|dead|death|fatalities)\b/i,
  /\b(deadly|fatal)\b.*\b(school|children|child|civilians?|famil(?:y|ies)|parents?|students?|victims?|survivors?)\b/i,
];
const NEWS_FALLBACKS: NewsSlide[] = [
  buildFallbackSlide(
    'fallback-1',
    'World desks are repricing war, rates, and AI policy at the same time.',
    'Fallback mode stays live if upstream feeds stall.'
  ),
  buildFallbackSlide(
    'fallback-2',
    'Election risk, energy pressure, and model-policy fights are colliding.',
    'Fallback mode stays live if upstream feeds stall.'
  ),
  buildFallbackSlide(
    'fallback-3',
    'Capital is still reacting fastest to conflict, chips, and central-bank signals.',
    'Fallback mode stays live if upstream feeds stall.'
  ),
  buildFallbackSlide(
    'fallback-4',
    'The desk is built to stay useful even when one or more live feeds go dark.',
    'Fallback mode stays live if upstream feeds stall.'
  ),
  buildFallbackSlide(
    'fallback-5',
    'The outcome layer remains the point: turn headlines into clear, tradeable questions.',
    'Fallback mode stays live if upstream feeds stall.'
  ),
  buildFallbackSlide(
    'fallback-6',
    'When the feed cools down, the market framing still gives users a decision surface.',
    'Fallback mode stays live if upstream feeds stall.'
  ),
];

let newsCache: CacheEntry<NewsSlide[]> | null = null;
let signalCache: CacheEntry<SignalSnapshot> | null = null;

function getApiBases(): string[] {
  const primary = process.env.NEXT_PUBLIC_API_URL?.trim() || DEFAULT_API_BASE;
  const fallback = process.env.NEXT_PUBLIC_API_FALLBACK_URL?.trim() || '';
  return [...new Set([primary, fallback].filter(Boolean))];
}

async function fetchMarketsFromBase(
  base: string,
  limit = 24
): Promise<PaginatedResponse<Market> | null> {
  const query = new URLSearchParams({
    limit: String(limit),
    offset: '0',
    source: 'all',
    tradable: 'all',
  });
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 3000);

  try {
    const res = await fetch(`${base}/evm/markets?${query.toString()}`, {
      method: 'GET',
      signal: controller.signal,
      next: { revalidate: 60 },
    });
    if (!res.ok) {
      return null;
    }

    const payload = (await res.json()) as BaseMarketsResponse;
    if (!Array.isArray(payload.markets)) {
      return null;
    }

    return normalizeBaseMarketsResponse(payload);
  } catch {
    return null;
  } finally {
    clearTimeout(timeout);
  }
}

async function fetchCapabilitiesFromBase(base: string): Promise<SignalCapabilities | null> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 3000);

  try {
    const res = await fetch(`${base}/web4/capabilities`, {
      method: 'GET',
      signal: controller.signal,
      next: { revalidate: 60 },
    });
    if (!res.ok) {
      return null;
    }

    const payload = (await res.json()) as SignalCapabilities;
    if (!payload.runtime) {
      return null;
    }

    return payload;
  } catch {
    return null;
  } finally {
    clearTimeout(timeout);
  }
}

async function fetchSignalSnapshot(): Promise<SignalSnapshot> {
  if (signalCache && signalCache.expiresAt > Date.now()) {
    return signalCache.value;
  }

  const startedAt = Date.now();
  const bases = getApiBases();
  let markets: PaginatedResponse<Market> | null = null;
  let capabilities: SignalCapabilities | null = null;

  for (const base of bases) {
    const [marketResult, capabilitiesResult] = await Promise.all([
      fetchMarketsFromBase(base, 32),
      fetchCapabilitiesFromBase(base),
    ]);

    if (!capabilities && capabilitiesResult) {
      capabilities = capabilitiesResult;
    }

    if (marketResult) {
      markets = marketResult;
    }

    if (marketResult?.data.length) {
      if (capabilitiesResult) {
        capabilities = capabilitiesResult;
      }
      break;
    }
  }

  const now = new Date().toISOString();
  const marketData = markets?.data ?? [];
  const rankedMarkets = [...marketData].sort((left, right) => {
    const leftScore = left.volume24h + left.totalVolume * 0.05;
    const rightScore = right.volume24h + right.totalVolume * 0.05;
    return rightScore - leftScore;
  });
  const points = rankedMarkets.slice(0, 24).map((market) => {
    const probability = Number.isFinite(market.yesPrice) ? market.yesPrice : 0.5;
    const volumeWeight = Math.min(12, Math.round(Math.log10((market.volume24h || 1) + 1) * 3));
    return Math.max(4, Math.min(96, Math.round(probability * 100) + volumeWeight - 6));
  });

  while (points.length < 24) {
    points.push(50);
  }

  const providers = [
    ...new Set(
      rankedMarkets
        .map((market) => market.provider?.trim().toLowerCase())
        .filter((provider): provider is string => Boolean(provider))
    ),
  ];
  const expectedProviders = [
    capabilities?.runtime.limitless_enabled !== false ? 'limitless' : null,
    capabilities?.runtime.polymarket_enabled !== false ? 'polymarket' : null,
  ].filter((provider): provider is string => Boolean(provider));
  const liveProviders = expectedProviders.filter((provider) =>
    providers.includes(provider)
  );
  const sourceLabel =
    liveProviders.length > 0
      ? `feeds: ${liveProviders.join(' + ')}`
      : expectedProviders.length > 0
        ? 'feeds: standby'
        : 'feeds: unavailable';

  const snapshot: SignalSnapshot = {
    label: 'MARKET_GRID',
    source: sourceLabel,
    latencyMs: Date.now() - startedAt,
    marketsTracked: rankedMarkets.length,
    feedsLive: liveProviders.length,
    feedsExpected: expectedProviders.length || providers.length,
    stageLabel: (capabilities?.launch?.beta ?? true) ? 'beta' : 'live',
    updatedAt: now,
    points,
  };

  signalCache = {
    value: snapshot,
    expiresAt: Date.now() + SIGNAL_CACHE_TTL_MS,
  };

  return snapshot;
}

function normalizeTitle(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, ' ').trim();
}

function trimSentence(value: string, limit: number): string {
  const clean = value.replace(/\s+/g, ' ').trim();
  if (clean.length <= limit) {
    return clean;
  }

  return `${clean.slice(0, limit - 1).trim()}...`;
}

function decodeHtmlEntities(value: string): string {
  const entityMap: Record<string, string> = {
    '&amp;': '&',
    '&lt;': '<',
    '&gt;': '>',
    '&quot;': '"',
    '&apos;': "'",
    '&nbsp;': ' ',
  };

  return value
    .replace(/<!\[CDATA\[([\s\S]*?)\]\]>/g, '$1')
    .replace(/&#(\d+);/g, (_, digits: string) => String.fromCharCode(Number(digits)))
    .replace(/&#x([0-9a-f]+);/gi, (_, digits: string) =>
      String.fromCharCode(parseInt(digits, 16))
    )
    .replace(/&(amp|lt|gt|quot|apos|nbsp);/g, (match) => entityMap[match] || match);
}

function stripTags(value: string): string {
  return value.replace(/<[^>]*>/g, ' ');
}

function sanitizeCopy(value: string): string {
  return value
    .replace(/[\u2018\u2019]/g, "'")
    .replace(/[\u201C\u201D]/g, '"')
    .replace(/[\u2013\u2014]/g, '-')
    .replace(/[\u2022\u2028\u2029]/g, ' ')
    .replace(/\u00a0/g, ' ');
}

function cleanXmlText(value: string): string {
  return sanitizeCopy(stripTags(decodeHtmlEntities(value))).replace(/\s+/g, ' ').trim();
}

function extractTag(itemXml: string, tagName: string): string {
  const escaped = tagName.replace(':', '\\:');
  const match = itemXml.match(new RegExp(`<${escaped}(?:\\s[^>]*)?>([\\s\\S]*?)</${escaped}>`, 'i'));
  return match?.[1] ?? '';
}

function parsePublishedAt(value: string): Date | null {
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

function formatRelativeTime(value: Date | null): string {
  if (!value) {
    return 'just now';
  }

  const diffMs = Date.now() - value.getTime();
  const diffMinutes = Math.max(1, Math.round(diffMs / 60_000));
  if (diffMinutes < 60) {
    return `${diffMinutes}m ago`;
  }

  const diffHours = Math.round(diffMinutes / 60);
  if (diffHours < 48) {
    return `${diffHours}h ago`;
  }

  return `${Math.round(diffHours / 24)}d ago`;
}

function isMarketSafeStory(story: FeedStory): boolean {
  const text = `${story.title} ${story.description}`.toLowerCase();
  return !SENSITIVE_TRAGEDY_PATTERNS.some((pattern) => pattern.test(text));
}

function inferCategory(story: FeedStory): string {
  const text = `${story.title} ${story.description} ${story.section}`.toLowerCase();

  if (story.section === 'Technology' || /\b(ai|openai|chip|chips|model|software|tech)\b/.test(text)) {
    return 'tech';
  }

  if (story.section === 'Business' || /\b(oil|gas|energy|rates?|market|stocks?|economy|inflation|bank)\b/.test(text)) {
    return 'finance';
  }

  if (/\b(war|strike|ceasefire|election|policy|court|government|president|minister|iran|ukraine|israel|china|russia)\b/.test(text)) {
    return 'politics';
  }

  return 'other';
}

function buildDraftDeadline(publishedAt: Date | null): Date {
  const base = publishedAt ? new Date(publishedAt) : new Date();
  const deadline = new Date(base);
  deadline.setUTCDate(deadline.getUTCDate() + 7);
  deadline.setUTCHours(23, 59, 0, 0);
  return deadline;
}

function formatQuestionDate(value: Date): string {
  return value.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    timeZone: 'UTC',
  });
}

function formatLongDeadline(value: Date): string {
  return value.toLocaleString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
    timeZone: 'UTC',
  }) + ' UTC';
}
