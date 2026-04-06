import type { MarketContext } from '../types/polymarket.js';
import type { Source } from '../types/graph.js';

export interface SocialSignals {
  tweetCount: number;
  uniqueAuthors: number;
  sentimentDistribution: { positive: number; neutral: number; negative: number };
  engagementRate: number;
  topInfluencers: Array<{ handle: string; followers: number; tweetCount: number }>;
  coordinationIndicators: {
    duplicateTextRatio: number;
    burstiness: number;
    lowFollowerRatio: number;
  };
  sources: Source[];
}

export class SocialEnricher {
  async enrich(market: MarketContext): Promise<SocialSignals> {
    const keywords = this.extractKeywords(market.question);
    const tweets = await this.searchTweets(keywords, market.slug);

    return this.analyzeTweets(tweets);
  }

  private extractKeywords(question: string): string[] {
    const stopWords = new Set([
      'will', 'the', 'be', 'is', 'are', 'was', 'were', 'a', 'an', 'and', 'or',
      'to', 'in', 'of', 'for', 'on', 'at', 'by', 'with', 'from', 'it', 'this',
      'that', 'which', 'who', 'what', 'when', 'where', 'how', 'do', 'does', 'did',
      'has', 'have', 'had', 'not', 'but', 'if', 'than', 'then', 'so', 'as',
      'before', 'after', 'during', 'between', 'through', 'above', 'below',
    ]);

    return question
      .replace(/[?.,!'"]/g, '')
      .split(/\s+/)
      .filter((w) => w.length > 2 && !stopWords.has(w.toLowerCase()))
      .slice(0, 6);
  }

  private async searchTweets(
    keywords: string[],
    slug: string,
  ): Promise<TweetData[]> {
    // Search X/Twitter via available APIs
    // In production, this would use the X API or a social listening service
    // For now, we construct a search query and return structured results
    const query = `${keywords.join(' ')} polymarket ${slug}`;

    try {
      const res = await fetch(
        `https://api.socialdata.tools/twitter/search?query=${encodeURIComponent(query)}&type=Latest`,
        {
          headers: {
            Authorization: `Bearer ${process.env.SOCIAL_DATA_API_KEY || ''}`,
            Accept: 'application/json',
          },
        },
      );

      if (!res.ok) return [];

      const data = await res.json();
      return (data.tweets || []).map((t: any) => ({
        id: t.id_str || t.id,
        text: t.full_text || t.text || '',
        author: t.user?.screen_name || 'unknown',
        authorFollowers: t.user?.followers_count || 0,
        createdAt: t.created_at || new Date().toISOString(),
        likes: t.favorite_count || 0,
        retweets: t.retweet_count || 0,
        replies: t.reply_count || 0,
        views: t.views?.count || 0,
      }));
    } catch {
      return [];
    }
  }

  private analyzeTweets(tweets: TweetData[]): SocialSignals {
    if (tweets.length === 0) {
      return {
        tweetCount: 0,
        uniqueAuthors: 0,
        sentimentDistribution: { positive: 0, neutral: 0, negative: 0 },
        engagementRate: 0,
        topInfluencers: [],
        coordinationIndicators: {
          duplicateTextRatio: 0,
          burstiness: 0,
          lowFollowerRatio: 0,
        },
        sources: [],
      };
    }

    const uniqueAuthors = new Set(tweets.map((t) => t.author));
    const totalEngagement = tweets.reduce(
      (sum, t) => sum + t.likes + t.retweets + t.replies,
      0,
    );

    // Sentiment analysis (basic keyword-based)
    const sentimentDist = { positive: 0, neutral: 0, negative: 0 };
    for (const tweet of tweets) {
      const s = this.basicSentiment(tweet.text);
      if (s > 0.2) sentimentDist.positive++;
      else if (s < -0.2) sentimentDist.negative++;
      else sentimentDist.neutral++;
    }

    // Coordination detection
    const textCounts = new Map<string, number>();
    for (const t of tweets) {
      const normalized = t.text.toLowerCase().trim().slice(0, 100);
      textCounts.set(normalized, (textCounts.get(normalized) || 0) + 1);
    }
    const duplicates = Array.from(textCounts.values()).filter((c) => c > 1);
    const duplicateTextRatio = duplicates.length / Math.max(textCounts.size, 1);

    const lowFollowerCount = tweets.filter((t) => t.authorFollowers < 100).length;
    const lowFollowerRatio = lowFollowerCount / Math.max(tweets.length, 1);

    // Burstiness: ratio of tweets in the busiest hour to average
    const hourBuckets = new Map<number, number>();
    for (const t of tweets) {
      const hour = Math.floor(new Date(t.createdAt).getTime() / 3600000);
      hourBuckets.set(hour, (hourBuckets.get(hour) || 0) + 1);
    }
    const maxHourly = Math.max(...hourBuckets.values(), 1);
    const avgHourly = tweets.length / Math.max(hourBuckets.size, 1);
    const burstiness = maxHourly / Math.max(avgHourly, 1);

    // Top influencers
    const authorMap = new Map<string, { followers: number; count: number }>();
    for (const t of tweets) {
      const existing = authorMap.get(t.author);
      if (existing) {
        existing.count++;
        existing.followers = Math.max(existing.followers, t.authorFollowers);
      } else {
        authorMap.set(t.author, { followers: t.authorFollowers, count: 1 });
      }
    }
    const topInfluencers = Array.from(authorMap.entries())
      .sort((a, b) => b[1].followers - a[1].followers)
      .slice(0, 10)
      .map(([handle, data]) => ({
        handle,
        followers: data.followers,
        tweetCount: data.count,
      }));

    // Convert tweets to sources
    const sources: Source[] = tweets.slice(0, 20).map((t) => ({
      id: `x:${t.id}`,
      url: `https://x.com/${t.author}/status/${t.id}`,
      platform: 'x' as const,
      author: t.author,
      publishedAt: t.createdAt,
      title: undefined,
      snippet: t.text.slice(0, 280),
      engagementMetrics: {
        likes: t.likes,
        shares: t.retweets,
        replies: t.replies,
        views: t.views,
      },
      credibilityScore: this.estimateCredibility(t),
      biasIndicators: [],
    }));

    return {
      tweetCount: tweets.length,
      uniqueAuthors: uniqueAuthors.size,
      sentimentDistribution: sentimentDist,
      engagementRate: totalEngagement / Math.max(tweets.length, 1),
      topInfluencers,
      coordinationIndicators: {
        duplicateTextRatio,
        burstiness: Math.min(burstiness, 10),
        lowFollowerRatio,
      },
      sources,
    };
  }

  private basicSentiment(text: string): number {
    const positive = ['correct', 'true', 'confirmed', 'verified', 'real', 'accurate', 'legit', 'official'];
    const negative = ['fake', 'false', 'misleading', 'scam', 'hoax', 'wrong', 'lie', 'debunked', 'manipulation'];
    const lower = text.toLowerCase();

    let score = 0;
    for (const w of positive) if (lower.includes(w)) score += 0.3;
    for (const w of negative) if (lower.includes(w)) score -= 0.3;

    return Math.max(-1, Math.min(1, score));
  }

  private estimateCredibility(tweet: TweetData): number {
    let score = 50;

    // Account age / followers as proxy
    if (tweet.authorFollowers > 10000) score += 20;
    else if (tweet.authorFollowers > 1000) score += 10;
    else if (tweet.authorFollowers < 50) score -= 20;

    // Engagement quality
    const engagementRatio =
      (tweet.likes + tweet.retweets) / Math.max(tweet.authorFollowers, 1);
    if (engagementRatio > 0.1) score += 10;
    if (engagementRatio > 1) score -= 10; // Suspiciously high

    return Math.max(0, Math.min(100, score));
  }
}

interface TweetData {
  id: string;
  text: string;
  author: string;
  authorFollowers: number;
  createdAt: string;
  likes: number;
  retweets: number;
  replies: number;
  views: number;
}
