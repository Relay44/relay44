import crypto from 'node:crypto';
import { Hono, type Context } from 'hono';
import { zValidator } from '@hono/zod-validator';
import { z } from 'zod';
import { getAuthContext, verifyWalletSignature, verifyWalletSignatureStrict } from '../middleware/auth.js';
import { requireStakeEligible } from '../middleware/beta-eligibility.js';
import { requireFeatureEnabled } from '../middleware/feature-gates.js';
import { requireAdminWallet } from '../middleware/admin.js';
import { getIdempotencyKey, requireIdempotencyKey } from '../middleware/idempotency.js';
import { agentService } from '../services/agents.js';
import { receiptService } from '../services/receipts.js';
import { executionService } from '../services/execution.js';
import {
  marketLedgerService,
  type MarketRecord,
  type MarketSort,
  type Outcome,
  type SortOrder,
} from '../services/market-ledger.js';
import { walletActionRateLimit } from '../services/action-rate-limit.js';
import { switchboardService } from '../services/switchboard.js';
import { disputeLedgerService } from '../services/dispute-ledger.js';
import {
  isLiquidityAllocatorError,
  liquidityAllocatorService,
} from '../services/liquidity-allocator.js';
import { pnpLiquidityService } from '../services/pnp-liquidity.js';
import { pnpMarketService } from '../services/pnp-market.js';
import { pnpExecutionService } from '../services/pnp-execution.js';
import { limitlessMarketService } from '../services/limitless-market.js';
import { limitlessExecutionService } from '../services/limitless-execution.js';
import {
  isJupiterPredictionUpstreamError,
  jupiterPredictionService,
} from '../services/jupiter-prediction.js';
import {
  isLegacyLedgerAlias,
  parseChainQuery,
  parseMarketRef,
  parseUnifiedSource,
  toNamespacedMarketId,
  type CoreChain,
  type CoreChainQuery,
  type UnifiedMarketSource,
} from '../services/core-ids.js';
import { coreProjectionService, type CoreProjectedMarket } from '../services/core-projections.js';
import {
  isSyntheticLedgerWriteEnabled,
  syntheticLedgerWriteBlockReason,
} from '../services/core-write-gate.js';
import {
  isProviderAllowed,
  noteProviderRestriction,
  requireProviderAllowed,
} from '../middleware/region-routing.js';

function clampPrice(value: number): number {
  const clamped = Math.max(0.01, Math.min(0.99, value));
  return Number(clamped.toFixed(4));
}

function parsePositiveInt(
  value: string | undefined,
  fallback: number,
  options: { min: number; max: number }
): number {
  if (!value) return fallback;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.max(options.min, Math.min(options.max, parsed));
}

function parseOffset(value: string | undefined): number {
  if (!value) return 0;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed < 0) return 0;
  return parsed;
}

function parseSort(value: string | undefined): MarketSort {
  if (value === 'newest' || value === 'ending' || value === 'volume') return value;
  return 'volume';
}

function parseSortOrder(value: string | undefined): SortOrder {
  return value === 'asc' ? 'asc' : 'desc';
}

function parseBoolean(value: string | undefined): boolean {
  if (!value) return false;
  const normalized = value.trim().toLowerCase();
  return normalized === '1' || normalized === 'true' || normalized === 'yes' || normalized === 'on';
}

function parseMarketSource(value: string | undefined): UnifiedMarketSource {
  const parsed = parseUnifiedSource(value);
  if (parsed !== 'all') return parsed;
  if (
    !jupiterPredictionService.isEnabled() &&
    !pnpMarketService.isEnabled() &&
    !limitlessMarketService.isEnabled()
  ) {
    return 'ledger';
  }
  if (!jupiterPredictionService.isEnabled() && pnpMarketService.isEnabled() && !limitlessMarketService.isEnabled()) {
    return 'pnp';
  }
  if (
    !jupiterPredictionService.isEnabled() &&
    !pnpMarketService.isEnabled() &&
    limitlessMarketService.isEnabled()
  ) {
    return 'limitless';
  }
  return parsed;
}

type CreateMarketProvider = 'core' | 'ledger' | 'pnp';
type MarketModel = 'v2_amm' | 'v3_p2p';
type OracleMode = 'pnp_default' | 'custom';
type ExecutionMode = 'user_signed' | 'server_custody';
const DEFAULT_COLLATERAL_MINT = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v';

function parseMarketModel(value: string | undefined): MarketModel | undefined {
  if (value === 'v2_amm' || value === 'v3_p2p') return value;
  return undefined;
}

function parseOutcome(value: string | undefined): Outcome | null {
  if (!value) return null;
  if (value === 'yes' || value === 'no') return value;
  return null;
}

function getAuthWallet(c: Context): string | null {
  const wallet = getAuthContext(c)?.walletAddress?.trim();
  return wallet && wallet.length > 0 ? wallet : null;
}

type CoreMarketEnvelope = MarketRecord & {
  source: 'core';
  provider: 'core_solana' | 'core_base';
  chain: CoreChain;
  legacyMarketId?: string | null;
  providerMarketRef: string;
  requiresCredentials: boolean;
  executionUsers: boolean;
  executionAgents: boolean;
  externalUrl: string | null;
  isExternal: false;
};

function toCoreSolanaMarket(
  market: MarketRecord,
  options?: { namespacedId?: boolean; providerMarketRef?: string }
): CoreMarketEnvelope {
  const providerMarketRef = options?.providerMarketRef || market.address || market.id;
  return {
    ...market,
    id: options?.namespacedId ? toNamespacedMarketId('solana', providerMarketRef) : market.id,
    source: 'core',
    provider: 'core_solana',
    chain: 'solana',
    legacyMarketId: market.id,
    providerMarketRef,
    requiresCredentials: false,
    executionUsers: true,
    executionAgents: true,
    externalUrl: null,
    isExternal: false,
  };
}

function toCoreBaseMarket(market: CoreProjectedMarket): CoreMarketEnvelope {
  return {
    ...market,
    source: 'core',
    provider: 'core_base',
    chain: 'base',
    requiresCredentials: false,
    executionUsers: true,
    executionAgents: true,
    externalUrl: null,
    isExternal: false,
    providerMarketRef: market.marketRef,
  };
}

async function resolveCoreSolanaMarket(ref: string): Promise<MarketRecord | null> {
  if (!ref) return null;

  if (ref.startsWith('mkt-')) {
    return marketLedgerService.getMarket(ref);
  }

  const mappedLegacyId = await coreProjectionService.resolveLegacyMarket(ref);
  if (mappedLegacyId) {
    const mapped = await marketLedgerService.getMarket(mappedLegacyId);
    if (mapped) return mapped;
  }

  const direct = await marketLedgerService.getMarket(ref);
  if (direct) return direct;

  const page = await marketLedgerService.listMarkets({
    limit: 500,
    offset: 0,
    sort: 'newest',
    order: 'desc',
  });
  return page.data.find((entry) => entry.address === ref || entry.id === ref) || null;
}

function parseLiquidityError(error: unknown): {
  status: number;
  payload: Record<string, unknown>;
} {
  if (isLiquidityAllocatorError(error)) {
    return {
      status: error.status,
      payload: {
        error: error.message,
        code: error.code,
        ...(error.details ?? {}),
      },
    };
  }
  if (error instanceof Error) {
    return {
      status: 500,
      payload: { error: error.message },
    };
  }
  return {
    status: 500,
    payload: { error: 'Liquidity reservation failed' },
  };
}

function getErrorMessage(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message.trim().length > 0) return error.message;
  return fallback;
}

function parseJupiterError(error: unknown): {
  status: 404 | 502;
  payload: Record<string, unknown>;
} {
  if (isJupiterPredictionUpstreamError(error)) {
    if (error.status === 404) {
      return {
        status: 404,
        payload: { error: 'Market not found' },
      };
    }
    return {
      status: 502,
      payload: {
        error: 'Jupiter Prediction upstream unavailable',
        detail: error.detail || error.message,
        upstreamStatus: error.status,
        source: 'jupiter_prediction',
      },
    };
  }

  return {
    status: 502,
    payload: {
      error: getErrorMessage(error, 'Jupiter Prediction upstream unavailable'),
      source: 'jupiter_prediction',
    },
  };
}

function parsePnpError(error: unknown): {
  status: 404 | 502 | 503;
  payload: Record<string, unknown>;
} {
  const message = getErrorMessage(error, 'PNP upstream unavailable');
  if (message.toLowerCase().includes('disabled')) {
    return {
      status: 503,
      payload: {
        error: message,
        source: 'pnp',
      },
    };
  }
  if (message.toLowerCase().includes('not found')) {
    return {
      status: 404,
      payload: {
        error: 'Market not found',
        source: 'pnp',
      },
    };
  }
  return {
    status: 502,
    payload: {
      error: message,
      source: 'pnp',
    },
  };
}

function parseLimitlessError(error: unknown): {
  status: 404 | 502 | 503;
  payload: Record<string, unknown>;
} {
  const message = getErrorMessage(error, 'Limitless upstream unavailable');
  const lowered = message.toLowerCase();
  if (lowered.includes('disabled')) {
    return {
      status: 503,
      payload: {
        error: message,
        source: 'limitless',
      },
    };
  }
  if (lowered.includes('not found')) {
    return {
      status: 404,
      payload: {
        error: 'Market not found',
        source: 'limitless',
      },
    };
  }
  return {
    status: 502,
    payload: {
      error: message,
      source: 'limitless',
    },
  };
}

const CreateMarketBaseSchema = z.object({
  question: z.string().min(10).max(200).refine((value) => value.trim().endsWith('?'), {
    message: 'Question must end with a question mark',
  }),
  description: z.string().max(2000).optional().default(''),
  category: z.string().min(2).max(32),
  resolutionSource: z.string().min(1).max(256),
  resolutionMode: z.enum(['committee_manual', 'switchboard_objective']).default('committee_manual'),
  tradingEnd: z.string().datetime(),
  targetLiquidity: z.number().positive().optional(),
  initialLiquidity: z.number().positive().optional(),
  creator: z
    .string()
    .min(32)
    .max(64)
    .regex(/^[1-9A-HJ-NP-Za-km-z]+$/, 'Invalid creator wallet'),
  provider: z.enum(['core', 'ledger', 'pnp']).optional(),
  marketModel: z.enum(['v2_amm', 'v3_p2p']).optional(),
  collateralMint: z
    .string()
    .min(32)
    .max(64)
    .regex(/^[1-9A-HJ-NP-Za-km-z]+$/, 'Invalid collateral mint')
    .optional(),
  oracleMode: z.enum(['pnp_default', 'custom']).optional(),
  customSettler: z
    .string()
    .min(32)
    .max(64)
    .regex(/^[1-9A-HJ-NP-Za-km-z]+$/, 'Invalid custom settler wallet')
    .optional(),
  executionMode: z.enum(['user_signed', 'server_custody']).optional(),
});

const hasTargetLiquidity = (payload: {
  targetLiquidity?: number;
  initialLiquidity?: number;
}) => payload.targetLiquidity !== undefined || payload.initialLiquidity !== undefined;

const CreateMarketSchema = CreateMarketBaseSchema.refine(hasTargetLiquidity, {
  message: 'targetLiquidity is required',
  path: ['targetLiquidity'],
});

const CreateAgentMarketSchema = CreateMarketBaseSchema.extend({
  agentId: z.string().min(1).max(64),
}).refine(hasTargetLiquidity, {
  message: 'targetLiquidity is required',
  path: ['targetLiquidity'],
});

const ResolveMarketSchema = z.object({
  outcome: z.enum(['yes', 'no']),
  resolverIdentity: z.string().min(2).max(128),
  oracleSource: z.string().min(1).max(256).optional(),
  evidenceHash: z.string().min(16).max(256).optional(),
  resolutionTx: z.string().min(32).max(128).optional(),
  switchboard: z
    .object({
      feedId: z.string().min(1).max(128),
      maxAgeSeconds: z.number().int().min(1).max(86_400).optional().default(900),
      maxConfidenceBps: z.number().int().min(0).max(10_000).optional().default(200),
      observedValue: z.number().optional(),
      confidenceBps: z.number().int().min(0).max(10_000).optional(),
      publishedAt: z.string().datetime().optional(),
    })
    .optional(),
});

type CreateMarketPayload = z.infer<typeof CreateMarketSchema>;

interface OrderBookLevel {
  price: number;
  quantity: number;
  orders: number;
}

interface TradeRecord {
  id: string;
  marketId: string;
  outcome: Outcome;
  price: number;
  quantity: number;
  buyer: string;
  seller: string;
  txSignature: string;
  createdAt: string;
  source?: 'synthetic' | 'jupiter_prediction' | 'pnp' | 'limitless';
}

function getTargetLiquidity(payload: CreateMarketPayload): number {
  return payload.targetLiquidity ?? payload.initialLiquidity ?? 0;
}

function resolveProvider(payload: CreateMarketPayload): CreateMarketProvider {
  if (payload.provider) return payload.provider;
  const defaultProvider = process.env.SINGULARITY_DEFAULT_MARKET_PROVIDER?.trim().toLowerCase();
  if (defaultProvider === 'pnp') return 'pnp';
  if (defaultProvider === 'core') return 'core';
  return 'ledger';
}

function resolveExecutionMode(payload: CreateMarketPayload): ExecutionMode {
  return payload.executionMode || 'user_signed';
}

function resolveMarketModel(payload: CreateMarketPayload): MarketModel {
  return payload.marketModel || 'v2_amm';
}

function resolveOracleMode(payload: CreateMarketPayload): OracleMode {
  return payload.oracleMode || 'pnp_default';
}

function buildOrderBook(midPrice: number, depth: number): { bids: OrderBookLevel[]; asks: OrderBookLevel[] } {
  const bids: OrderBookLevel[] = [];
  const asks: OrderBookLevel[] = [];

  for (let i = 0; i < depth; i += 1) {
    const distance = (i + 1) * 0.01;
    const bidPrice = clampPrice(midPrice - distance);
    const askPrice = clampPrice(midPrice + distance);
    const bidQty = Math.max(10, (depth - i) * 35);
    const askQty = Math.max(10, (depth - i) * 32);

    bids.push({
      price: bidPrice,
      quantity: bidQty,
      orders: Math.max(1, Math.ceil(bidQty / 100)),
    });
    asks.push({
      price: askPrice,
      quantity: askQty,
      orders: Math.max(1, Math.ceil(askQty / 100)),
    });
  }

  return { bids, asks };
}

function buildSyntheticTrades(market: MarketRecord): TradeRecord[] {
  const now = Date.now();
  const seed = market.id.split('').reduce((sum, char) => sum + char.charCodeAt(0), 0);
  const entries: TradeRecord[] = [];

  for (let i = 0; i < 80; i += 1) {
    const outcome: Outcome = i % 2 === 0 ? 'yes' : 'no';
    const basePrice = outcome === 'yes' ? market.yesPrice : market.noPrice;
    const drift = (((i + seed) % 9) - 4) * 0.005;
    const price = clampPrice(basePrice + drift);
    const quantity = 50 + ((seed * 11 + i * 17) % 450);
    const createdAt = new Date(now - (i + 1 + (seed % 20)) * 60_000).toISOString();
    entries.push({
      id: `${market.id}-trade-${i + 1}`,
      marketId: market.id,
      outcome,
      price,
      quantity,
      buyer: `buyer_${seed % 997}_${i}`,
      seller: `seller_${(seed + 37) % 997}_${i}`,
      txSignature: `tx_${market.id}_${i}`,
      createdAt,
      source: 'synthetic',
    });
  }

  return entries;
}

function paginate<T>(items: T[], limit: number, offset: number) {
  const data = items.slice(offset, offset + limit);
  return {
    data,
    total: items.length,
    limit,
    offset,
    hasMore: offset + data.length < items.length,
  };
}

export const marketsRouter = new Hono();

marketsRouter.post(
  '/',
  verifyWalletSignatureStrict,
  requireFeatureEnabled('MARKETS_WRITE_ENABLED', { feature: 'markets.write' }),
  requireStakeEligible('markets.create'),
  requireIdempotencyKey,
  walletActionRateLimit('markets.create'),
  zValidator('json', CreateMarketSchema),
  async (c) => {
    const payload = c.req.valid('json');
    const walletAddress = getAuthWallet(c);
    if (!walletAddress) return c.json({ error: 'Missing signed wallet auth' }, 401);
    if (payload.creator !== walletAddress) {
      return c.json({ error: 'creator must match signed wallet' }, 403);
    }

    const idempotencyKey = getIdempotencyKey(c);
    if (!idempotencyKey) return c.json({ error: 'Missing idempotency key' }, 400);

    const existing = await executionService.getExecutionByIdempotencyKey(idempotencyKey);
    if (existing?.response) {
      return c.json(existing.response, existing.status === 'confirmed' ? 201 : 409);
    }

    const execution = await executionService.begin({
      idempotencyKey,
      walletAddress,
      action: 'markets.create',
      request: payload as unknown as Record<string, unknown>,
    });

    try {
      const provider = resolveProvider(payload);
      const executionMode = resolveExecutionMode(payload);
      const marketModel = resolveMarketModel(payload);
      const oracleMode = resolveOracleMode(payload);
      let market: MarketRecord | Record<string, unknown> | null = null;

      if (provider === 'pnp') {
        const created = await pnpExecutionService.createMarket({
          walletAddress,
          question: payload.question,
          category: payload.category,
          tradingEnd: payload.tradingEnd,
          targetLiquidityMicro: getTargetLiquidity(payload),
          collateralMint: payload.collateralMint || DEFAULT_COLLATERAL_MINT,
          marketModel,
          oracleMode,
          customSettler: payload.customSettler || null,
          executionMode,
        });
        market = await pnpMarketService.getMarket(created.marketId);
      } else {
        if (!isSyntheticLedgerWriteEnabled()) {
          throw new Error(syntheticLedgerWriteBlockReason('markets.create'));
        }
        market = await marketLedgerService.createMarket({
          question: payload.question,
          description: payload.description,
          category: payload.category,
          resolutionSource: payload.resolutionSource,
          resolutionMode: payload.resolutionMode,
          tradingEnd: payload.tradingEnd,
          targetLiquidity: getTargetLiquidity(payload),
          creator: payload.creator,
        });
      }

      if (!market) {
        throw new Error('Failed to load market after create');
      }

      const response = {
        market:
          provider === 'pnp'
            ? market
            : toCoreSolanaMarket(market as MarketRecord, {
                namespacedId: provider === 'core',
              }),
        provider,
        source: provider === 'pnp' ? 'pnp' : provider === 'ledger' ? 'ledger' : 'core',
        chain: provider === 'pnp' ? undefined : 'solana',
        executionId: execution.id,
        idempotencyKey,
      };
      await executionService.complete({
        executionId: execution.id,
        status: 'confirmed',
        response: response as unknown as Record<string, unknown>,
      });

      return c.json(response, 201);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      await executionService.complete({
        executionId: execution.id,
        status: 'failed',
        error: message,
      });
      if (
        message.includes('tradingEnd') ||
        message.includes('timestamp') ||
        message.includes('required') ||
        message.includes('Invalid')
      ) {
        return c.json({ error: message }, 400);
      }
      if (message.includes('disabled')) {
        return c.json({ error: message }, 503);
      }
      return c.json({ error: message }, 500);
    }
  }
);

marketsRouter.post(
  '/agent',
  verifyWalletSignatureStrict,
  requireFeatureEnabled('MARKETS_WRITE_ENABLED', { feature: 'markets.write' }),
  requireStakeEligible('markets.create_agent'),
  requireIdempotencyKey,
  walletActionRateLimit('markets.create_agent'),
  zValidator('json', CreateAgentMarketSchema),
  async (c) => {
    const payload = c.req.valid('json');
    const walletAddress = getAuthWallet(c);
    if (!walletAddress) return c.json({ error: 'Missing signed wallet auth' }, 401);
    const idempotencyKey = getIdempotencyKey(c);
    if (!idempotencyKey) return c.json({ error: 'Missing idempotency key' }, 400);

    const existing = await executionService.getExecutionByIdempotencyKey(idempotencyKey);
    if (existing?.response) {
      return c.json(existing.response, existing.status === 'confirmed' ? 201 : 409);
    }

    const agent = agentService.getById(payload.agentId);
    if (!agent) return c.json({ error: 'Agent not found' }, 404);
    if (!agent.isActive) return c.json({ error: 'Agent is inactive' }, 403);
    if (agent.walletAddress !== walletAddress) {
      return c.json({ error: 'Agent wallet must match signed wallet' }, 403);
    }
    if (payload.creator !== walletAddress) {
      return c.json({ error: 'creator must match signed wallet' }, 403);
    }

    const allowedCategories = agent.policy?.allowedCategories ?? [];
    const category = payload.category.trim().toLowerCase();
    if (allowedCategories.length > 0 && !allowedCategories.includes(category)) {
      return c.json({ error: 'Category is not allowed by agent policy' }, 403);
    }

    const requestedTargetLiquidity = Math.round(getTargetLiquidity(payload));
    const maxInitialLiquidity = agent.policy?.maxInitialLiquidity ?? 1_000_000_000;
    if (requestedTargetLiquidity > maxInitialLiquidity) {
      return c.json({ error: 'Target liquidity exceeds agent policy limit' }, 403);
    }

    const execution = await executionService.begin({
      idempotencyKey,
      walletAddress,
      action: 'markets.create_agent',
      request: payload as unknown as Record<string, unknown>,
    });

    try {
      const provider = resolveProvider(payload);
      const executionMode = resolveExecutionMode(payload);
      const marketModel = resolveMarketModel(payload);
      const oracleMode = resolveOracleMode(payload);
      let market: MarketRecord | Record<string, unknown> | null = null;

      if (provider === 'pnp') {
        const created = await pnpExecutionService.createMarket({
          walletAddress,
          question: payload.question,
          category: payload.category,
          tradingEnd: payload.tradingEnd,
          targetLiquidityMicro: getTargetLiquidity(payload),
          collateralMint: payload.collateralMint || DEFAULT_COLLATERAL_MINT,
          marketModel,
          oracleMode,
          customSettler: payload.customSettler || null,
          executionMode,
        });
        market = await pnpMarketService.getMarket(created.marketId);
      } else {
        if (!isSyntheticLedgerWriteEnabled()) {
          throw new Error(syntheticLedgerWriteBlockReason('markets.create_agent'));
        }
        market = await marketLedgerService.createMarket({
          question: payload.question,
          description: payload.description,
          category: payload.category,
          resolutionSource: payload.resolutionSource,
          resolutionMode: payload.resolutionMode,
          tradingEnd: payload.tradingEnd,
          targetLiquidity: getTargetLiquidity(payload),
          creator: payload.creator,
        });
      }

      if (!market) {
        throw new Error('Failed to load market after create');
      }

      const receipt = await receiptService.create({
        agent,
        kind: 'market_created',
        summary: `agent created ${provider} market ${String((market as { id?: string }).id ?? 'unknown')}`,
        payload: {
          marketId: String((market as { id?: string }).id ?? ''),
          question: String((market as { question?: string }).question ?? payload.question),
          category: String((market as { category?: string }).category ?? payload.category),
          targetLiquidity: Number(
            (market as { totalCollateral?: number }).totalCollateral ?? getTargetLiquidity(payload)
          ),
          creator: payload.creator,
          resolutionMode: String((market as { resolutionMode?: string }).resolutionMode ?? payload.resolutionMode),
          provider,
          marketModel: provider === 'pnp' ? marketModel : undefined,
          executionMode: provider === 'pnp' ? executionMode : undefined,
          executionId: execution.id,
        },
        idempotencyKey,
      });

      const response = {
        market:
          provider === 'pnp'
            ? market
            : toCoreSolanaMarket(market as MarketRecord, {
                namespacedId: provider === 'core',
              }),
        receipt,
        provider,
        source: provider === 'pnp' ? 'pnp' : provider === 'ledger' ? 'ledger' : 'core',
        chain: provider === 'pnp' ? undefined : 'solana',
        executionId: execution.id,
        idempotencyKey,
      };

      await executionService.complete({
        executionId: execution.id,
        status: 'confirmed',
        response: response as unknown as Record<string, unknown>,
      });

      return c.json(response, 201);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      await executionService.complete({
        executionId: execution.id,
        status: 'failed',
        error: message,
      });
      if (
        message.includes('tradingEnd') ||
        message.includes('timestamp') ||
        message.includes('required') ||
        message.includes('Invalid')
      ) {
        return c.json({ error: message }, 400);
      }
      if (message.includes('disabled')) {
        return c.json({ error: message }, 503);
      }
      return c.json({ error: message }, 500);
    }
  }
);

marketsRouter.get('/', async (c) => {
  const category = c.req.query('category')?.trim().toLowerCase();
  const status = c.req.query('status')?.trim().toLowerCase();
  const limit = parsePositiveInt(c.req.query('limit'), 50, { min: 1, max: 100 });
  const offset = parseOffset(c.req.query('offset'));
  const sort = parseSort(c.req.query('sort'));
  const order = parseSortOrder(c.req.query('order'));
  const sourceInput = parseMarketSource(c.req.query('source'));
  const source = isLegacyLedgerAlias(sourceInput) ? 'core' : sourceInput;
  const chainQuery = parseChainQuery(c.req.query('chain'));
  const effectiveChain: CoreChainQuery = isLegacyLedgerAlias(sourceInput) ? 'solana' : chainQuery;
  const includeLowLiquidity = parseBoolean(c.req.query('includeLowLiquidity'));
  const marketModel = parseMarketModel(c.req.query('model'));
  const collateralMint = c.req.query('collateralMint')?.trim();
  const namespacedCoreIds = source === 'core';

  const includeCoreSolana =
    (source === 'core' || source === 'all') &&
    (effectiveChain === 'solana' || effectiveChain === 'all');
  const includeCoreBase =
    (source === 'core' || source === 'all') &&
    (effectiveChain === 'base' || effectiveChain === 'all');

  const warnings: Array<{ source: string; message: string }> = [];

  let coreSolanaMarkets: CoreMarketEnvelope[] = [];
  if (includeCoreSolana) {
    const solanaPage = await marketLedgerService.listMarkets({
      category,
      status,
      limit: source === 'all' ? 500 : Math.max(limit + offset, 200),
      offset: 0,
      sort,
      order,
    });
    coreSolanaMarkets = solanaPage.data.map((market) =>
      toCoreSolanaMarket(market, {
        namespacedId: namespacedCoreIds,
      })
    );
  }

  let coreBaseMarkets: CoreMarketEnvelope[] = [];
  if (includeCoreBase) {
    try {
      const baseProjected = await coreProjectionService.listBaseMarkets({
        category,
        status,
        limit: source === 'all' ? 500 : Math.max(limit + offset, 200),
        offset: 0,
      });
      coreBaseMarkets = baseProjected.map(toCoreBaseMarket);
    } catch (error) {
      warnings.push({
        source: 'core_base',
        message: getErrorMessage(error, 'Base core projection unavailable'),
      });
    }
  }

  if (isLegacyLedgerAlias(sourceInput)) {
    return c.json({
      ...paginate(coreSolanaMarkets.map((entry) => ({ ...entry, id: entry.legacyMarketId || entry.id })), limit, offset),
      source: 'ledger',
      chain: 'solana',
      alias: 'source=core&chain=solana',
    });
  }

  if (source === 'core') {
    const coreMerged = [...coreSolanaMarkets, ...coreBaseMarkets];
    coreMerged.sort((left, right) => {
      if (sort === 'newest') {
        const delta = Date.parse(left.createdAt) - Date.parse(right.createdAt);
        return order === 'asc' ? delta : -delta;
      }
      if (sort === 'ending') {
        const delta = Date.parse(left.tradingEnd) - Date.parse(right.tradingEnd);
        return order === 'asc' ? delta : -delta;
      }
      const delta = left.volume24h - right.volume24h;
      return order === 'asc' ? delta : -delta;
    });

    return c.json({
      ...paginate(coreMerged, limit, offset),
      source: 'core',
      chain: effectiveChain,
      ...(warnings.length > 0 ? { warnings } : {}),
    });
  }

  if (source === 'jupiter') {
    const blocked = requireProviderAllowed(c, 'jupiter_prediction', 'feed');
    if (blocked) return blocked;
  }
  if (source === 'limitless') {
    const blocked = requireProviderAllowed(c, 'limitless', 'feed');
    if (blocked) return blocked;
  }

  let pnpMarkets: MarketRecord[] = [];
  let pnpError: unknown = null;
  if (source !== 'jupiter' && source !== 'limitless' && pnpMarketService.isEnabled()) {
    try {
      pnpMarkets = await pnpMarketService.listMarkets({
        category,
        status,
        sort,
        order,
        marketModel,
        collateralMint: collateralMint || undefined,
      });
    } catch (error) {
      pnpError = error;
    }
  }

  if (source === 'pnp') {
    if (!pnpMarketService.isEnabled()) {
      return c.json({
        ...paginate([], limit, offset),
        source: 'pnp',
      });
    }
    if (pnpError) {
      const parsed = parsePnpError(pnpError);
      return c.json(parsed.payload, parsed.status);
    }
    return c.json({
      ...paginate(pnpMarkets, limit, offset),
      source: 'pnp',
    });
  }

  let limitlessMarkets: MarketRecord[] = [];
  let limitlessError: unknown = null;
  const canUseLimitlessFeed = isProviderAllowed(c, 'limitless', 'feed');
  if (source !== 'jupiter' && limitlessMarketService.isEnabled() && canUseLimitlessFeed) {
    try {
      limitlessMarkets = await limitlessMarketService.listMarkets({
        category,
        status,
        sort,
        order,
        includeLowLiquidity,
      });
    } catch (error) {
      limitlessError = error;
    }
  } else if (!canUseLimitlessFeed) {
    noteProviderRestriction(c, 'limitless', 'feed');
  }

  if (source === 'limitless') {
    if (!limitlessMarketService.isEnabled()) {
      return c.json({
        ...paginate([], limit, offset),
        source: 'limitless',
      });
    }
    if (limitlessError) {
      const parsed = parseLimitlessError(limitlessError);
      return c.json(parsed.payload, parsed.status);
    }
    return c.json({
      ...paginate(limitlessMarkets, limit, offset),
      source: 'limitless',
    });
  }

  let jupiterMarkets: MarketRecord[] = [];
  let jupiterError: unknown = null;
  const canUseJupiterFeed = isProviderAllowed(c, 'jupiter_prediction', 'feed');
  if (jupiterPredictionService.isEnabled() && canUseJupiterFeed) {
    try {
      jupiterMarkets = await jupiterPredictionService.listMarkets({
        category,
        status,
        sort,
        order,
      });
    } catch (error) {
      jupiterError = error;
    }
  } else if (!canUseJupiterFeed) {
    noteProviderRestriction(c, 'jupiter_prediction', 'feed');
  }

  if (source === 'jupiter') {
    if (jupiterError) {
      const parsed = parseJupiterError(jupiterError);
      return c.json(parsed.payload, parsed.status);
    }
    return c.json({
      ...paginate(jupiterMarkets, limit, offset),
      source: 'jupiter_prediction',
    });
  }

  if (jupiterError) {
    warnings.push({
      source: 'jupiter_prediction',
      message: getErrorMessage(jupiterError, 'Jupiter Prediction upstream unavailable'),
    });
  } else if (jupiterPredictionService.isEnabled() && !canUseJupiterFeed) {
    warnings.push({
      source: 'jupiter_prediction',
      message: 'Jupiter Prediction feed unavailable in your region',
    });
  }
  if (pnpError) {
    warnings.push({
      source: 'pnp',
      message: getErrorMessage(pnpError, 'PNP upstream unavailable'),
    });
  }
  if (limitlessError) {
    warnings.push({
      source: 'limitless',
      message: getErrorMessage(limitlessError, 'Limitless upstream unavailable'),
    });
  } else if (limitlessMarketService.isEnabled() && !canUseLimitlessFeed) {
    warnings.push({
      source: 'limitless',
      message: 'Limitless feed unavailable in your region',
    });
  }

  const merged = [...coreSolanaMarkets, ...coreBaseMarkets, ...jupiterMarkets, ...pnpMarkets, ...limitlessMarkets];
  merged.sort((left, right) => {
    if (sort === 'newest') {
      const delta = Date.parse(left.createdAt) - Date.parse(right.createdAt);
      return order === 'asc' ? delta : -delta;
    }
    if (sort === 'ending') {
      const delta = Date.parse(left.tradingEnd) - Date.parse(right.tradingEnd);
      return order === 'asc' ? delta : -delta;
    }
    const delta = left.volume24h - right.volume24h;
    return order === 'asc' ? delta : -delta;
  });

  const page = paginate(merged, limit, offset);
  const hasExternalOrBaseCore = merged.some((entry) => {
    const sourceTag = String((entry as { source?: string }).source || 'ledger');
    const chainTag = String((entry as { chain?: string }).chain || '');
    return (
      sourceTag === 'pnp' ||
      sourceTag === 'jupiter_prediction' ||
      sourceTag === 'limitless' ||
      chainTag === 'base'
    );
  });
  return c.json({
    ...page,
    source: hasExternalOrBaseCore ? 'hybrid' : 'ledger',
    chain: 'all',
    ...(warnings.length > 0 ? { warnings } : {}),
  });
});

marketsRouter.get('/admin/overview', verifyWalletSignatureStrict, async (c) => {
  const admin = requireAdminWallet(c);
  if ('error' in admin) return c.json({ error: admin.error }, admin.status);

  const [stats, pendingMarkets, liquidity] = await Promise.all([
    marketLedgerService.getAdminStats(),
    marketLedgerService.listPendingMarkets(),
    liquidityAllocatorService.getSnapshot(),
  ]);
  const pendingDisputes = await disputeLedgerService.countActiveDisputes();

  return c.json({
    stats: {
      ...stats,
      pendingDisputes,
    },
    pendingMarkets,
    liquidity,
  });
});

marketsRouter.post(
  '/admin/:id/approve',
  verifyWalletSignatureStrict,
  requireFeatureEnabled('MARKETS_WRITE_ENABLED', { feature: 'markets.write' }),
  requireIdempotencyKey,
  walletActionRateLimit('markets.admin_approve'),
  async (c) => {
    const admin = requireAdminWallet(c);
    if ('error' in admin) return c.json({ error: admin.error }, admin.status);
    const idempotencyKey = getIdempotencyKey(c);
    if (!idempotencyKey) return c.json({ error: 'Missing idempotency key' }, 400);

    const existing = await executionService.getExecutionByIdempotencyKey(idempotencyKey);
    if (existing?.response) {
      return c.json(existing.response, existing.status === 'confirmed' ? 200 : 409);
    }

    const marketId = c.req.param('id');
    const execution = await executionService.begin({
      idempotencyKey,
      walletAddress: admin.walletAddress,
      action: 'markets.admin_approve',
      resourceId: marketId,
    });

    const pendingMarkets = await marketLedgerService.listPendingMarkets();
    const pendingEntry = pendingMarkets.find((entry) => entry.id === marketId);
    if (!pendingEntry) {
      await executionService.complete({
        executionId: execution.id,
        status: 'failed',
        error: 'Market not pending approval',
      });
      return c.json({ error: 'Market not pending approval' }, 404);
    }

    const pendingMarket = await marketLedgerService.getMarket(marketId);
    if (!pendingMarket) {
      await executionService.complete({
        executionId: execution.id,
        status: 'failed',
        error: 'Market not found',
      });
      return c.json({ error: 'Market not found' }, 404);
    }

    let liquidityPlan;
    try {
      liquidityPlan = await liquidityAllocatorService.reserveForPendingMarket({
        marketId,
        creatorWallet: pendingEntry.creator,
        targetLiquidity: pendingMarket.totalCollateral,
      });
    } catch (error) {
      const parsed = parseLiquidityError(error);
      await executionService.complete({
        executionId: execution.id,
        status: 'failed',
        error: String(parsed.payload.error || 'Liquidity reservation failed'),
      });
      return c.json(parsed.payload, parsed.status as 400 | 401 | 403 | 404 | 409 | 422 | 500 | 503);
    }

    const market = await marketLedgerService.approvePendingMarket(marketId);
    if (!market) {
      await liquidityAllocatorService.releaseReservation(marketId);
      await executionService.complete({
        executionId: execution.id,
        status: 'failed',
        error: 'Market not pending approval',
      });
      return c.json({ error: 'Market not pending approval' }, 404);
    }

    const response = {
      market,
      liquidityPlan,
      executionId: execution.id,
      idempotencyKey,
      approvedBy: admin.walletAddress,
    };
    await executionService.complete({
      executionId: execution.id,
      status: 'confirmed',
      response: response as unknown as Record<string, unknown>,
    });

    return c.json(response);
  }
);

marketsRouter.post(
  '/admin/:id/reject',
  verifyWalletSignatureStrict,
  requireFeatureEnabled('MARKETS_WRITE_ENABLED', { feature: 'markets.write' }),
  requireIdempotencyKey,
  walletActionRateLimit('markets.admin_reject'),
  async (c) => {
    const admin = requireAdminWallet(c);
    if ('error' in admin) return c.json({ error: admin.error }, admin.status);
    const idempotencyKey = getIdempotencyKey(c);
    if (!idempotencyKey) return c.json({ error: 'Missing idempotency key' }, 400);

    const existing = await executionService.getExecutionByIdempotencyKey(idempotencyKey);
    if (existing?.response) {
      return c.json(existing.response, existing.status === 'confirmed' ? 200 : 409);
    }

    const marketId = c.req.param('id');
    const execution = await executionService.begin({
      idempotencyKey,
      walletAddress: admin.walletAddress,
      action: 'markets.admin_reject',
      resourceId: marketId,
    });

    const market = await marketLedgerService.rejectPendingMarket(marketId);
    if (!market) {
      await executionService.complete({
        executionId: execution.id,
        status: 'failed',
        error: 'Market not pending approval',
      });
      return c.json({ error: 'Market not pending approval' }, 404);
    }

    const response = {
      market,
      executionId: execution.id,
      idempotencyKey,
      rejectedBy: admin.walletAddress,
    };
    await executionService.complete({
      executionId: execution.id,
      status: 'confirmed',
      response: response as unknown as Record<string, unknown>,
    });

    return c.json(response);
  }
);

marketsRouter.post(
  '/admin/:id/resolve',
  verifyWalletSignatureStrict,
  requireFeatureEnabled('MARKETS_WRITE_ENABLED', { feature: 'markets.write' }),
  requireIdempotencyKey,
  walletActionRateLimit('markets.admin_resolve'),
  zValidator('json', ResolveMarketSchema),
  async (c) => {
    const admin = requireAdminWallet(c);
    if ('error' in admin) return c.json({ error: admin.error }, admin.status);
    const idempotencyKey = getIdempotencyKey(c);
    if (!idempotencyKey) return c.json({ error: 'Missing idempotency key' }, 400);

    const existing = await executionService.getExecutionByIdempotencyKey(idempotencyKey);
    if (existing?.response) {
      return c.json(existing.response, existing.status === 'confirmed' ? 200 : 409);
    }

    const marketId = c.req.param('id');
    const market = await marketLedgerService.getMarket(marketId);
    if (!market) return c.json({ error: 'Market not found' }, 404);
    if (market.status === 'resolved' || market.status === 'cancelled') {
      return c.json({ error: 'Market is in terminal state' }, 409);
    }

    const payload = c.req.valid('json');
    const execution = await executionService.begin({
      idempotencyKey,
      walletAddress: admin.walletAddress,
      action: 'markets.admin_resolve',
      resourceId: marketId,
      request: payload as unknown as Record<string, unknown>,
    });

    let evidenceHash = payload.evidenceHash;
    let oracleSource = payload.oracleSource?.trim() || market.oracle;
    let oracleProof: Record<string, unknown> | undefined;

    if (market.resolutionMode === 'switchboard_objective') {
      if (!payload.switchboard) {
        await executionService.complete({
          executionId: execution.id,
          status: 'failed',
          error: 'switchboard payload required for objective resolution mode',
        });
        return c.json({ error: 'switchboard payload required' }, 422);
      }
      const verification = await switchboardService.validateObjectiveResolution(payload.switchboard);
      if (!verification.ok || !verification.snapshot) {
        const message = verification.error || 'switchboard validation failed';
        await executionService.complete({
          executionId: execution.id,
          status: 'failed',
          error: message,
        });
        return c.json({ error: message }, 422);
      }
      oracleSource = `switchboard:${verification.snapshot.feedId}`;
      if (!evidenceHash) {
        evidenceHash = crypto
          .createHash('sha256')
          .update(JSON.stringify(verification.snapshot.raw))
          .digest('hex');
      }
      oracleProof = {
        snapshot: verification.snapshot,
      };
    } else if (!evidenceHash) {
      await executionService.complete({
        executionId: execution.id,
        status: 'failed',
        error: 'evidenceHash is required for committee_manual resolution mode',
      });
      return c.json({ error: 'evidenceHash is required for committee_manual resolution mode' }, 422);
    }

    const resolved = await marketLedgerService.resolveMarket(marketId, {
      outcome: payload.outcome,
      resolverIdentity: payload.resolverIdentity,
      oracleSource,
      evidenceHash,
      resolutionTx: payload.resolutionTx,
    });

    if (!resolved) {
      await executionService.complete({
        executionId: execution.id,
        status: 'failed',
        error: 'Market not found',
      });
      return c.json({ error: 'Market not found' }, 404);
    }

    await executionService.appendEvent(execution.id, 'market_resolved', {
      marketId,
      resolutionMode: market.resolutionMode,
      oracleSource,
      evidenceHash,
      outcome: payload.outcome,
      proof: oracleProof,
    });

    const response = {
      market: resolved,
      executionId: execution.id,
      idempotencyKey,
      resolvedBy: admin.walletAddress,
    };
    await executionService.complete({
      executionId: execution.id,
      status: 'confirmed',
      txSignature: payload.resolutionTx ?? null,
      response: response as unknown as Record<string, unknown>,
    });

    return c.json(response);
  }
);

marketsRouter.get('/:id', async (c) => {
  const id = c.req.param('id');
  const parsedRef = parseMarketRef(id);

  if (parsedRef.chain === 'solana') {
    const coreMarket = await resolveCoreSolanaMarket(parsedRef.coreRef);
    if (!coreMarket) return c.json({ error: 'Market not found' }, 404);
    return c.json(
      toCoreSolanaMarket(coreMarket, {
        namespacedId: parsedRef.namespaced,
        providerMarketRef: parsedRef.coreRef,
      })
    );
  }

  if (parsedRef.chain === 'base') {
    const baseMarket = await coreProjectionService.getBaseMarketByRef(parsedRef.coreRef);
    if (!baseMarket) return c.json({ error: 'Market not found' }, 404);
    return c.json(toCoreBaseMarket(baseMarket));
  }

  if (limitlessMarketService.isLocalMarketId(id)) {
    const blocked = requireProviderAllowed(c, 'limitless', 'market_data');
    if (blocked) return blocked;
    try {
      const limitlessMarket = await limitlessMarketService.getMarket(id);
      if (!limitlessMarket) return c.json({ error: 'Market not found' }, 404);
      return c.json(limitlessMarket);
    } catch (error) {
      const parsed = parseLimitlessError(error);
      return c.json(parsed.payload, parsed.status);
    }
  }
  if (pnpMarketService.isLocalMarketId(id)) {
    try {
      const pnpMarket = await pnpMarketService.getMarket(id);
      if (!pnpMarket) return c.json({ error: 'Market not found' }, 404);
      return c.json(pnpMarket);
    } catch (error) {
      const parsed = parsePnpError(error);
      return c.json(parsed.payload, parsed.status);
    }
  }
  if (jupiterPredictionService.isLocalMarketId(id)) {
    try {
      const jupiterMarket = await jupiterPredictionService.getMarket(id);
      if (!jupiterMarket) return c.json({ error: 'Market not found' }, 404);
      return c.json(jupiterMarket);
    } catch (error) {
      const parsed = parseJupiterError(error);
      return c.json(parsed.payload, parsed.status);
    }
  }

  const market = await marketLedgerService.getMarket(id);
  if (!market) return c.json({ error: 'Market not found' }, 404);
  return c.json(
    toCoreSolanaMarket(market, {
      namespacedId: false,
    })
  );
});

marketsRouter.get('/:id/orderbook', async (c) => {
  const marketId = c.req.param('id');
  const parsedRef = parseMarketRef(marketId);
  const isLimitlessMarket = limitlessMarketService.isLocalMarketId(marketId);
  const isPnpMarket = pnpMarketService.isLocalMarketId(marketId);
  const isJupiterMarket = jupiterPredictionService.isLocalMarketId(marketId);
  let market: MarketRecord | null = null;
  let coreChain: CoreChain | null = null;
  let coreRef: string | null = null;

  if (parsedRef.chain === 'solana') {
    market = await resolveCoreSolanaMarket(parsedRef.coreRef);
    if (!market) return c.json({ error: 'Market not found' }, 404);
    coreChain = 'solana';
    coreRef = parsedRef.coreRef;
  } else if (parsedRef.chain === 'base') {
    const baseMarket = await coreProjectionService.getBaseMarketByRef(parsedRef.coreRef);
    if (!baseMarket) return c.json({ error: 'Market not found' }, 404);
    coreChain = 'base';
    coreRef = parsedRef.coreRef;
  } else if (!isLimitlessMarket && !isPnpMarket && !isJupiterMarket) {
    market = await marketLedgerService.getMarket(marketId);
    if (!market) return c.json({ error: 'Market not found' }, 404);
    coreChain = 'solana';
    coreRef = market.id;
  }

  const rawOutcome = c.req.query('outcome');
  const outcome = parseOutcome(rawOutcome) ?? 'yes';
  if (rawOutcome && !parseOutcome(rawOutcome)) {
    return c.json({ error: 'Invalid outcome' }, 400);
  }

  const depth = parsePositiveInt(c.req.query('depth'), 20, { min: 1, max: 100 });
  if (coreChain === 'base') {
    return c.json({
      source: 'core',
      provider: 'core_base',
      chain: 'base',
      providerMarketRef: coreRef,
      marketId: toNamespacedMarketId('base', coreRef || marketId),
      outcome,
      bids: [],
      asks: [],
      isSynthetic: true,
      lastUpdated: new Date().toISOString(),
    });
  }
  if (isLimitlessMarket) {
    const blocked = requireProviderAllowed(c, 'limitless', 'market_data');
    if (blocked) return blocked;
    try {
      const book = await limitlessMarketService.getOrderbook(marketId, outcome, depth);
      if (!book) return c.json({ error: 'Market not found' }, 404);
      return c.json(book);
    } catch (error) {
      const parsed = parseLimitlessError(error);
      return c.json(parsed.payload, parsed.status);
    }
  }
  if (isPnpMarket) {
    try {
      const book = await pnpMarketService.getOrderbook(marketId, outcome, depth);
      if (!book) return c.json({ error: 'Market not found' }, 404);
      return c.json({
        ...book,
        bids: book.bids.slice(0, depth),
        asks: book.asks.slice(0, depth),
      });
    } catch (error) {
      const parsed = parsePnpError(error);
      return c.json(parsed.payload, parsed.status);
    }
  }
  if (isJupiterMarket) {
    const blocked = requireProviderAllowed(c, 'jupiter_prediction', 'market_data');
    if (blocked) return blocked;
    try {
      const book = await jupiterPredictionService.getOrderbook(marketId, outcome);
      if (!book) return c.json({ error: 'Market not found' }, 404);
      return c.json({
        ...book,
        bids: book.bids.slice(0, depth),
        asks: book.asks.slice(0, depth),
      });
    } catch (error) {
      const parsed = parseJupiterError(error);
      return c.json(parsed.payload, parsed.status);
    }
  }
  if (!market) return c.json({ error: 'Market not found' }, 404);
  const ledgerMarket = market;

  const pnpOrderBook = await pnpLiquidityService.getOrderBook(ledgerMarket.id, outcome, depth);
  if (pnpOrderBook) {
    return c.json(pnpOrderBook);
  }

  const midPrice = outcome === 'yes' ? ledgerMarket.yesPrice : ledgerMarket.noPrice;
  const { bids, asks } = buildOrderBook(midPrice, depth);

  return c.json({
    source: parsedRef.namespaced ? 'core' : 'synthetic',
    provider: 'core_solana',
    chain: 'solana',
    providerMarketRef: coreRef || ledgerMarket.id,
    marketId: parsedRef.namespaced
      ? toNamespacedMarketId('solana', coreRef || ledgerMarket.id)
      : ledgerMarket.id,
    outcome,
    bids,
    asks,
    isSynthetic: true,
    lastUpdated: new Date().toISOString(),
  });
});

marketsRouter.get('/:id/trades', async (c) => {
  const marketId = c.req.param('id');
  const parsedRef = parseMarketRef(marketId);
  const isLimitlessMarket = limitlessMarketService.isLocalMarketId(marketId);
  const isPnpMarket = pnpMarketService.isLocalMarketId(marketId);
  const isJupiterMarket = jupiterPredictionService.isLocalMarketId(marketId);
  let market: MarketRecord | null = null;
  let coreChain: CoreChain | null = null;
  let coreRef: string | null = null;

  if (parsedRef.chain === 'solana') {
    market = await resolveCoreSolanaMarket(parsedRef.coreRef);
    if (!market) return c.json({ error: 'Market not found' }, 404);
    coreChain = 'solana';
    coreRef = parsedRef.coreRef;
  } else if (parsedRef.chain === 'base') {
    const baseMarket = await coreProjectionService.getBaseMarketByRef(parsedRef.coreRef);
    if (!baseMarket) return c.json({ error: 'Market not found' }, 404);
    coreChain = 'base';
    coreRef = parsedRef.coreRef;
  } else if (!isLimitlessMarket && !isPnpMarket && !isJupiterMarket) {
    market = await marketLedgerService.getMarket(marketId);
    if (!market) return c.json({ error: 'Market not found' }, 404);
    coreChain = 'solana';
    coreRef = market.id;
  }

  const rawOutcome = c.req.query('outcome');
  const outcome = parseOutcome(rawOutcome);
  if (rawOutcome && !outcome) {
    return c.json({ error: 'Invalid outcome' }, 400);
  }

  const limit = parsePositiveInt(c.req.query('limit'), 20, { min: 1, max: 100 });
  const before = c.req.query('before');
  if (before) {
    const beforeMs = Date.parse(before);
    if (Number.isNaN(beforeMs)) return c.json({ error: 'Invalid before timestamp' }, 400);
  }

  if (coreChain === 'base') {
    return c.json({
      source: 'core',
      provider: 'core_base',
      chain: 'base',
      providerMarketRef: coreRef,
      marketId: toNamespacedMarketId('base', coreRef || marketId),
      isSynthetic: true,
      data: [],
      total: 0,
      limit,
      offset: 0,
      hasMore: false,
    });
  }

  if (isPnpMarket) {
    try {
      let trades = await pnpExecutionService.listTrades(marketId, Math.max(100, limit));
      if (trades.length === 0) {
        trades = await pnpMarketService.listTrades(marketId, Math.max(100, limit));
      }
      if (outcome) trades = trades.filter((trade) => trade.outcome === outcome);
      if (before) {
        const beforeMs = Date.parse(before);
        trades = trades.filter((trade) => Date.parse(trade.createdAt) < beforeMs);
      }
      const data = trades.slice(0, limit);
      return c.json({
        source: 'pnp',
        data,
        total: trades.length,
        limit,
        offset: 0,
        hasMore: trades.length > limit,
      });
    } catch (error) {
      const parsed = parsePnpError(error);
      return c.json(parsed.payload, parsed.status);
    }
  }

  if (isLimitlessMarket) {
    const blocked = requireProviderAllowed(c, 'limitless', 'market_data');
    if (blocked) return blocked;
    try {
      let trades = await limitlessExecutionService.listTrades(marketId, Math.max(100, limit));
      if (trades.length === 0) {
        trades = await limitlessMarketService.listTrades(marketId, Math.max(100, limit));
      }
      if (outcome) trades = trades.filter((trade) => trade.outcome === outcome);
      if (before) {
        const beforeMs = Date.parse(before);
        trades = trades.filter((trade) => Date.parse(trade.createdAt) < beforeMs);
      }

      const data = trades.slice(0, limit);
      return c.json({
        source: 'limitless',
        data,
        total: trades.length,
        limit,
        offset: 0,
        hasMore: trades.length > limit,
      });
    } catch (error) {
      const parsed = parseLimitlessError(error);
      return c.json(parsed.payload, parsed.status);
    }
  }

  if (isJupiterMarket) {
    const blocked = requireProviderAllowed(c, 'jupiter_prediction', 'market_data');
    if (blocked) return blocked;
    try {
      let trades = await jupiterPredictionService.listTrades(marketId, Math.max(100, limit));
      if (outcome) trades = trades.filter((trade) => trade.outcome === outcome);
      if (before) {
        const beforeMs = Date.parse(before);
        trades = trades.filter((trade) => Date.parse(trade.createdAt) < beforeMs);
      }

      const data = trades.slice(0, limit);
      return c.json({
        source: 'jupiter_prediction',
        data,
        total: trades.length,
        limit,
        offset: 0,
        hasMore: trades.length > limit,
      });
    } catch (error) {
      const parsed = parseJupiterError(error);
      return c.json(parsed.payload, parsed.status);
    }
  }
  if (!market) return c.json({ error: 'Market not found' }, 404);

  let trades = buildSyntheticTrades(market);
  if (outcome) trades = trades.filter((trade) => trade.outcome === outcome);

  if (before) {
    const beforeMs = Date.parse(before);
    trades = trades.filter((trade) => Date.parse(trade.createdAt) < beforeMs);
  }

  const data = trades.slice(0, limit);
  return c.json({
    source: parsedRef.namespaced ? 'core' : 'synthetic',
    provider: 'core_solana',
    chain: 'solana',
    providerMarketRef: coreRef || market.id,
    marketId: parsedRef.namespaced
      ? toNamespacedMarketId('solana', coreRef || market.id)
      : market.id,
    isSynthetic: true,
    data: data.map((trade) => ({
      ...trade,
      marketId: parsedRef.namespaced
        ? toNamespacedMarketId('solana', coreRef || trade.marketId)
        : trade.marketId,
    })),
    total: trades.length,
    limit,
    offset: 0,
    hasMore: trades.length > limit,
  });
});
