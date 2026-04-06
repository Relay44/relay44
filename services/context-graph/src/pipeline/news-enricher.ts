import type { MarketContext } from '../types/polymarket.js';
import type { Source } from '../types/graph.js';

export interface NewsSignals {
  articleCount: number;
  uniqueDomains: number;
  sourceDiversity: number;
  sources: Source[];
  factCheckResults: Array<{
    url: string;
    verdict: string;
    source: string;
  }>;
}

const KNOWN_RELIABLE_DOMAINS = new Set([
  'reuters.com', 'apnews.com', 'bbc.com', 'bbc.co.uk', 'nytimes.com',
  'washingtonpost.com', 'theguardian.com', 'bloomberg.com', 'ft.com',
  'economist.com', 'npr.org', 'pbs.org', 'aljazeera.com',
]);

const KNOWN_FACTCHECK_DOMAINS = new Set([
  'snopes.com', 'factcheck.org', 'politifact.com', 'fullfact.org',
  'reuters.com/fact-check', 'apnews.com/hub/ap-fact-check',
]);

const LOW_CREDIBILITY_DOMAINS = new Set([
  'infowars.com', 'naturalnews.com', 'beforeitsnews.com', 'zerohedge.com',
]);

export class NewsEnricher {
  async crossReference(market: MarketContext): Promise<NewsSignals> {
    const query = this.buildSearchQuery(market);
    const articles = await this.searchNews(query);
    const factChecks = await this.searchFactChecks(market.question);

    const domains = new Set(articles.map((a) => this.extractDomain(a.url)));

    return {
      articleCount: articles.length,
      uniqueDomains: domains.size,
      sourceDiversity: domains.size / Math.max(articles.length, 1),
      sources: articles,
      factCheckResults: factChecks,
    };
  }

  private buildSearchQuery(market: MarketContext): string {
    // Remove common prediction market phrasing
    const cleaned = market.question
      .replace(/^will /i, '')
      .replace(/\?$/, '')
      .replace(/before [\w\s]+\d{4}/i, '')
      .trim();

    return cleaned;
  }

  private async searchNews(query: string): Promise<Source[]> {
    // Use a news API (NewsAPI, Google News, or web search)
    // Falls back gracefully when no API key is configured
    const apiKey = process.env.NEWS_API_KEY;

    if (apiKey) {
      return this.searchViaNewsApi(query, apiKey);
    }

    // Fallback: use a lightweight web search
    return this.searchViaWebSearch(query);
  }

  private async searchViaNewsApi(query: string, apiKey: string): Promise<Source[]> {
    try {
      const res = await fetch(
        `https://newsapi.org/v2/everything?q=${encodeURIComponent(query)}&sortBy=relevancy&pageSize=20&language=en`,
        { headers: { 'X-Api-Key': apiKey } },
      );

      if (!res.ok) return [];

      const data = await res.json();
      return (data.articles || []).map((article: any, i: number) => ({
        id: `news:${i}:${this.hashString(article.url || '')}`,
        url: article.url || '',
        platform: 'news' as const,
        author: article.author || article.source?.name || 'Unknown',
        publishedAt: article.publishedAt || new Date().toISOString(),
        title: article.title || '',
        snippet: article.description || '',
        engagementMetrics: {},
        credibilityScore: this.scoreDomain(article.url || ''),
        biasIndicators: [],
      }));
    } catch {
      return [];
    }
  }

  private async searchViaWebSearch(query: string): Promise<Source[]> {
    // Lightweight fallback — in production, integrate with a search API
    // Returns empty when no external API is available
    return [];
  }

  private async searchFactChecks(question: string): Promise<Array<{
    url: string;
    verdict: string;
    source: string;
  }>> {
    // Google Fact Check API
    const apiKey = process.env.GOOGLE_FACTCHECK_API_KEY;
    if (!apiKey) return [];

    try {
      const res = await fetch(
        `https://factchecktools.googleapis.com/v1alpha1/claims:search?query=${encodeURIComponent(question)}&key=${apiKey}&pageSize=5`,
      );

      if (!res.ok) return [];

      const data = await res.json();
      return (data.claims || []).map((claim: any) => {
        const review = claim.claimReview?.[0] || {};
        return {
          url: review.url || '',
          verdict: review.textualRating || 'Unknown',
          source: review.publisher?.name || 'Unknown',
        };
      });
    } catch {
      return [];
    }
  }

  private scoreDomain(url: string): number {
    const domain = this.extractDomain(url);
    if (KNOWN_FACTCHECK_DOMAINS.has(domain)) return 95;
    if (KNOWN_RELIABLE_DOMAINS.has(domain)) return 85;
    if (LOW_CREDIBILITY_DOMAINS.has(domain)) return 15;

    // Unknown domain — moderate score
    return 50;
  }

  private extractDomain(url: string): string {
    try {
      return new URL(url).hostname.replace(/^www\./, '');
    } catch {
      return 'unknown';
    }
  }

  private hashString(str: string): string {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      const char = str.charCodeAt(i);
      hash = ((hash << 5) - hash) + char;
      hash |= 0;
    }
    return Math.abs(hash).toString(36);
  }
}
