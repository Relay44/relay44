import type { DKGClientInterface } from './client.js';
import type { PublishResult, ServiceConfig } from '../types/index.js';
import type { MarketContext } from '../types/polymarket.js';
import type { Claim, Source, NarrativeEdge, ContextGraphSnapshot } from '../types/graph.js';
import type { MisinformationScore } from '../types/scoring.js';
import {
  MarketContextSchema,
  ClaimSchema,
  SourceSchema,
  NarrativeEdgeSchema,
  ContextGraphSnapshotSchema,
  buildMarketContextAsset,
  buildClaimAsset,
  buildSourceAsset,
  buildNarrativeEdgeAsset,
  buildContextGraphSnapshotAsset,
} from './schemas.js';

export interface PublisherLogger {
  info: (msg: string, meta?: object) => void;
  warn: (msg: string, meta?: object) => void;
  error: (msg: string, meta?: object) => void;
  debug: (msg: string, meta?: object) => void;
}

const noopLogger: PublisherLogger = {
  info: () => {},
  warn: () => {},
  error: () => {},
  debug: () => {},
};

export class ContextGraphPublisher {
  private dkg: DKGClientInterface;
  private epochs: number;
  private paranetUAL?: string;
  private logger: PublisherLogger;

  constructor(
    dkg: DKGClientInterface,
    config: ServiceConfig['dkg'],
    logger?: PublisherLogger,
  ) {
    this.dkg = dkg;
    this.epochs = config.epochs;
    this.paranetUAL = config.paranetUAL;
    this.logger = logger || noopLogger;
  }

  async publishMarket(market: MarketContext): Promise<PublishResult> {
    const validation = MarketContextSchema.safeParse(market);
    if (!validation.success) {
      const errorMsg = validation.error.issues.map((i) => i.message).join(', ');
      this.logger.warn('Market validation failed', { error: errorMsg });
      return { success: false, error: `Validation failed: ${errorMsg}` };
    }

    const asset = buildMarketContextAsset(market);
    return this.publish(asset, 'market', market.conditionId);
  }

  async publishClaim(claim: Claim): Promise<PublishResult> {
    const validation = ClaimSchema.safeParse(claim);
    if (!validation.success) {
      const errorMsg = validation.error.issues.map((i) => i.message).join(', ');
      this.logger.warn('Claim validation failed', { error: errorMsg });
      return { success: false, error: `Validation failed: ${errorMsg}` };
    }

    const asset = buildClaimAsset(claim);
    return this.publish(asset, 'claim', claim.claimHash);
  }

  async publishSource(source: Source): Promise<PublishResult> {
    const validation = SourceSchema.safeParse(source);
    if (!validation.success) {
      const errorMsg = validation.error.issues.map((i) => i.message).join(', ');
      this.logger.warn('Source validation failed', { error: errorMsg });
      return { success: false, error: `Validation failed: ${errorMsg}` };
    }

    const asset = buildSourceAsset(source);
    return this.publish(asset, 'source', source.id);
  }

  async publishNarrativeEdge(edge: NarrativeEdge): Promise<PublishResult> {
    const validation = NarrativeEdgeSchema.safeParse(edge);
    if (!validation.success) {
      const errorMsg = validation.error.issues.map((i) => i.message).join(', ');
      this.logger.warn('Edge validation failed', { error: errorMsg });
      return { success: false, error: `Validation failed: ${errorMsg}` };
    }

    const asset = buildNarrativeEdgeAsset(edge);
    return this.publish(asset, 'edge', edge.id);
  }

  async publishSnapshot(
    snapshot: ContextGraphSnapshot,
    claimUALs: string[],
    sourceUALs: string[],
    edgeUALs: string[],
  ): Promise<PublishResult> {
    const validation = ContextGraphSnapshotSchema.safeParse(snapshot);
    if (!validation.success) {
      const errorMsg = validation.error.issues.map((i) => i.message).join(', ');
      this.logger.warn('Snapshot validation failed', { error: errorMsg });
      return { success: false, error: `Validation failed: ${errorMsg}` };
    }

    const asset = buildContextGraphSnapshotAsset(snapshot, claimUALs, sourceUALs, edgeUALs);
    return this.publish(asset, 'snapshot', snapshot.id);
  }

  async publishFullGraph(
    market: MarketContext,
    claims: Claim[],
    sources: Source[],
    edges: NarrativeEdge[],
    score: MisinformationScore,
  ): Promise<{
    marketUAL?: string;
    claimUALs: string[];
    sourceUALs: string[];
    edgeUALs: string[];
    snapshotUAL?: string;
    errors: string[];
  }> {
    const errors: string[] = [];
    const claimUALs: string[] = [];
    const sourceUALs: string[] = [];
    const edgeUALs: string[] = [];

    // Publish market
    const marketResult = await this.publishMarket(market);
    if (!marketResult.success) errors.push(`market: ${marketResult.error}`);

    // Publish sources in parallel
    const sourceResults = await Promise.all(
      sources.map((s) => this.publishSource(s)),
    );
    for (const r of sourceResults) {
      if (r.success && r.ual) sourceUALs.push(r.ual);
      else if (r.error) errors.push(`source: ${r.error}`);
    }

    // Publish claims in parallel
    const claimResults = await Promise.all(
      claims.map((c) => this.publishClaim(c)),
    );
    for (const r of claimResults) {
      if (r.success && r.ual) claimUALs.push(r.ual);
      else if (r.error) errors.push(`claim: ${r.error}`);
    }

    // Publish edges in parallel
    const edgeResults = await Promise.all(
      edges.map((e) => this.publishNarrativeEdge(e)),
    );
    for (const r of edgeResults) {
      if (r.success && r.ual) edgeUALs.push(r.ual);
      else if (r.error) errors.push(`edge: ${r.error}`);
    }

    // Publish snapshot linking everything
    const snapshotId = `${market.conditionId}:${Date.now()}`;
    const snapshot: ContextGraphSnapshot = {
      id: snapshotId,
      conditionId: market.conditionId,
      marketQuestion: market.question,
      analysisTimestamp: new Date().toISOString(),
      misinfoScore: score.overall,
      claims,
      sources,
      edges,
      narrativePattern: score.summary,
      anomalyFlags: score.components
        .filter((c) => c.value > 70)
        .map((c) => c.name),
    };

    const snapshotResult = await this.publishSnapshot(
      snapshot,
      claimUALs,
      sourceUALs,
      edgeUALs,
    );
    if (!snapshotResult.success) errors.push(`snapshot: ${snapshotResult.error}`);

    this.logger.info('Full graph published', {
      conditionId: market.conditionId,
      claims: claimUALs.length,
      sources: sourceUALs.length,
      edges: edgeUALs.length,
      snapshotUAL: snapshotResult.ual,
      errors: errors.length,
    });

    return {
      marketUAL: marketResult.ual,
      claimUALs,
      sourceUALs,
      edgeUALs,
      snapshotUAL: snapshotResult.ual,
      errors,
    };
  }

  private async publish(asset: object, type: string, id: string): Promise<PublishResult> {
    const start = Date.now();
    try {
      this.logger.debug(`Publishing ${type}`, { id });
      const ual = await this.dkg.publish(asset, {
        epochs: this.epochs,
        paranetUAL: this.paranetUAL,
      });
      this.logger.info(`${type} published`, { id, ual, duration: Date.now() - start });
      return { success: true, ual };
    } catch (error) {
      const msg = error instanceof Error ? error.message : 'Publishing failed';
      this.logger.error(`${type} publish failed`, { id, error: msg, duration: Date.now() - start });
      return { success: false, error: msg };
    }
  }
}
