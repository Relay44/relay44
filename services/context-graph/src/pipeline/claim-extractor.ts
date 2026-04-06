import { createHash } from 'crypto';
import type { MarketContext } from '../types/polymarket.js';
import type { Claim } from '../types/graph.js';
import type { SocialSignals } from './social-enricher.js';
import type { NewsSignals } from './news-enricher.js';
import type { ServiceConfig } from '../types/index.js';

export class ClaimExtractor {
  private apiKey?: string;
  private model: string;
  private enabled: boolean;

  constructor(config: ServiceConfig['llm']) {
    this.apiKey = config.apiKey;
    this.model = config.model;
    this.enabled = config.enabled;
  }

  async extract(
    market: MarketContext,
    social: SocialSignals,
    news: NewsSignals,
  ): Promise<Claim[]> {
    if (this.enabled && this.apiKey) {
      return this.extractWithLLM(market, social, news);
    }

    return this.extractHeuristic(market, social, news);
  }

  private async extractWithLLM(
    market: MarketContext,
    social: SocialSignals,
    news: NewsSignals,
  ): Promise<Claim[]> {
    const prompt = this.buildPrompt(market, social, news);

    try {
      const res = await fetch('https://api.anthropic.com/v1/messages', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'x-api-key': this.apiKey!,
          'anthropic-version': '2023-06-01',
        },
        body: JSON.stringify({
          model: this.model,
          max_tokens: 2000,
          messages: [{ role: 'user', content: prompt }],
        }),
      });

      if (!res.ok) {
        console.error(`LLM API error: ${res.status}`);
        return this.extractHeuristic(market, social, news);
      }

      const data = await res.json();
      const text = data.content?.[0]?.text || '';

      return this.parseLLMResponse(text, market.conditionId);
    } catch (error) {
      console.error('LLM extraction failed:', error);
      return this.extractHeuristic(market, social, news);
    }
  }

  private buildPrompt(
    market: MarketContext,
    social: SocialSignals,
    news: NewsSignals,
  ): string {
    const socialSnippets = social.sources
      .slice(0, 10)
      .map((s) => `- @${s.author}: "${s.snippet}"`)
      .join('\n');

    const newsSnippets = news.sources
      .slice(0, 5)
      .map((s) => `- [${s.author}] ${s.title}: "${s.snippet}"`)
      .join('\n');

    return `You are analyzing a Polymarket prediction market for potential misinformation. Extract all distinct factual claims being made about this market topic.

## Market
Question: ${market.question}
Description: ${market.description}
Current Odds: ${JSON.stringify(market.currentOdds)}
Category: ${market.category}

## Social Media Posts (X/Twitter)
${socialSnippets || 'No social data available'}

## News Articles
${newsSnippets || 'No news articles found'}

## Fact Checks
${news.factCheckResults.map((f) => `- [${f.source}] ${f.verdict}: ${f.url}`).join('\n') || 'None found'}

Extract distinct factual claims. For each claim, provide:
1. The claim text (one sentence)
2. Confidence (0-100, how certain the claim is being asserted)
3. Sentiment (-1 to 1, negative = bearish/against, positive = bullish/for the market question)
4. Verification status: "supported" (evidence backs it), "disputed" (conflicting evidence), "debunked" (proven false), or "unverified" (no clear evidence)

Respond in JSON format:
[
  {
    "text": "claim text here",
    "confidence": 75,
    "sentiment": 0.5,
    "verificationStatus": "unverified"
  }
]

Only return the JSON array, no other text.`;
  }

  private parseLLMResponse(text: string, conditionId: string): Claim[] {
    try {
      // Extract JSON from the response
      const jsonMatch = text.match(/\[[\s\S]*\]/);
      if (!jsonMatch) return [];

      const parsed = JSON.parse(jsonMatch[0]);
      if (!Array.isArray(parsed)) return [];

      return parsed
        .filter((c: any) => c.text && typeof c.text === 'string')
        .map((c: any, i: number) => {
          const claimText = String(c.text).slice(0, 1000);
          const claimHash = this.hashClaim(claimText);

          return {
            id: `${conditionId}:claim:${i}`,
            text: claimText,
            claimHash,
            sourceId: 'llm-extraction',
            confidence: Math.max(0, Math.min(100, Number(c.confidence) || 50)),
            sentiment: Math.max(-1, Math.min(1, Number(c.sentiment) || 0)),
            verificationStatus: this.validateStatus(c.verificationStatus),
            evidenceUALs: [],
            extractedAt: new Date().toISOString(),
          } satisfies Claim;
        });
    } catch {
      return [];
    }
  }

  private extractHeuristic(
    market: MarketContext,
    social: SocialSignals,
    news: NewsSignals,
  ): Claim[] {
    const claims: Claim[] = [];
    const seen = new Set<string>();

    // Extract from market itself
    const marketClaim = `${market.question.replace(/\?$/, '')} is currently at ${Math.round((market.currentOdds['Yes'] || 0) * 100)}% probability`;
    claims.push(this.createClaim(marketClaim, market.conditionId, 'polymarket', 90, 0, 'supported', seen));

    // Extract from social snippets — look for strong assertions
    for (const source of social.sources.slice(0, 15)) {
      const snippet = source.snippet || '';
      if (snippet.length < 20) continue;

      // Find sentences that make claims
      const sentences = snippet.split(/[.!]/).filter((s) => s.trim().length > 15);
      for (const sentence of sentences.slice(0, 2)) {
        const trimmed = sentence.trim();
        if (this.isAssertion(trimmed)) {
          claims.push(
            this.createClaim(trimmed, market.conditionId, source.id, 50, this.basicSentiment(trimmed), 'unverified', seen),
          );
        }
      }
    }

    // Extract from news
    for (const source of news.sources.slice(0, 10)) {
      if (source.title) {
        claims.push(
          this.createClaim(source.title, market.conditionId, source.id, 70, 0, 'unverified', seen),
        );
      }
    }

    // Extract from fact checks
    for (const fc of news.factCheckResults) {
      const status = fc.verdict.toLowerCase().includes('false') ? 'debunked' as const
        : fc.verdict.toLowerCase().includes('true') ? 'supported' as const
        : 'disputed' as const;

      claims.push(
        this.createClaim(`Fact check: ${fc.verdict}`, market.conditionId, `factcheck:${fc.source}`, 85, 0, status, seen),
      );
    }

    return claims.slice(0, 30);
  }

  private createClaim(
    text: string,
    conditionId: string,
    sourceId: string,
    confidence: number,
    sentiment: number,
    status: Claim['verificationStatus'],
    seen: Set<string>,
  ): Claim {
    const claimHash = this.hashClaim(text);
    seen.add(claimHash);

    return {
      id: `${conditionId}:claim:${claimHash.slice(0, 8)}`,
      text: text.slice(0, 1000),
      claimHash,
      sourceId,
      confidence,
      sentiment,
      verificationStatus: status,
      evidenceUALs: [],
      extractedAt: new Date().toISOString(),
    };
  }

  private isAssertion(text: string): boolean {
    const assertionPatterns = [
      /\b(is|are|was|were|will be|has been|have been)\b/i,
      /\b(confirmed|reported|announced|revealed|discovered|found)\b/i,
      /\b(according to|sources say|officials say)\b/i,
    ];

    return assertionPatterns.some((p) => p.test(text));
  }

  private basicSentiment(text: string): number {
    const positive = ['correct', 'true', 'confirmed', 'verified', 'real', 'likely', 'win', 'gain'];
    const negative = ['fake', 'false', 'misleading', 'wrong', 'unlikely', 'lose', 'scam', 'hoax'];
    const lower = text.toLowerCase();

    let score = 0;
    for (const w of positive) if (lower.includes(w)) score += 0.25;
    for (const w of negative) if (lower.includes(w)) score -= 0.25;

    return Math.max(-1, Math.min(1, score));
  }

  hashClaim(text: string): string {
    const normalized = text.toLowerCase().replace(/\s+/g, ' ').trim();
    return createHash('sha256').update(normalized).digest('hex');
  }

  private validateStatus(status: unknown): Claim['verificationStatus'] {
    const valid = ['unverified', 'supported', 'disputed', 'debunked'] as const;
    if (typeof status === 'string' && valid.includes(status as any)) {
      return status as Claim['verificationStatus'];
    }
    return 'unverified';
  }
}
