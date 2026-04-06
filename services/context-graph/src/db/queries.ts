import type Database from 'better-sqlite3';
import type { ContextGraphResult } from '../types/graph.js';
import type { MisinformationScore } from '../types/scoring.js';

const CACHE_TTL_SECONDS = 300; // 5 minutes

export class AnalysisStore {
  private db: Database.Database;

  constructor(db: Database.Database) {
    this.db = db;
  }

  getCached(conditionId: string): CachedAnalysis | null {
    const row = this.db
      .prepare(
        `SELECT * FROM analyses
         WHERE condition_id = ? AND status = 'complete'
         AND expires_at > unixepoch()
         ORDER BY created_at DESC LIMIT 1`,
      )
      .get(conditionId) as CachedRow | undefined;

    if (!row) return null;

    return {
      id: row.id,
      conditionId: row.condition_id,
      marketQuestion: row.market_question,
      score: {
        overall: row.misinfo_score,
        confidence: row.confidence,
        riskLevel: row.risk_level as MisinformationScore['riskLevel'],
        summary: row.summary,
        components: [],
      },
      graph: row.graph_json ? JSON.parse(row.graph_json) : null,
      snapshotUAL: row.snapshot_ual,
      createdAt: row.created_at,
    };
  }

  save(
    conditionId: string,
    marketQuestion: string,
    result: ContextGraphResult,
  ): string {
    const id = `${conditionId}:${Date.now()}`;
    const score = result.score;

    const componentMap = new Map(
      score.components.map((c) => [c.name, c.value]),
    );

    this.db
      .prepare(
        `INSERT INTO analyses (
          id, condition_id, market_question, status,
          misinfo_score, source_credibility_score, claim_consistency_score,
          narrative_velocity_score, coordination_score,
          odds_divergence_score, temporal_anomaly_score,
          confidence, risk_level, summary,
          snapshot_ual, graph_json,
          claim_count, source_count, edge_count,
          expires_at
        ) VALUES (
          ?, ?, ?, 'complete',
          ?, ?, ?,
          ?, ?,
          ?, ?,
          ?, ?, ?,
          ?, ?,
          ?, ?, ?,
          unixepoch() + ?
        )`,
      )
      .run(
        id,
        conditionId,
        marketQuestion,
        score.overall,
        componentMap.get('sourceCredibility') ?? null,
        componentMap.get('claimConsistency') ?? null,
        componentMap.get('narrativeVelocity') ?? null,
        componentMap.get('coordinationSignal') ?? null,
        componentMap.get('oddsDivergence') ?? null,
        componentMap.get('temporalAnomaly') ?? null,
        score.confidence,
        score.riskLevel,
        score.summary,
        result.metadata.snapshotUAL ?? null,
        JSON.stringify(result),
        result.metadata.claimCount,
        result.metadata.sourceCount,
        result.metadata.edgeCount,
        CACHE_TTL_SECONDS,
      );

    return id;
  }

  saveError(conditionId: string, marketQuestion: string, error: string): void {
    const id = `${conditionId}:err:${Date.now()}`;
    this.db
      .prepare(
        `INSERT INTO analyses (id, condition_id, market_question, status, error, expires_at)
         VALUES (?, ?, ?, 'failed', ?, unixepoch() + 60)`,
      )
      .run(id, conditionId, marketQuestion, error);
  }

  getHistory(conditionId: string, limit = 20): HistoryRow[] {
    return this.db
      .prepare(
        `SELECT id, misinfo_score, confidence, risk_level, summary,
                claim_count, source_count, edge_count, snapshot_ual, created_at
         FROM analyses
         WHERE condition_id = ? AND status = 'complete'
         ORDER BY created_at DESC LIMIT ?`,
      )
      .all(conditionId, limit) as HistoryRow[];
  }

  saveNarrativeSnapshot(
    conditionId: string,
    score: number,
    claimCount: number,
    sourceCount: number,
    narrative: string,
  ): void {
    const id = `${conditionId}:snap:${Date.now()}`;
    this.db
      .prepare(
        `INSERT INTO narrative_snapshots
         (id, condition_id, snapshot_at, misinfo_score, claim_count, source_count, dominant_narrative)
         VALUES (?, ?, unixepoch(), ?, ?, ?, ?)`,
      )
      .run(id, conditionId, score, claimCount, sourceCount, narrative);
  }

  getNarrativeTimeline(conditionId: string, limit = 50): NarrativeRow[] {
    return this.db
      .prepare(
        `SELECT * FROM narrative_snapshots
         WHERE condition_id = ? ORDER BY snapshot_at DESC LIMIT ?`,
      )
      .all(conditionId, limit) as NarrativeRow[];
  }
}

interface CachedRow {
  id: string;
  condition_id: string;
  market_question: string;
  misinfo_score: number;
  confidence: number;
  risk_level: string;
  summary: string;
  graph_json: string | null;
  snapshot_ual: string | null;
  created_at: number;
}

export interface CachedAnalysis {
  id: string;
  conditionId: string;
  marketQuestion: string;
  score: Partial<MisinformationScore>;
  graph: ContextGraphResult | null;
  snapshotUAL: string | null;
  createdAt: number;
}

export interface HistoryRow {
  id: string;
  misinfo_score: number;
  confidence: number;
  risk_level: string;
  summary: string;
  claim_count: number;
  source_count: number;
  edge_count: number;
  snapshot_ual: string | null;
  created_at: number;
}

export interface NarrativeRow {
  id: string;
  condition_id: string;
  snapshot_at: number;
  misinfo_score: number;
  claim_count: number;
  source_count: number;
  dominant_narrative: string;
}
