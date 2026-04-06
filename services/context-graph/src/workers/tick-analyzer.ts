import type { ServiceConfig } from '../types/index.js';
import type { DKGClientInterface } from '../dkg/client.js';
import { ContextGraphPipeline } from '../pipeline/orchestrator.js';
import { AnalysisStore } from '../db/queries.js';
import type Database from 'better-sqlite3';

const TICK_INTERVAL_MS = 15 * 60 * 1000; // 15 minutes
const ODDS_CHANGE_THRESHOLD = 0.05; // 5% movement triggers re-analysis

export class TickAnalyzer {
  private pipeline: ContextGraphPipeline;
  private store: AnalysisStore;
  private trackedMarkets: Map<string, { lastOdds: Record<string, number>; lastAnalyzed: number }>;
  private interval: NodeJS.Timeout | null = null;

  constructor(dkg: DKGClientInterface, db: Database.Database, config: ServiceConfig) {
    this.pipeline = new ContextGraphPipeline(dkg, config);
    this.store = new AnalysisStore(db);
    this.trackedMarkets = new Map();
  }

  start(): void {
    console.log(`[tick-analyzer] Starting with ${TICK_INTERVAL_MS / 60000}min interval`);
    this.interval = setInterval(() => this.tick(), TICK_INTERVAL_MS);
  }

  stop(): void {
    if (this.interval) {
      clearInterval(this.interval);
      this.interval = null;
    }
    console.log('[tick-analyzer] Stopped');
  }

  trackMarket(conditionId: string, currentOdds: Record<string, number>): void {
    this.trackedMarkets.set(conditionId, {
      lastOdds: currentOdds,
      lastAnalyzed: Date.now(),
    });
    console.log(`[tick-analyzer] Tracking market: ${conditionId}`);
  }

  untrackMarket(conditionId: string): void {
    this.trackedMarkets.delete(conditionId);
  }

  private async tick(): Promise<void> {
    console.log(`[tick-analyzer] Tick: ${this.trackedMarkets.size} markets tracked`);

    for (const [conditionId, state] of this.trackedMarkets) {
      try {
        // Quick fetch to check odds movement
        const result = await this.pipeline.analyze({
          conditionId,
          depth: 'quick',
        });

        const marketNode = result.nodes.find((n) => n.type === 'market');
        const currentOdds = (marketNode?.data?.odds as Record<string, number>) || {};

        // Check if odds moved significantly
        let maxChange = 0;
        for (const [outcome, odds] of Object.entries(currentOdds)) {
          const prevOdds = state.lastOdds[outcome] || 0;
          maxChange = Math.max(maxChange, Math.abs(odds - prevOdds));
        }

        if (maxChange > ODDS_CHANGE_THRESHOLD) {
          console.log(`[tick-analyzer] ${conditionId}: odds moved ${(maxChange * 100).toFixed(1)}%, re-analyzing`);

          const fullResult = await this.pipeline.analyze({
            conditionId,
            depth: 'full',
          });

          const question = marketNode?.label || conditionId;
          this.store.save(conditionId, question, fullResult);
          this.store.saveNarrativeSnapshot(
            conditionId,
            fullResult.score.overall,
            fullResult.metadata.claimCount,
            fullResult.metadata.sourceCount,
            fullResult.score.summary,
          );

          state.lastOdds = currentOdds;
          state.lastAnalyzed = Date.now();
        }
      } catch (error) {
        console.error(`[tick-analyzer] Error analyzing ${conditionId}:`, error);
      }
    }
  }
}
