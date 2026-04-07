import type { Metadata } from 'next';
import type { LeaderboardEntry, Market, PublicProfile } from '@/types';

export const SITE_NAME = 'Relay44';
export const SITE_URL = (process.env.NEXT_PUBLIC_SITE_URL?.trim() || 'https://relay44.com')
  .replace(/\/$/, '');
export const SITE_HANDLE = '@relay44';
export const SITE_IMAGE_PATH = '/relay44-sharing.jpg';
export const SITE_IMAGE_ALT = 'Relay44 share image';
export const DEFAULT_DESCRIPTION =
  'Relay44 is a prediction market app on Base with live markets, agent execution, and market data across connected venues.';
export const DEFAULT_KEYWORDS = [
  'prediction markets',
  'market trading',
  'agents',
  'trading agents',
  'decision workflows',
  'forecasting markets',
  'Base prediction markets',
  'crypto prediction markets',
  'Polymarket',
  'Limitless',
  'market data',
  'event forecasting',
  'automated execution',
];

type OpenGraphType = 'website' | 'article' | 'profile';

type StructuredDataNode = Record<string, unknown>;

interface PageMetadataOptions {
  title: string;
  description: string;
  path: string;
  noIndex?: boolean;
  keywords?: string[];
  image?: string;
  openGraphType?: OpenGraphType;
}

export function absoluteUrl(path = '/') {
  if (/^https?:\/\//i.test(path)) {
    return path;
  }

  const normalized = path.startsWith('/') ? path : `/${path}`;
  return new URL(normalized, `${SITE_URL}/`).toString();
}

export function cleanText(value: string, maxLength = 160) {
  const compact = value.replace(/\s+/g, ' ').trim();
  if (compact.length <= maxLength) {
    return compact;
  }

  return `${compact.slice(0, maxLength - 1).trimEnd()}…`;
}

export function titleText(title: string) {
  return `${title} | ${SITE_NAME}`;
}

export function resolveSeoImage(image?: string) {
  if (!image) {
    return absoluteUrl(SITE_IMAGE_PATH);
  }

  if (/^https?:\/\//i.test(image)) {
    return image;
  }

  return absoluteUrl(image);
}

export function buildRobots(noIndex = false): Metadata['robots'] {
  if (noIndex) {
    return {
      index: false,
      follow: false,
      nocache: true,
      googleBot: {
        index: false,
        follow: false,
        noimageindex: true,
        'max-image-preview': 'none',
        'max-snippet': 0,
        'max-video-preview': 0,
      },
    };
  }

  return {
    index: true,
    follow: true,
    googleBot: {
      index: true,
      follow: true,
      'max-image-preview': 'large',
      'max-snippet': -1,
      'max-video-preview': -1,
    },
  };
}

export function buildPageMetadata({
  title,
  description,
  path,
  noIndex = false,
  keywords = [],
  image,
  openGraphType = 'website',
}: PageMetadataOptions): Metadata {
  const canonical = absoluteUrl(path);
  const imageUrl = resolveSeoImage(image);
  const summary = cleanText(description);

  return {
    title,
    description: summary,
    keywords: [...new Set([...DEFAULT_KEYWORDS, ...keywords])],
    alternates: {
      canonical,
    },
    openGraph: {
      title: titleText(title),
      description: summary,
      url: canonical,
      siteName: SITE_NAME,
      locale: 'en_US',
      type: openGraphType,
      images: [
        {
          url: imageUrl,
          width: 1200,
          height: 630,
          alt: SITE_IMAGE_ALT,
        },
      ],
    },
    twitter: {
      card: 'summary_large_image',
      title: titleText(title),
      description: summary,
      creator: SITE_HANDLE,
      site: SITE_HANDLE,
      images: [imageUrl],
    },
    robots: buildRobots(noIndex),
  };
}

export function buildOrganizationStructuredData(): StructuredDataNode {
  return {
    '@context': 'https://schema.org',
    '@type': 'Organization',
    '@id': absoluteUrl('/#organization'),
    name: SITE_NAME,
    url: SITE_URL,
    logo: absoluteUrl('/relay44.svg'),
    image: absoluteUrl(SITE_IMAGE_PATH),
    sameAs: ['https://x.com/Relay44BASE'],
  };
}

export function buildWebsiteStructuredData(): StructuredDataNode {
  return {
    '@context': 'https://schema.org',
    '@type': 'WebSite',
    '@id': absoluteUrl('/#website'),
    name: SITE_NAME,
    url: SITE_URL,
    description: DEFAULT_DESCRIPTION,
    inLanguage: 'en-US',
    publisher: {
      '@id': absoluteUrl('/#organization'),
    },
  };
}

export function buildWebApplicationStructuredData(): StructuredDataNode {
  return {
    '@context': 'https://schema.org',
    '@type': 'WebApplication',
    '@id': absoluteUrl('/#webapp'),
    name: SITE_NAME,
    url: SITE_URL,
    applicationCategory: 'FinanceApplication',
    operatingSystem: 'Any',
    browserRequirements: 'Requires JavaScript and a modern browser',
    description: DEFAULT_DESCRIPTION,
    image: absoluteUrl(SITE_IMAGE_PATH),
    featureList: [
      'Live prediction markets and pricing',
      'Agent management across onchain and external venues',
      'Private decision workflows',
      'Market discovery and portfolio tools',
    ],
    offers: {
      '@type': 'Offer',
      price: '0',
      priceCurrency: 'USD',
    },
    publisher: {
      '@id': absoluteUrl('/#organization'),
    },
  };
}

export function buildWebPageStructuredData({
  path,
  name,
  description,
  type = 'WebPage',
}: {
  path: string;
  name: string;
  description: string;
  type?: string;
}): StructuredDataNode {
  return {
    '@context': 'https://schema.org',
    '@type': type,
    '@id': absoluteUrl(`${path}#webpage`),
    name,
    description: cleanText(description),
    url: absoluteUrl(path),
    inLanguage: 'en-US',
    isPartOf: {
      '@id': absoluteUrl('/#website'),
    },
  };
}

export function buildBreadcrumbStructuredData(
  items: Array<{ name: string; url: string }>
): StructuredDataNode {
  return {
    '@context': 'https://schema.org',
    '@type': 'BreadcrumbList',
    itemListElement: items.map((item, index) => ({
      '@type': 'ListItem',
      position: index + 1,
      name: item.name,
      item: absoluteUrl(item.url),
    })),
  };
}

export function buildCollectionPageStructuredData({
  path,
  name,
  description,
  items,
}: {
  path: string;
  name: string;
  description: string;
  items?: Array<{ name: string; url: string }>;
}): StructuredDataNode {
  const itemListElement = (items || []).slice(0, 20).map((item, index) => ({
    '@type': 'ListItem',
    position: index + 1,
    name: item.name,
    url: absoluteUrl(item.url),
  }));

  return {
    '@context': 'https://schema.org',
    '@type': 'CollectionPage',
    '@id': absoluteUrl(`${path}#collection`),
    name,
    description: cleanText(description),
    url: absoluteUrl(path),
    isPartOf: {
      '@id': absoluteUrl('/#website'),
    },
    mainEntity: itemListElement.length
      ? {
          '@type': 'ItemList',
          itemListElement,
        }
      : undefined,
  };
}

export function buildMarketStructuredData(market: Market): StructuredDataNode {
  const keywords = [market.category, market.provider, market.source].filter(Boolean).join(', ');

  return {
    '@context': 'https://schema.org',
    '@type': 'WebPage',
    '@id': absoluteUrl(`/markets/${encodeURIComponent(market.id)}#webpage`),
    name: market.question,
    description: cleanText(market.description || DEFAULT_DESCRIPTION),
    url: absoluteUrl(`/markets/${encodeURIComponent(market.id)}`),
    datePublished: market.createdAt,
    dateModified: market.resolvedAt || market.tradingEnd || market.createdAt,
    inLanguage: 'en-US',
    isPartOf: {
      '@id': absoluteUrl('/#website'),
    },
    primaryImageOfPage: resolveSeoImage(market.imageUrl),
    keywords,
    about: {
      '@type': 'Thing',
      name: market.question,
      description: cleanText(market.description || DEFAULT_DESCRIPTION),
    },
  };
}

export function buildProfileStructuredData(
  wallet: string,
  profile?: PublicProfile | null
): StructuredDataNode {
  const name = profile?.username || `${wallet.slice(0, 6)}...${wallet.slice(-4)}`;

  return {
    '@context': 'https://schema.org',
    '@type': 'ProfilePage',
    '@id': absoluteUrl(`/profile/${wallet}#profile`),
    name: `${name} profile`,
    url: absoluteUrl(`/profile/${wallet}`),
    isPartOf: {
      '@id': absoluteUrl('/#website'),
    },
    mainEntity: {
      '@type': 'Person',
      name,
      identifier: wallet,
      description:
        cleanText(profile?.bio || '') ||
        `Trading profile, positions, and market activity for ${name} on ${SITE_NAME}.`,
      image: profile?.avatarUrl ? resolveSeoImage(profile.avatarUrl) : absoluteUrl(SITE_IMAGE_PATH),
    },
  };
}

export function buildLeaderboardStructuredData(entries: LeaderboardEntry[]): StructuredDataNode {
  return {
    '@context': 'https://schema.org',
    '@type': 'CollectionPage',
    '@id': absoluteUrl('/leaderboard#collection'),
    name: 'Trader leaderboard',
    description: 'Top traders and performance rankings on Relay44.',
    url: absoluteUrl('/leaderboard'),
    isPartOf: {
      '@id': absoluteUrl('/#website'),
    },
    mainEntity: {
      '@type': 'ItemList',
      itemListElement: entries.slice(0, 20).map((entry, index) => ({
        '@type': 'ListItem',
        position: index + 1,
        name: entry.username || `${entry.wallet.slice(0, 6)}...${entry.wallet.slice(-4)}`,
        url: absoluteUrl(`/profile/${entry.wallet}`),
      })),
    },
  };
}
