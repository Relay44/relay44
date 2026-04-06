import Database from 'better-sqlite3';
import { mkdirSync } from 'fs';
import { dirname } from 'path';

export function initDatabase(dbPath: string): Database.Database {
  mkdirSync(dirname(dbPath), { recursive: true });

  const db = new Database(dbPath);
  db.pragma('journal_mode = WAL');
  db.pragma('busy_timeout = 5000');

  db.exec(`
    CREATE TABLE IF NOT EXISTS analyses (
      id TEXT PRIMARY KEY,
      condition_id TEXT NOT NULL,
      market_question TEXT NOT NULL,
      status TEXT NOT NULL DEFAULT 'pending',
      misinfo_score REAL,
      source_credibility_score REAL,
      claim_consistency_score REAL,
      narrative_velocity_score REAL,
      coordination_score REAL,
      odds_divergence_score REAL,
      temporal_anomaly_score REAL,
      confidence REAL,
      risk_level TEXT,
      summary TEXT,
      snapshot_ual TEXT,
      graph_json TEXT,
      claim_count INTEGER DEFAULT 0,
      source_count INTEGER DEFAULT 0,
      edge_count INTEGER DEFAULT 0,
      error TEXT,
      created_at INTEGER DEFAULT (unixepoch()),
      updated_at INTEGER DEFAULT (unixepoch()),
      expires_at INTEGER
    );

    CREATE INDEX IF NOT EXISTS idx_analyses_condition ON analyses(condition_id);
    CREATE INDEX IF NOT EXISTS idx_analyses_status ON analyses(status);
    CREATE INDEX IF NOT EXISTS idx_analyses_created ON analyses(created_at);

    CREATE TABLE IF NOT EXISTS narrative_snapshots (
      id TEXT PRIMARY KEY,
      condition_id TEXT NOT NULL,
      snapshot_at INTEGER NOT NULL,
      misinfo_score REAL,
      claim_count INTEGER,
      source_count INTEGER,
      dominant_narrative TEXT,
      created_at INTEGER DEFAULT (unixepoch())
    );

    CREATE INDEX IF NOT EXISTS idx_narrative_condition ON narrative_snapshots(condition_id);
  `);

  return db;
}
