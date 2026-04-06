import type { ServiceConfig } from '../types/index.js';
import type { MarketInput } from '../types/polymarket.js';
import type { ContextGraphResult, GraphNode, GraphEdge } from '../types/graph.js';
import type { DKGClientInterface } from '../dkg/client.js';
import { ContextGraphPublisher } from '../dkg/publisher.js';
import { PolymarketFetcher } from './polymarket-fetcher.js';
import { SocialEnricher } from './social-enricher.js';
import { NewsEnricher } from './news-enricher.js';
import { ClaimExtractor } from './claim-extractor.js';
import { NarrativeTracker } from './narrative-tracker.js';
import { ScoringEngine } from '../scoring/engine.js';

export class ContextGraphPipeline {
  private fetcher: PolymarketFetcher;
  private socialEnricher: SocialEnricher;
  private newsEnricher: NewsEnricher;
  private claimExtractor: ClaimExtractor;
  private narrativeTracker: NarrativeTracker;
  private scoringEngine: ScoringEngine;
  private publisher: ContextGraphPublisher;
  private dkgEnabled: boolean;

  constructor(dkg: DKGClientInterface, config: ServiceConfig) {
    this.fetcher = new PolymarketFetcher(config.polymarket);
    this.socialEnricher = new SocialEnricher();
    this.newsEnricher = new NewsEnricher();
    this.claimExtractor = new ClaimExtractor(config.llm);
    this.narrativeTracker = new NarrativeTracker();
    this.scoringEngine = new ScoringEngine();
    this.publisher = new ContextGraphPublisher(dkg, config.dkg);
    this.dkgEnabled = config.features.dkgEnabled;
  }

  async analyze(input: MarketInput): Promise<ContextGraphResult> {
    const start = Date.now();

    // Phase 1: Fetch market data
    const marketData = await this.fetcher.fetch(input);
    console.log(`[pipeline] Market fetched: ${marketData.question} (${Date.now() - start}ms)`);

    // Phase 2: Parallel enrichment
    const [socialSignals, newsSignals] = await Promise.all([
      this.socialEnricher.enrich(marketData),
      this.newsEnricher.crossReference(marketData),
    ]);
    console.log(`[pipeline] Enrichment complete: ${socialSignals.tweetCount} tweets, ${newsSignals.articleCount} articles (${Date.now() - start}ms)`);

    // Phase 3: Claim extraction
    const claims = await this.claimExtractor.extract(marketData, socialSignals, newsSignals);
    console.log(`[pipeline] Extracted ${claims.length} claims (${Date.now() - start}ms)`);

    // Phase 4: Narrative tracking
    const allSources = [...socialSignals.sources, ...newsSignals.sources];
    const edges = this.narrativeTracker.buildEdges(claims, socialSignals, newsSignals);
    console.log(`[pipeline] Built ${edges.length} narrative edges (${Date.now() - start}ms)`);

    // Phase 5: Scoring
    const score = this.scoringEngine.score(claims, edges, socialSignals, newsSignals, marketData);
    console.log(`[pipeline] Misinfo score: ${score.overall} (${score.riskLevel}) (${Date.now() - start}ms)`);

    // Phase 6: Publish to DKG (if enabled)
    let snapshotUAL: string | undefined;
    if (this.dkgEnabled) {
      try {
        const publishResult = await this.publisher.publishFullGraph(
          marketData,
          claims,
          allSources,
          edges,
          score,
        );
        snapshotUAL = publishResult.snapshotUAL;
        if (publishResult.errors.length > 0) {
          console.warn(`[pipeline] DKG publish warnings:`, publishResult.errors);
        }
        console.log(`[pipeline] Published to DKG: ${snapshotUAL} (${Date.now() - start}ms)`);
      } catch (error) {
        console.error('[pipeline] DKG publish failed:', error);
      }
    }

    // Phase 7: Build graph response
    const nodes: GraphNode[] = [
      {
        id: `market:${marketData.conditionId}`,
        type: 'market',
        label: marketData.question,
        data: {
          conditionId: marketData.conditionId,
          odds: marketData.currentOdds,
          volume24h: marketData.volume24h,
          liquidity: marketData.totalLiquidity,
          category: marketData.category,
          slug: marketData.slug,
        },
      },
      ...claims.map((c) => ({
        id: c.id,
        type: 'claim' as const,
        label: c.text.slice(0, 80),
        data: {
          text: c.text,
          claimHash: c.claimHash,
          confidence: c.confidence,
          sentiment: c.sentiment,
          verificationStatus: c.verificationStatus,
        },
      })),
      ...allSources.slice(0, 30).map((s) => ({
        id: s.id,
        type: 'source' as const,
        label: s.title || s.author,
        data: {
          url: s.url,
          platform: s.platform,
          author: s.author,
          credibilityScore: s.credibilityScore,
          engagement: s.engagementMetrics,
        },
      })),
    ];

    const graphEdges: GraphEdge[] = edges.map((e) => ({
      id: e.id,
      source: e.sourceNodeId,
      target: e.targetNodeId,
      type: e.relationshipType,
      weight: e.weight,
    }));

    // Add market → claim edges
    for (const claim of claims) {
      graphEdges.push({
        id: `market→${claim.id}`,
        source: `market:${marketData.conditionId}`,
        target: claim.id,
        type: 'originates',
        weight: 1,
      });
    }

    console.log(`[pipeline] Analysis complete in ${Date.now() - start}ms`);

    return {
      nodes,
      edges: graphEdges,
      score,
      metadata: {
        analyzedAt: new Date().toISOString(),
        snapshotUAL,
        claimCount: claims.length,
        sourceCount: allSources.length,
        edgeCount: edges.length,
      },
    };
  }
}
