import { fetchLiveBaseMarkets } from '@/lib/server/baseMarketData';

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
  const primary =
    process.env.API_PROXY_TARGET?.trim()
    || process.env.NEXT_PUBLIC_API_URL?.trim()
    || 'http://localhost:8080/v1';
  const fallback = process.env.NEXT_PUBLIC_API_FALLBACK_URL?.trim() || '';
  return [...new Set([primary, fallback].filter(Boolean))];
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
  const markets = await fetchLiveBaseMarkets({ limit: 32, revalidateSeconds: 60 });
  let capabilities: SignalCapabilities | null = null;

  for (const base of bases) {
    const capabilitiesResult = await fetchCapabilitiesFromBase(base);
    if (capabilitiesResult) {
      capabilities = capabilitiesResult;
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

function draftHeadlineTopic(value: string): string {
  return trimSentence(value.replace(/["]+/g, ''), 72);
}

function buildDraftQuestion(prefix: string, headline: string, deadline: Date): string {
  const topic = draftHeadlineTopic(headline);
  return trimSentence(`${prefix} "${topic}" by ${formatQuestionDate(deadline)}?`, 180);
}

function buildMarketDrafts(story: FeedStory, headline: string): MarketDraftOption[] {
  const deadline = buildDraftDeadline(story.publishedAt);
  const deadlineLabel = formatLongDeadline(deadline);
  const category = inferCategory(story);
  const sourceLabel = `${story.source} ${story.section.toLowerCase()} feed`;
  const drafts: MarketDraftOption[] = [
    {
      id: 'official-confirmation',
      label: 'Official confirmation',
      summary: 'An official source confirms the core claim behind the story.',
      question: buildDraftQuestion(
        'Will an official source materially confirm the core claim behind',
        headline,
        deadline
      ),
      description:
        `Generated from live news. Resolve YES if a government, company, court, or institutional source materially confirms the core claim behind "${headline}" by ${deadlineLabel}. ` +
        `Resolve NO otherwise. Primary resolution source: official statements, filings, or institutional releases. Seed source: ${story.link}`,
      resolutionSource: 'official',
      tradingEnd: deadline.toISOString(),
      category,
    },
    {
      id: 'major-outlet-follow-up',
      label: 'Major outlet follow-up',
      summary: 'A top-tier outlet advances the story with substantive new reporting.',
      question: buildDraftQuestion(
        'Will Reuters, BBC, AP, or NYT publish a substantive follow-up on',
        headline,
        deadline
      ),
      description:
        `Generated from live news. Resolve YES if Reuters, BBC, AP, or The New York Times publishes a new follow-up article that materially advances this story by ${deadlineLabel}. ` +
        `Resolve NO otherwise. Primary resolution source: named outlet coverage and archived article URLs. Seed source: ${story.link}`,
      resolutionSource: 'news',
      tradingEnd: deadline.toISOString(),
      category,
    },
    {
      id: 'official-reversal',
      label: 'Official reversal',
      summary: 'An official source denies, reverses, or materially contradicts the claim.',
      question: buildDraftQuestion(
        'Will an official source materially deny or reverse the core claim behind',
        headline,
        deadline
      ),
      description:
        `Generated from live news. Resolve YES if an official source materially denies, reverses, or contradicts the core claim behind "${headline}" by ${deadlineLabel}. ` +
        `Resolve NO otherwise. Primary resolution source: official statements, filings, or institutional releases. Seed source: ${story.link}`,
      resolutionSource: 'official',
      tradingEnd: deadline.toISOString(),
      category,
    },
  ];

  return drafts.map((draft) => ({
    ...draft,
    description: `${draft.description} Draft context: ${sourceLabel}.`,
  }));
}

function cleanHeadline(title: string, source: string): string {
  const clean = cleanXmlText(title);
  const sourceSuffix = ` - ${source}`;
  if (clean.endsWith(sourceSuffix)) {
    return clean.slice(0, -sourceSuffix.length).trim();
  }
  return clean;
}

function parseFeedStories(xml: string, feed: RssFeedConfig): FeedStory[] {
  const items = [...xml.matchAll(/<item\b[^>]*>([\s\S]*?)<\/item>/gi)];

  return items
    .map((match) => {
      const item = match[1];
      const title = cleanHeadline(extractTag(item, 'title'), feed.source);
      const description = cleanXmlText(
        extractTag(item, 'description') || extractTag(item, 'content:encoded')
      );
      const link = cleanXmlText(extractTag(item, 'link'));
      const publishedAt = parsePublishedAt(cleanXmlText(extractTag(item, 'pubDate')));

      if (!title || !link) {
        return null;
      }

      return {
        title,
        description,
        link,
        source: feed.source,
        section: feed.section,
        publishedAt,
        weight: feed.weight,
      };
    })
    .filter((story): story is FeedStory => Boolean(story));
}

async function fetchFeedStories(feed: RssFeedConfig): Promise<FeedStory[]> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), FEED_TIMEOUT_MS);

  try {
    const response = await fetch(feed.url, {
      headers: {
        Accept: 'application/rss+xml, application/xml, text/xml;q=0.9',
        'User-Agent': 'relay44-web/1.0',
      },
      signal: controller.signal,
      next: { revalidate: 900 },
      cache: 'force-cache',
    });

    if (!response.ok) {
      return [];
    }

    const xml = await response.text();
    return parseFeedStories(xml, feed);
  } catch {
    return [];
  } finally {
    clearTimeout(timeout);
  }
}

function scoreStory(story: FeedStory): number {
  const text = `${story.title} ${story.description}`.toLowerCase();
  const ageHours = story.publishedAt
    ? Math.max(0, (Date.now() - story.publishedAt.getTime()) / 3_600_000)
    : 24;

  let score = story.weight * 10;
  score += Math.max(0, 48 - ageHours) * 1.4;
  score += Math.min(story.title.length / 18, 6);
  score += Math.min(story.description.length / 45, 5);

  for (const [pattern, weight] of IMPACT_PATTERNS) {
    if (pattern.test(text)) {
      score += weight;
    }
  }

  if (/live updates?/i.test(story.title)) {
    score += 6;
  }

  return score;
}

function buildNewsSlide(story: FeedStory): NewsSlide {
  const headline = trimSentence(story.title, 108);
  const description = story.description || `${story.source} added this story to current coverage.`;
  const body = description.replace(/\s+/g, ' ').trim();
  const relativeTime = formatRelativeTime(story.publishedAt);

  return {
    id: normalizeTitle(headline).slice(0, 64).replace(/\s+/g, '-'),
    kicker: `${story.source.toUpperCase()} // ${story.section.toUpperCase()}`,
    headline,
    body,
    lines: [
      trimSentence(body, 120),
      `${story.source} ${story.section.toLowerCase()} desk pushed this ${relativeTime}.`,
      'Outcome angle: price whether this narrative intensifies, resolves, or fades over the next 7 days.',
    ],
    sourceUrl: story.link,
    marketDrafts: buildMarketDrafts(story, headline),
  };
}

function buildFallbackSlide(id: string, headline: string, note: string): NewsSlide {
  const fallbackStory: FeedStory = {
    title: headline,
    description: note,
    link: '#',
    source: 'Desk',
    section: 'Fallback',
    publishedAt: null,
    weight: 0,
  };

  return {
    id,
    kicker: 'WORLD DESK // FALLBACK',
    headline,
    body:
      `${note} The desk keeps fallback copy live so the homepage still carries a usable narrative layer while upstream feeds recover.`,
    lines: [
      note,
      'The desk keeps a stable fallback so the homepage never collapses into empty chrome.',
      'Outcome angle: turn the prevailing narrative into a concrete market question.',
    ],
    sourceUrl: '#',
    marketDrafts: buildMarketDrafts(fallbackStory, headline),
  };
}

async function fetchWorldDeskNews(): Promise<NewsSlide[]> {
  if (newsCache && newsCache.expiresAt > Date.now()) {
    return newsCache.value;
  }

  const results = await Promise.allSettled(RSS_FEEDS.map((feed) => fetchFeedStories(feed)));
  const stories = results.flatMap((result) => (result.status === 'fulfilled' ? result.value : []));
  const seen = new Set<string>();
  const rankedSlides = stories
    .filter((story) => isMarketSafeStory(story))
    .sort((left, right) => scoreStory(right) - scoreStory(left))
    .filter((story) => {
      const key = normalizeTitle(story.title);
      if (!key || seen.has(key)) {
        return false;
      }
      seen.add(key);
      return true;
    })
    .slice(0, 6)
    .map((story) => buildNewsSlide(story));

  const value =
    rankedSlides.length === 6
      ? rankedSlides
      : [...rankedSlides, ...NEWS_FALLBACKS].slice(0, 6);

  newsCache = {
    value,
    expiresAt: Date.now() + NEWS_CACHE_TTL_MS,
  };

  return value;
}

export async function getHomeLiveFeed(): Promise<HomeLiveFeed> {
  const [news, signal] = await Promise.all([
    fetchWorldDeskNews(),
    fetchSignalSnapshot(),
  ]);

  return {
    news,
    signal,
    fetchedAt: new Date().toISOString(),
  };
}
