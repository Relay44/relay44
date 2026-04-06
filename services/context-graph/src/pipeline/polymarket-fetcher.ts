import type { MarketContext, MarketInput, GammaMarketResponse, OddsSnapshot } from '../types/polymarket.js';
import type { ServiceConfig } from '../types/index.js';

export class PolymarketFetcher {
  private gammaApi: string;
  private clobApi: string;

  constructor(config: ServiceConfig['polymarket']) {
    this.gammaApi = config.gammaApi;
    this.clobApi = config.clobApi;
  }

  async fetch(input: MarketInput): Promise<MarketContext> {
    const conditionId = input.conditionId || this.extractConditionId(input.marketUrl);
    const slug = input.slug || this.extractSlug(input.marketUrl);

    if (!conditionId && !slug) {
      throw new Error('Must provide conditionId, slug, or marketUrl');
    }

    const market = slug
      ? await this.fetchBySlug(slug)
      : await this.fetchByConditionId(conditionId!);

    const movementHistory = await this.fetchPriceHistory(market.conditionId);

    return {
      ...market,
      movementHistory,
    };
  }

  private extractConditionId(url?: string): string | undefined {
    if (!url) return undefined;
    // Polymarket URLs: polymarket.com/event/slug/condition-slug?tid=conditionId
    const match = url.match(/[?&]tid=([a-fA-F0-9]+)/);
    return match?.[1];
  }

  private extractSlug(url?: string): string | undefined {
    if (!url) return undefined;
    // polymarket.com/event/some-event-slug
    const match = url.match(/polymarket\.com\/event\/([^/?#]+)/);
    return match?.[1];
  }

  private async fetchBySlug(slug: string): Promise<Omit<MarketContext, 'movementHistory'>> {
    const res = await fetch(`${this.gammaApi}/markets?slug=${encodeURIComponent(slug)}&limit=1`);
    if (!res.ok) throw new Error(`Gamma API error: ${res.status}`);

    const markets: GammaMarketResponse[] = await res.json();
    if (!markets.length) throw new Error(`No market found for slug: ${slug}`);

    return this.parseGammaMarket(markets[0]);
  }

  private async fetchByConditionId(conditionId: string): Promise<Omit<MarketContext, 'movementHistory'>> {
    const res = await fetch(`${this.gammaApi}/markets?condition_id=${encodeURIComponent(conditionId)}&limit=1`);
    if (!res.ok) throw new Error(`Gamma API error: ${res.status}`);

    const markets: GammaMarketResponse[] = await res.json();
    if (!markets.length) throw new Error(`No market found for conditionId: ${conditionId}`);

    return this.parseGammaMarket(markets[0]);
  }

  private parseGammaMarket(raw: GammaMarketResponse): Omit<MarketContext, 'movementHistory'> {
    const outcomes = this.safeParseJson<string[]>(raw.outcomes, []);
    const outcomePrices = this.safeParseJson<string[]>(raw.outcomePrices, []);

    const currentOdds: Record<string, number> = {};
    for (let i = 0; i < outcomes.length; i++) {
      currentOdds[outcomes[i]] = parseFloat(outcomePrices[i] || '0');
    }

    return {
      conditionId: raw.conditionId,
      question: raw.question,
      description: raw.description || '',
      outcomes,
      currentOdds,
      volume24h: parseFloat(raw.volume || '0'),
      totalLiquidity: parseFloat(raw.liquidity || '0'),
      category: raw.category || 'Other',
      createdAt: raw.startDate || new Date().toISOString(),
      endDate: raw.endDate || '',
      slug: raw.slug,
      active: raw.active,
    };
  }

  private async fetchPriceHistory(conditionId: string): Promise<OddsSnapshot[]> {
    try {
      // Gamma API timeseries endpoint
      const res = await fetch(
        `${this.gammaApi}/markets/${conditionId}/timeseries?interval=1h&fidelity=60`,
      );
      if (!res.ok) return [];

      const data = await res.json();
      if (!Array.isArray(data)) return [];

      return data.slice(-48).map((point: any) => ({
        timestamp: new Date(point.t * 1000).toISOString(),
        outcomes: { 'Yes': Number(point.p) || 0, 'No': 1 - (Number(point.p) || 0) } as Record<string, number>,
        volume24h: Number(point.v) || 0,
      }));
    } catch {
      return [];
    }
  }

  private safeParseJson<T>(value: string | undefined, fallback: T): T {
    if (!value) return fallback;
    try {
      return JSON.parse(value);
    } catch {
      return fallback;
    }
  }
}
