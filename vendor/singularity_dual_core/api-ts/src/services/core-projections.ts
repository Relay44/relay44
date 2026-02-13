import { Pool } from 'pg';
import type { MarketRecord } from './market-ledger.js';
import { toNamespacedMarketId, type CoreChain } from './core-ids.js';

export interface CoreProjectedMarket {
  id: string;
  chain: CoreChain;
  marketRef: string;
  legacyMarketId: string | null;
  address: string;
  question: string;
  description: string;
  category: string;
  status: MarketRecord['status'];
  yesPrice: number;
  noPrice: number;
  yesSupply: number;
  noSupply: number;
  volume24h: number;
  totalVolume: number;
  totalCollateral: number;
  feeBps: number;
  oracle: string;
  resolutionMode: MarketRecord['resolutionMode'];
  collateralMint: string;
  yesMint: string;
  noMint: string;
  resolutionDeadline: string;
  tradingEnd: string;
  createdAt: string;
  resolvedOutcome?: MarketRecord['resolvedOutcome'];
  resolvedAt?: string;
  resolutionTx?: string;
  evidenceHash?: string;
  oracleSource?: string;
  resolverIdentity?: string;
  source: 'core';
  provider: 'core_solana' | 'core_base';
}

interface BaseMarketProjectionRow {
  chain: CoreChain;
  market_ref: string;
  legacy_market_id: string | null;
  payload: Record<string, unknown>;
  updated_at: string;
}

const databaseUrl = process.env.DATABASE_URL?.trim() || '';
const usePostgres = databaseUrl.length > 0;

let pool: Pool | null = null;
let initPromise: Promise<void> | null = null;

const baseProjectionMemory = new Map<string, CoreProjectedMarket>();
const legacyMarketMapMemory = new Map<string, string>();
const checkpointMemory = new Map<string, string>();

function getPool(): Pool {
  if (!pool) {
    pool = new Pool({ connectionString: databaseUrl, max: 5 });
  }
  return pool;
}

async function ensureInit(): Promise<void> {
  if (!usePostgres) return;
  if (initPromise) return initPromise;

  initPromise = (async () => {
    const client = await getPool().connect();
    try {
      await client.query(`
        CREATE TABLE IF NOT EXISTS keiro_core_market_projection (
          chain text NOT NULL,
          market_ref text NOT NULL,
          legacy_market_id text,
          payload jsonb NOT NULL,
          updated_at timestamptz NOT NULL DEFAULT now(),
          PRIMARY KEY (chain, market_ref)
        );

        CREATE TABLE IF NOT EXISTS keiro_core_order_projection (
          chain text NOT NULL,
          order_ref text NOT NULL,
          market_ref text NOT NULL,
          payload jsonb NOT NULL,
          updated_at timestamptz NOT NULL DEFAULT now(),
          PRIMARY KEY (chain, order_ref)
        );

        CREATE TABLE IF NOT EXISTS keiro_core_position_projection (
          chain text NOT NULL,
          position_ref text NOT NULL,
          owner_wallet text NOT NULL,
          payload jsonb NOT NULL,
          updated_at timestamptz NOT NULL DEFAULT now(),
          PRIMARY KEY (chain, position_ref)
        );

        CREATE TABLE IF NOT EXISTS keiro_core_trade_projection (
          chain text NOT NULL,
          trade_ref text NOT NULL,
          market_ref text NOT NULL,
          payload jsonb NOT NULL,
          updated_at timestamptz NOT NULL DEFAULT now(),
          PRIMARY KEY (chain, trade_ref)
        );

        CREATE TABLE IF NOT EXISTS keiro_core_dispute_projection (
          chain text NOT NULL,
          dispute_ref text NOT NULL,
          market_ref text NOT NULL,
          payload jsonb NOT NULL,
          updated_at timestamptz NOT NULL DEFAULT now(),
          PRIMARY KEY (chain, dispute_ref)
        );

        CREATE TABLE IF NOT EXISTS keiro_legacy_market_map (
          legacy_market_id text PRIMARY KEY,
          sol_market_id text NOT NULL,
          created_at timestamptz NOT NULL DEFAULT now(),
          updated_at timestamptz NOT NULL DEFAULT now()
        );

        CREATE TABLE IF NOT EXISTS keiro_legacy_order_map (
          legacy_order_id text PRIMARY KEY,
          sol_order_ref text NOT NULL,
          created_at timestamptz NOT NULL DEFAULT now(),
          updated_at timestamptz NOT NULL DEFAULT now()
        );

        CREATE TABLE IF NOT EXISTS keiro_migration_runs (
          id bigserial PRIMARY KEY,
          run_id text UNIQUE NOT NULL,
          status text NOT NULL,
          snapshot_hash text,
          started_at timestamptz NOT NULL DEFAULT now(),
          completed_at timestamptz
        );

        CREATE TABLE IF NOT EXISTS keiro_migration_deltas (
          id bigserial PRIMARY KEY,
          run_id text NOT NULL,
          entity_type text NOT NULL,
          entity_ref text NOT NULL,
          delta jsonb NOT NULL,
          created_at timestamptz NOT NULL DEFAULT now()
        );

        CREATE TABLE IF NOT EXISTS keiro_migration_failures (
          id bigserial PRIMARY KEY,
          run_id text NOT NULL,
          entity_type text NOT NULL,
          entity_ref text NOT NULL,
          error_code text,
          error_message text NOT NULL,
          created_at timestamptz NOT NULL DEFAULT now()
        );

        CREATE TABLE IF NOT EXISTS keiro_chain_checkpoints (
          engine text NOT NULL,
          chain text NOT NULL,
          cursor text NOT NULL,
          updated_at timestamptz NOT NULL DEFAULT now(),
          PRIMARY KEY (engine, chain)
        );
      `);
    } finally {
      client.release();
    }
  })();

  return initPromise;
}

function mapProjectedMarket(payload: Record<string, unknown>, row: BaseMarketProjectionRow): CoreProjectedMarket | null {
  const id = row.market_ref;
  const marketRef = row.market_ref;
  const question = typeof payload.question === 'string' ? payload.question : '';
  if (!question) return null;

  const readString = (key: string, fallback = ''): string => {
    const value = payload[key];
    return typeof value === 'string' ? value : fallback;
  };
  const readNumber = (key: string, fallback = 0): number => {
    const value = payload[key];
    if (typeof value === 'number') return value;
    const parsed = Number.parseFloat(String(value));
    return Number.isFinite(parsed) ? parsed : fallback;
  };

  const status = readString('status', 'active') as MarketRecord['status'];

  return {
    id: toNamespacedMarketId('base', id),
    chain: 'base',
    marketRef,
    legacyMarketId: row.legacy_market_id,
    address: readString('address', marketRef),
    question,
    description: readString('description'),
    category: readString('category', 'uncategorized'),
    status,
    yesPrice: readNumber('yesPrice', 0.5),
    noPrice: readNumber('noPrice', 0.5),
    yesSupply: readNumber('yesSupply', 0),
    noSupply: readNumber('noSupply', 0),
    volume24h: readNumber('volume24h', 0),
    totalVolume: readNumber('totalVolume', 0),
    totalCollateral: readNumber('totalCollateral', 0),
    feeBps: readNumber('feeBps', 50),
    oracle: readString('oracle', 'committee'),
    resolutionMode: readString('resolutionMode', 'committee_manual') as MarketRecord['resolutionMode'],
    collateralMint: readString('collateralMint', 'USDC'),
    yesMint: readString('yesMint', `${marketRef}:yes`),
    noMint: readString('noMint', `${marketRef}:no`),
    resolutionDeadline: readString('resolutionDeadline', new Date().toISOString()),
    tradingEnd: readString('tradingEnd', new Date().toISOString()),
    createdAt: readString('createdAt', row.updated_at),
    resolvedOutcome:
      payload.resolvedOutcome === 'yes' || payload.resolvedOutcome === 'no'
        ? (payload.resolvedOutcome as MarketRecord['resolvedOutcome'])
        : undefined,
    resolvedAt: readString('resolvedAt') || undefined,
    resolutionTx: readString('resolutionTx') || undefined,
    evidenceHash: readString('evidenceHash') || undefined,
    oracleSource: readString('oracleSource') || undefined,
    resolverIdentity: readString('resolverIdentity') || undefined,
    source: 'core',
    provider: 'core_base',
  };
}
