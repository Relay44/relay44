import { Hono } from 'hono';
import { zValidator } from '@hono/zod-validator';
import { z } from 'zod';
import { getAuthContext, verifyWalletSignature, verifyWalletSignatureStrict } from '../middleware/auth.js';
import { requireStakeEligible } from '../middleware/beta-eligibility.js';
import { requireFeatureEnabled } from '../middleware/feature-gates.js';
import { getIdempotencyKey, requireIdempotencyKey } from '../middleware/idempotency.js';
import { agentService } from '../services/agents.js';
import { receiptService } from '../services/receipts.js';
import { executionService } from '../services/execution.js';
import { walletActionRateLimit } from '../services/action-rate-limit.js';
import { tradingLedgerService } from '../services/trading-ledger.js';
import { pnpExecutionService } from '../services/pnp-execution.js';
import { pnpMarketService } from '../services/pnp-market.js';
import { limitlessExecutionService } from '../services/limitless-execution.js';
import { limitlessMarketService } from '../services/limitless-market.js';
import { isLimitlessLocalOrderId } from '../services/limitless-ids.js';
import { parseMarketRef, toNamespacedMarketId } from '../services/core-ids.js';
import { coreProjectionService } from '../services/core-projections.js';
import {
  isSyntheticLedgerWriteEnabled,
  syntheticLedgerWriteBlockReason,
} from '../services/core-write-gate.js';
import {
  isJupiterPredictionUpstreamError,
  jupiterPredictionService,
} from '../services/jupiter-prediction.js';
import { requireProviderAllowed } from '../middleware/region-routing.js';

const PlaceOrderSchema = z.object({
  marketId: z.string().min(1),
  side: z.enum(['buy', 'sell']),
  outcome: z.enum(['yes', 'no']),
  orderType: z.enum(['limit', 'market']),
  price: z.number().min(0.01).max(0.99).optional(),
  quantity: z.number().positive(),
  expiresIn: z.number().int().positive().optional(),
  isPrivate: z.boolean().optional(),
  txSignature: z.string().trim().min(32).max(128).optional(),
  provider: z.enum(['pnp', 'jupiter_prediction', 'ledger', 'limitless', 'core']).optional(),
  source: z.enum(['core', 'ledger', 'pnp', 'jupiter_prediction', 'limitless']).optional(),
  chain: z.enum(['solana', 'base']).optional(),
  executionMode: z.enum(['user_signed', 'server_custody']).optional(),
  preparedOrderId: z.string().min(1).max(128).optional(),
  signature: z.string().min(64).max(256).optional(),
  walletAddress: z.string().regex(/^0x[a-fA-F0-9]{40}$/, 'Invalid Base wallet address').optional(),
});
const AgentPlaceOrderSchema = PlaceOrderSchema.extend({
  agentId: z.string().min(1).max(64),
});

export const ordersRouter = new Hono();
const requireSignedTxInProduction = process.env.NODE_ENV === 'production';

function getErrorMessage(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message.trim().length > 0) return error.message;
  return fallback;
}

function parseJupiterError(error: unknown): {
  status: 400 | 404 | 502;
  payload: Record<string, unknown>;
} {
  if (isJupiterPredictionUpstreamError(error)) {
    if (error.status === 404) {
      return { status: 404, payload: { error: 'Order not found' } };
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
    status: 400,
    payload: { error: getErrorMessage(error, 'Order request failed') },
  };
}

function parsePnpError(error: unknown): {
  status: 400 | 404 | 502 | 503;
  payload: Record<string, unknown>;
} {
  const message = getErrorMessage(error, 'PNP order request failed');
  const lowered = message.toLowerCase();
  if (lowered.includes('not found')) {
    return { status: 404, payload: { error: message, source: 'pnp' } };
  }
  if (lowered.includes('disabled')) {
    return { status: 503, payload: { error: message, source: 'pnp' } };
  }
  if (lowered.includes('invalid') || lowered.includes('required')) {
    return { status: 400, payload: { error: message, source: 'pnp' } };
  }
  return { status: 502, payload: { error: message, source: 'pnp' } };
}

function parseLimitlessError(error: unknown): {
  status: 400 | 404 | 502 | 503;
  payload: Record<string, unknown>;
} {
  const message = getErrorMessage(error, 'Limitless order request failed');
  const lowered = message.toLowerCase();
  if (lowered.includes('not found')) {
    return { status: 404, payload: { error: message, source: 'limitless' } };
  }
  if (lowered.includes('disabled') || lowered.includes('unavailable')) {
    return { status: 503, payload: { error: message, source: 'limitless' } };
  }
  if (
    lowered.includes('invalid') ||
    lowered.includes('required') ||
    lowered.includes('expired') ||
    lowered.includes('mismatch') ||
    lowered.includes('already') ||
    lowered.includes('bound')
  ) {
    return { status: 400, payload: { error: message, source: 'limitless' } };
  }
  return { status: 502, payload: { error: message, source: 'limitless' } };
}

function parseOrderSortTimestamp(order: { updatedAt: string; createdAt: string }): number {
  const updatedAt = Date.parse(order.updatedAt);
  if (Number.isFinite(updatedAt)) return updatedAt;
  return Date.parse(order.createdAt);
}

function parseOrderStatus(status: string | undefined):
  | 'open'
  | 'partially_filled'
  | 'filled'
  | 'cancelled'
  | 'expired'
  | undefined {
  if (
    status === 'open' ||
    status === 'partially_filled' ||
    status === 'filled' ||
    status === 'cancelled' ||
    status === 'expired'
  ) {
    return status;
  }
  return undefined;
}

async function resolveCoreRoutingTarget(input: {
  marketId: string;
  chain?: 'solana' | 'base';
  source?: 'core' | 'ledger' | 'pnp' | 'jupiter_prediction' | 'limitless';
  provider?: 'pnp' | 'jupiter_prediction' | 'ledger' | 'limitless' | 'core';
}): Promise<{
  isCore: boolean;
  chain: 'solana' | 'base' | null;
  marketId: string;
  responseMarketId: string;
}> {
  const parsed = parseMarketRef(input.marketId);
  const source = input.source || (input.provider === 'core' || input.provider === 'ledger' ? 'core' : undefined);
  const explicitChain = input.chain;
  const provider = input.provider;

  if (parsed.chain === 'base' || explicitChain === 'base') {
    return {
      isCore: true,
      chain: 'base',
      marketId: parsed.coreRef || input.marketId,
      responseMarketId: toNamespacedMarketId('base', parsed.coreRef || input.marketId),
    };
  }

  if (
    parsed.chain === 'solana' ||
    source === 'core' ||
    source === 'ledger' ||
    provider === 'core' ||
    provider === 'ledger' ||
    explicitChain === 'solana'
  ) {
    const rawRef = parsed.coreRef || input.marketId;
    let legacyMarketId = rawRef;
    if (!rawRef.startsWith('mkt-')) {
      const mapped = await coreProjectionService.resolveLegacyMarket(rawRef);
      if (mapped) legacyMarketId = mapped;
    }
    return {
      isCore: true,
      chain: 'solana',
      marketId: legacyMarketId,
      responseMarketId: toNamespacedMarketId('solana', rawRef),
    };
  }

  return {
    isCore: false,
    chain: null,
    marketId: input.marketId,
    responseMarketId: input.marketId,
  };
}

ordersRouter.get('/', verifyWalletSignature, async (c) => {
  const walletAddress = getAuthContext(c)?.walletAddress?.trim();
  if (!walletAddress) return c.json({ error: 'Missing signed wallet auth' }, 401);

  const marketId = c.req.query('marketId');
  const status = c.req.query('status');
  const limitRaw = c.req.query('limit');
  const offsetRaw = c.req.query('offset');
  const parsedLimit = limitRaw ? Number.parseInt(limitRaw, 10) : 50;
  const parsedOffset = offsetRaw ? Number.parseInt(offsetRaw, 10) : 0;
  const limit = Number.isFinite(parsedLimit) ? Math.max(1, Math.min(100, parsedLimit)) : 50;
  const offset = Number.isFinite(parsedOffset) ? Math.max(0, parsedOffset) : 0;
  const normalizedStatus = parseOrderStatus(status || undefined);

  const ledgerResult = await tradingLedgerService.listOrders(walletAddress, {
    marketId: marketId || undefined,
    status: normalizedStatus,
    limit: 200,
    offset: 0,
  });
  let pnpOrders: Awaited<ReturnType<typeof pnpExecutionService.listOrders>> = [];
  let pnpWarning: string | null = null;
  if (pnpExecutionService.isEnabled()) {
    try {
      pnpOrders = await pnpExecutionService.listOrders(walletAddress);
    } catch (error) {
      pnpWarning = getErrorMessage(error, 'PNP order history unavailable');
    }
  }
  let jupiterOrders: Awaited<ReturnType<typeof jupiterPredictionService.listOrders>> = [];
  let jupiterWarning: string | null = null;
  if (jupiterPredictionService.isEnabled()) {
    try {
      jupiterOrders = await jupiterPredictionService.listOrders(walletAddress);
    } catch (error) {
      jupiterWarning = getErrorMessage(error, 'Jupiter Prediction upstream unavailable');
    }
  }
  let limitlessOrders: Awaited<ReturnType<typeof limitlessExecutionService.listOrders>> = [];
  let limitlessWarning: string | null = null;
  if (limitlessExecutionService.isEnabled()) {
    try {
      limitlessOrders = await limitlessExecutionService.listOrders(walletAddress);
    } catch (error) {
      limitlessWarning = getErrorMessage(error, 'Limitless order history unavailable');
    }
  }

  let merged = [...ledgerResult.data, ...pnpOrders, ...jupiterOrders, ...limitlessOrders];
  if (marketId) {
    merged = merged.filter((order) => order.marketId === marketId);
  }
  if (normalizedStatus) {
    merged = merged.filter((order) => order.status === normalizedStatus);
  }

  merged.sort((left, right) => parseOrderSortTimestamp(right) - parseOrderSortTimestamp(left));
  const data = merged.slice(offset, offset + limit);
  return c.json({
    data,
    total: merged.length,
    limit,
    offset,
    hasMore: offset + data.length < merged.length,
    ...((jupiterWarning || pnpWarning || limitlessWarning)
      ? {
          warnings: [
            ...(pnpWarning
              ? [
                  {
                    source: 'pnp',
                    message: pnpWarning,
                  },
                ]
              : []),
            ...(jupiterWarning
              ? [
                  {
                    source: 'jupiter_prediction',
                    message: jupiterWarning,
                  },
                ]
              : []),
            ...(limitlessWarning
              ? [
                  {
                    source: 'limitless',
                    message: limitlessWarning,
                  },
                ]
              : []),
          ],
        }
      : {}),
  });
});

ordersRouter.get('/:id', verifyWalletSignature, async (c) => {
  const walletAddress = getAuthContext(c)?.walletAddress?.trim();
  if (!walletAddress) return c.json({ error: 'Missing signed wallet auth' }, 401);

  const id = c.req.param('id');
  if (isLimitlessLocalOrderId(id)) {
    try {
      const order = await limitlessExecutionService.getOrder(id, walletAddress);
      if (!order) return c.json({ error: 'Order not found' }, 404);
      return c.json(order);
    } catch (error) {
      const parsed = parseLimitlessError(error);
      return c.json(parsed.payload, parsed.status);
    }
  }
  if (id.startsWith('pnpord_')) {
    try {
      const order = await pnpExecutionService.getOrder(id, walletAddress);
      if (!order) return c.json({ error: 'Order not found' }, 404);
      return c.json(order);
    } catch (error) {
      const parsed = parsePnpError(error);
      return c.json(parsed.payload, parsed.status);
    }
  }
  if (jupiterPredictionService.isLocalOrderId(id)) {
    try {
      const order = await jupiterPredictionService.getOrder(id);
      if (!order || order.owner !== walletAddress) return c.json({ error: 'Order not found' }, 404);
      return c.json(order);
    } catch (error) {
      const parsed = parseJupiterError(error);
      return c.json(parsed.payload, parsed.status);
    }
  }

  const order = await tradingLedgerService.getOrder(walletAddress, id);
  if (!order) return c.json({ error: 'Order not found' }, 404);
  return c.json(order);
});

ordersRouter.post(
  '/',
  verifyWalletSignatureStrict,
  requireFeatureEnabled('ORDERS_WRITE_ENABLED', { feature: 'orders.write' }),
  requireStakeEligible('orders.place'),
  requireIdempotencyKey,
  walletActionRateLimit('orders.place'),
  zValidator('json', PlaceOrderSchema),
  async (c) => {
    const walletAddress = getAuthContext(c)?.walletAddress?.trim();
    if (!walletAddress) return c.json({ error: 'Missing signed wallet auth' }, 401);
    const idempotencyKey = getIdempotencyKey(c);
    if (!idempotencyKey) return c.json({ error: 'Missing idempotency key' }, 400);

    const existing = await executionService.getExecutionByIdempotencyKey(idempotencyKey);
    if (existing?.response) {
      return c.json(existing.response, existing.status === 'confirmed' ? 201 : 409);
    }

    const payload = c.req.valid('json');
    const execution = await executionService.begin({
      idempotencyKey,
      walletAddress,
      action: 'orders.place',
      resourceId: payload.marketId,
      request: payload as unknown as Record<string, unknown>,
    });

    try {
      const preferredProvider = payload.provider || payload.source || 'ledger';
      const coreTarget = await resolveCoreRoutingTarget({
        marketId: payload.marketId,
        source: payload.source,
        provider: payload.provider,
        chain: payload.chain,
      });
      const isLimitlessMarket = limitlessMarketService.isLocalMarketId(payload.marketId);
      const isPnpMarket = pnpMarketService.isLocalMarketId(payload.marketId);
      const isJupiterMarket = jupiterPredictionService.isLocalMarketId(payload.marketId);
      const shouldRouteLimitless = preferredProvider === 'limitless' || isLimitlessMarket;
      const shouldRoutePnp = preferredProvider === 'pnp' || isPnpMarket;
      const shouldRouteJupiter = preferredProvider === 'jupiter_prediction' || isJupiterMarket;
      const shouldRouteCoreSolana =
        coreTarget.isCore &&
        coreTarget.chain === 'solana' &&
        !shouldRouteLimitless &&
        !shouldRoutePnp &&
        !shouldRouteJupiter;

      if (coreTarget.isCore && coreTarget.chain === 'base' && !shouldRouteLimitless) {
        throw new Error('Base core order execution adapter is not configured');
      }

      if (shouldRouteLimitless) {
        if (!isLimitlessMarket) {
          throw new Error('Limitless order requires a Limitless market id');
        }
        const blocked = requireProviderAllowed(c, 'limitless', 'trade_open');
        if (blocked) {
          await executionService.complete({
            executionId: execution.id,
            status: 'failed',
            error: 'Limitless trade_open blocked by region policy',
          });
          return blocked;
        }

        const preparedOrderId = payload.preparedOrderId?.trim();
        const signature = payload.signature?.trim();
        const evmWalletAddress = payload.walletAddress?.trim();
        if (!preparedOrderId || !signature || !evmWalletAddress) {
          throw new Error(
            'Limitless order requires preparedOrderId, signature, and walletAddress (use /api/limitless/orders/prepare first)'
          );
        }

        const created = await limitlessExecutionService.submitPreparedOrder({
          ownerWallet: walletAddress,
          preparedOrderId,
          signature,
          walletAddress: evmWalletAddress,
        });
        const response = {
          orderId: created.orderId,
          status: created.status,
          provider: 'limitless' as const,
          marketId: created.marketId,
          externalOrderId: created.externalOrderId,
          upstreamMarketId: created.upstreamMarketId,
          executionMode: created.executionMode,
          executionId: execution.id,
          idempotencyKey,
        };
        await executionService.complete({
          executionId: execution.id,
          status: 'confirmed',
          response: response as unknown as Record<string, unknown>,
        });
        return c.json(response, 201);
      }

      if (shouldRoutePnp) {
        if (!isPnpMarket) {
          throw new Error('PNP order requires a PNP market id');
        }
        const executionMode = payload.executionMode || 'user_signed';
        const created = await pnpExecutionService.placeOrder({
          walletAddress,
          marketId: payload.marketId,
          side: payload.side,
          outcome: payload.outcome,
          quantity: payload.quantity,
          price: payload.price,
          executionMode,
        });
        const response = {
          orderId: created.id,
          status: created.status,
          provider: 'pnp' as const,
          marketId: created.marketId,
          txSignature: created.txSignature,
          executionMode: created.executionMode,
          marketModel: created.marketModel,
          upstreamMarketId: created.upstreamMarketId,
          executionId: execution.id,
          idempotencyKey,
        };
        await executionService.complete({
          executionId: execution.id,
          status: 'confirmed',
          txSignature: created.txSignature,
          response: response as unknown as Record<string, unknown>,
        });
        return c.json(response, 201);
      }

      if (!shouldRouteJupiter && !shouldRouteLimitless && requireSignedTxInProduction && !payload.txSignature) {
        await executionService.complete({
          executionId: execution.id,
          status: 'failed',
          error: 'txSignature is required in production',
        });
        return c.json({ error: 'txSignature is required in production' }, 400);
      }

      if (shouldRouteJupiter) {
        const blocked = requireProviderAllowed(c, 'jupiter_prediction', 'trade_open');
        if (blocked) {
          await executionService.complete({
            executionId: execution.id,
            status: 'failed',
            error: 'Jupiter Prediction trade_open blocked by region policy',
          });
          return blocked;
        }

        const upstreamMarketId = jupiterPredictionService.toUpstreamMarketId(payload.marketId);
        if (!upstreamMarketId) {
          throw new Error('Invalid Jupiter market id');
        }

        const isYes = payload.outcome === 'yes';
        const isBuy = payload.side === 'buy';
        let positionPubkey: string | undefined;
        if (!isBuy) {
          const positions = await jupiterPredictionService.listPositions({
            ownerPubkey: walletAddress,
            upstreamMarketId,
            isYes,
          });
          positionPubkey = positions[0]?.positionPubkey;
          if (!positionPubkey) {
            throw new Error('No matching Jupiter position found for sell order');
          }
        }

        const referencePrice = payload.price ?? 0.5;
        const depositAmountMicro = Math.max(1, Math.round(payload.quantity * referencePrice * 1_000_000));
        const contracts = Math.max(1, Math.round(payload.quantity));

        const created = await jupiterPredictionService.createOrder({
          ownerPubkey: walletAddress,
          marketId: upstreamMarketId,
          isYes,
          isBuy,
          contracts: isBuy ? undefined : String(contracts),
          depositAmount: isBuy ? String(depositAmountMicro) : undefined,
          positionPubkey,
        });

        const upstreamOrderPubkey = created.response.order?.orderPubkey || null;
        const response = {
          orderId: upstreamOrderPubkey
            ? jupiterPredictionService.toLocalOrderId(upstreamOrderPubkey)
            : `jup_pending_${Date.now().toString(36)}`,
          status: 'pending_signature' as const,
          provider: 'jupiter_prediction' as const,
          marketId: created.localMarketId,
          upstreamMarketId: created.upstreamMarketId,
          transaction: created.response.transaction ?? undefined,
          txMeta: created.response.txMeta ?? undefined,
          order: created.response.order ?? undefined,
          externalOrderId: created.response.externalOrderId ?? undefined,
          executionId: execution.id,
          idempotencyKey,
        };
        await executionService.complete({
          executionId: execution.id,
          status: 'confirmed',
          response: response as unknown as Record<string, unknown>,
        });
        return c.json(response, 201);
      }

      if (!shouldRouteCoreSolana) {
        throw new Error('Unsupported provider/source for order placement');
      }
      if (!isSyntheticLedgerWriteEnabled()) {
        throw new Error(syntheticLedgerWriteBlockReason('orders.place'));
      }

      const ledgerPayload = {
        ...payload,
        marketId: coreTarget.marketId,
      };
      const result = await tradingLedgerService.placeOrder(walletAddress, ledgerPayload, payload.txSignature);
      const response = {
        ...result,
        source: 'core' as const,
        chain: 'solana' as const,
        provider: 'core_solana' as const,
        marketId: coreTarget.responseMarketId,
        executionId: execution.id,
        idempotencyKey,
        status: result.status as 'open',
      };
      await executionService.complete({
        executionId: execution.id,
        status: 'confirmed',
        txSignature: result.txSignature,
        response: response as unknown as Record<string, unknown>,
      });
      return c.json(response, 201);
    } catch (err) {
      const message = getErrorMessage(err, 'Failed to place order');
      await executionService.complete({
        executionId: execution.id,
        status: 'failed',
        error: message,
      });
      if (message.toLowerCase().includes('synthetic ledger writes disabled')) {
        return c.json({ error: message, source: 'core', chain: 'solana' }, 503);
      }
      if (message.toLowerCase().includes('base core order execution adapter')) {
        return c.json({ error: message, source: 'core', chain: 'base' }, 503);
      }
      if (message.toLowerCase().includes('unsupported provider/source')) {
        return c.json({ error: message }, 400);
      }
      if (
        payload.provider === 'limitless' ||
        limitlessMarketService.isLocalMarketId(payload.marketId) ||
        message.toLowerCase().includes('limitless')
      ) {
        const parsed = parseLimitlessError(err);
        return c.json(parsed.payload, parsed.status);
      }
      if (
        payload.provider === 'pnp' ||
        pnpMarketService.isLocalMarketId(payload.marketId) ||
        message.toLowerCase().includes('pnp')
      ) {
        const parsed = parsePnpError(err);
        return c.json(parsed.payload, parsed.status);
      }
      const parsed = parseJupiterError(err);
      return c.json(parsed.payload, parsed.status);
    }
  }
);

ordersRouter.post(
  '/agent',
  verifyWalletSignatureStrict,
  requireFeatureEnabled('ORDERS_WRITE_ENABLED', { feature: 'orders.write' }),
  requireStakeEligible('orders.place_agent'),
  requireIdempotencyKey,
  walletActionRateLimit('orders.place_agent'),
  zValidator('json', AgentPlaceOrderSchema),
  async (c) => {
    const walletAddress = getAuthContext(c)?.walletAddress?.trim();
    if (!walletAddress) return c.json({ error: 'Missing signed wallet auth' }, 401);
    const idempotencyKey = getIdempotencyKey(c);
    if (!idempotencyKey) return c.json({ error: 'Missing idempotency key' }, 400);

    const existing = await executionService.getExecutionByIdempotencyKey(idempotencyKey);
    if (existing?.response) {
      return c.json(existing.response, existing.status === 'confirmed' ? 201 : 409);
    }

    const payload = c.req.valid('json');
    const agent = agentService.getById(payload.agentId);
    if (!agent) return c.json({ error: 'Agent not found' }, 404);
    if (!agent.isActive) return c.json({ error: 'Agent is inactive' }, 403);
    if (agent.walletAddress !== walletAddress) {
      return c.json({ error: 'Agent wallet must match signed wallet' }, 403);
    }

    const maxOrderQuantity = agent.policy?.maxOrderQuantity ?? 100_000;
    if (payload.quantity > maxOrderQuantity) {
      return c.json({ error: 'Order quantity exceeds agent policy limit' }, 403);
    }

    const effectivePrice = payload.price ?? 0.5;
    const orderNotional = Math.round(payload.quantity * effectivePrice * 1_000_000);
    const maxOrderNotional = agent.policy?.maxOrderNotional ?? 250_000_000;
    if (orderNotional > maxOrderNotional) {
      return c.json({ error: 'Order notional exceeds agent policy limit' }, 403);
    }

    const execution = await executionService.begin({
      idempotencyKey,
      walletAddress,
      action: 'orders.place_agent',
      resourceId: payload.marketId,
      request: payload as unknown as Record<string, unknown>,
    });

    try {
      const preferredProvider = payload.provider || payload.source || 'ledger';
      const coreTarget = await resolveCoreRoutingTarget({
        marketId: payload.marketId,
        source: payload.source,
        provider: payload.provider,
        chain: payload.chain,
      });
      const isLimitlessMarket = limitlessMarketService.isLocalMarketId(payload.marketId);
      const isPnpMarket = pnpMarketService.isLocalMarketId(payload.marketId);
      const isJupiterMarket = jupiterPredictionService.isLocalMarketId(payload.marketId);
      const shouldRouteLimitless = preferredProvider === 'limitless' || isLimitlessMarket;
      const shouldRoutePnp = preferredProvider === 'pnp' || isPnpMarket;
      const shouldRouteJupiter = preferredProvider === 'jupiter_prediction' || isJupiterMarket;
      const shouldRouteCoreSolana =
        coreTarget.isCore &&
        coreTarget.chain === 'solana' &&
        !shouldRouteLimitless &&
        !shouldRoutePnp &&
        !shouldRouteJupiter;
      if (coreTarget.isCore && coreTarget.chain === 'base' && !shouldRouteLimitless) {
        throw new Error('Base core order execution adapter is not configured');
      }
      if (
        shouldRouteCoreSolana &&
        requireSignedTxInProduction &&
        !payload.txSignature
      ) {
        await executionService.complete({
          executionId: execution.id,
          status: 'failed',
          error: 'txSignature is required in production',
        });
        return c.json({ error: 'txSignature is required in production' }, 400);
      }

      if (shouldRouteLimitless) {
        if (!isLimitlessMarket) {
          throw new Error('Limitless order requires a Limitless market id');
        }
        const blocked = requireProviderAllowed(c, 'limitless', 'trade_open');
        if (blocked) {
          await executionService.complete({
            executionId: execution.id,
            status: 'failed',
            error: 'Limitless trade_open blocked by region policy',
          });
          return blocked;
        }
        if ((payload.executionMode || 'server_custody') !== 'server_custody') {
          throw new Error('Agent Limitless orders require executionMode=server_custody');
        }

        const created = await limitlessExecutionService.placeAgentOrder({
          ownerWallet: walletAddress,
          agentId: payload.agentId,
          marketId: payload.marketId,
          side: payload.side,
          outcome: payload.outcome,
          orderType: payload.orderType,
          quantity: payload.quantity,
          price: payload.price,
        });

        const receipt = await receiptService.create({
          agent,
          kind: 'trade_executed',
          summary: `agent placed ${payload.side} Limitless order on ${payload.marketId}`,
          payload: {
            marketId: payload.marketId,
            side: payload.side,
            outcome: payload.outcome,
            quantity: payload.quantity,
            price: payload.price ?? null,
            orderType: payload.orderType,
            orderNotional,
            orderId: created.orderId,
            provider: 'limitless',
            executionMode: created.executionMode,
            upstreamMarketId: created.upstreamMarketId,
            externalOrderId: created.externalOrderId,
            executionId: execution.id,
          },
          idempotencyKey,
        });

        const response = {
          orderId: created.orderId,
          status: created.status,
          provider: 'limitless' as const,
          marketId: created.marketId,
          upstreamMarketId: created.upstreamMarketId,
          externalOrderId: created.externalOrderId,
          executionMode: created.executionMode,
          receipt,
          executionId: execution.id,
          idempotencyKey,
        };
        await executionService.complete({
          executionId: execution.id,
          status: 'confirmed',
          response: response as unknown as Record<string, unknown>,
        });
        return c.json(response, 201);
      }

      if (shouldRoutePnp) {
        if (!isPnpMarket) {
          throw new Error('PNP order requires a PNP market id');
        }
        const created = await pnpExecutionService.placeOrder({
          walletAddress,
          marketId: payload.marketId,
          side: payload.side,
          outcome: payload.outcome,
          quantity: payload.quantity,
          price: payload.price,
          executionMode: payload.executionMode || 'user_signed',
        });
        const receipt = await receiptService.create({
          agent,
          kind: 'trade_executed',
          summary: `agent placed ${payload.side} PNP order on ${payload.marketId}`,
          payload: {
            marketId: payload.marketId,
            side: payload.side,
            outcome: payload.outcome,
            quantity: payload.quantity,
            price: payload.price ?? null,
            orderType: payload.orderType,
            orderNotional,
            orderId: created.id,
            provider: 'pnp',
            executionMode: created.executionMode,
            marketModel: created.marketModel,
            upstreamMarketId: created.upstreamMarketId,
            executionId: execution.id,
            txSignature: created.txSignature ?? null,
          },
          idempotencyKey,
        });

        const response = {
          orderId: created.id,
          status: created.status,
          provider: 'pnp' as const,
          marketId: created.marketId,
          txSignature: created.txSignature,
          executionMode: created.executionMode,
          marketModel: created.marketModel,
          upstreamMarketId: created.upstreamMarketId,
          receipt,
          executionId: execution.id,
          idempotencyKey,
        };
        await executionService.complete({
          executionId: execution.id,
          status: 'confirmed',
          txSignature: created.txSignature,
          response: response as unknown as Record<string, unknown>,
        });
        return c.json(response, 201);
      }

      if (shouldRouteJupiter) {
        const blocked = requireProviderAllowed(c, 'jupiter_prediction', 'trade_open');
        if (blocked) {
          await executionService.complete({
            executionId: execution.id,
            status: 'failed',
            error: 'Jupiter Prediction trade_open blocked by region policy',
          });
          return blocked;
        }

        const upstreamMarketId = jupiterPredictionService.toUpstreamMarketId(payload.marketId);
        if (!upstreamMarketId) {
          throw new Error('Invalid Jupiter market id');
        }

        const isYes = payload.outcome === 'yes';
        const isBuy = payload.side === 'buy';
        let positionPubkey: string | undefined;
        if (!isBuy) {
          const positions = await jupiterPredictionService.listPositions({
            ownerPubkey: walletAddress,
            upstreamMarketId,
            isYes,
          });
          positionPubkey = positions[0]?.positionPubkey;
          if (!positionPubkey) {
            throw new Error('No matching Jupiter position found for sell order');
          }
        }

        const depositAmountMicro = Math.max(1, Math.round(payload.quantity * effectivePrice * 1_000_000));
        const contracts = Math.max(1, Math.round(payload.quantity));
        const created = await jupiterPredictionService.createOrder({
          ownerPubkey: walletAddress,
          marketId: upstreamMarketId,
          isYes,
          isBuy,
          contracts: isBuy ? undefined : String(contracts),
          depositAmount: isBuy ? String(depositAmountMicro) : undefined,
          positionPubkey,
        });

        const upstreamOrderPubkey = created.response.order?.orderPubkey || null;
        const receipt = await receiptService.create({
          agent,
          kind: 'trade_executed',
          summary: `agent placed ${payload.side} Jupiter order on ${payload.marketId}`,
          payload: {
            marketId: payload.marketId,
            side: payload.side,
            outcome: payload.outcome,
            quantity: payload.quantity,
            price: payload.price ?? null,
            orderType: payload.orderType,
            orderNotional,
            orderId: upstreamOrderPubkey,
            provider: 'jupiter_prediction',
            transaction: created.response.transaction ?? null,
            txMeta: created.response.txMeta ?? null,
            executionId: execution.id,
            txSignature: null,
          },
          idempotencyKey,
        });

        const response = {
          orderId: upstreamOrderPubkey
            ? jupiterPredictionService.toLocalOrderId(upstreamOrderPubkey)
            : `jup_pending_${Date.now().toString(36)}`,
          status: 'pending_signature' as const,
          provider: 'jupiter_prediction' as const,
          marketId: created.localMarketId,
          upstreamMarketId: created.upstreamMarketId,
          transaction: created.response.transaction ?? undefined,
          txMeta: created.response.txMeta ?? undefined,
          order: created.response.order ?? undefined,
          externalOrderId: created.response.externalOrderId ?? undefined,
          receipt,
          executionId: execution.id,
          idempotencyKey,
        };
        await executionService.complete({
          executionId: execution.id,
          status: 'confirmed',
          response: response as unknown as Record<string, unknown>,
        });
        return c.json(response, 201);
      }

      if (!shouldRouteCoreSolana) {
        throw new Error('Unsupported provider/source for order placement');
      }
      if (!isSyntheticLedgerWriteEnabled()) {
        throw new Error(syntheticLedgerWriteBlockReason('orders.place_agent'));
      }

      const ledgerPayload = {
        ...payload,
        marketId: coreTarget.marketId,
      };
      const result = await tradingLedgerService.placeOrder(walletAddress, ledgerPayload, payload.txSignature);
      const receipt = await receiptService.create({
        agent,
        kind: 'trade_executed',
        summary: `agent placed ${payload.side} order on ${coreTarget.responseMarketId}`,
        payload: {
          marketId: coreTarget.responseMarketId,
          side: payload.side,
          outcome: payload.outcome,
          quantity: payload.quantity,
          price: payload.price ?? null,
          orderType: payload.orderType,
          orderNotional,
          orderId: result.orderId,
          executionId: execution.id,
          txSignature: result.txSignature ?? null,
        },
        idempotencyKey,
      });

      const response = {
        ...result,
        source: 'core' as const,
        chain: 'solana' as const,
        provider: 'core_solana' as const,
        marketId: coreTarget.responseMarketId,
        receipt,
        executionId: execution.id,
        idempotencyKey,
      };
      await executionService.complete({
        executionId: execution.id,
        status: 'confirmed',
        txSignature: result.txSignature,
        response: response as unknown as Record<string, unknown>,
      });
      return c.json(response, 201);
    } catch (err) {
      const message = getErrorMessage(err, 'Failed to place order');
      await executionService.complete({
        executionId: execution.id,
        status: 'failed',
        error: message,
      });
      if (message.toLowerCase().includes('synthetic ledger writes disabled')) {
        return c.json({ error: message, source: 'core', chain: 'solana' }, 503);
      }
      if (message.toLowerCase().includes('base core order execution adapter')) {
        return c.json({ error: message, source: 'core', chain: 'base' }, 503);
      }
      if (message.toLowerCase().includes('unsupported provider/source')) {
        return c.json({ error: message }, 400);
      }
      if (
        payload.provider === 'limitless' ||
        limitlessMarketService.isLocalMarketId(payload.marketId) ||
        message.toLowerCase().includes('limitless')
      ) {
        const parsed = parseLimitlessError(err);
        return c.json(parsed.payload, parsed.status);
      }
      if (
        payload.provider === 'pnp' ||
        pnpMarketService.isLocalMarketId(payload.marketId) ||
        message.toLowerCase().includes('pnp')
      ) {
        const parsed = parsePnpError(err);
        return c.json(parsed.payload, parsed.status);
      }
      const parsed = parseJupiterError(err);
      return c.json(parsed.payload, parsed.status);
    }
  }
);

ordersRouter.delete(
  '/:id',
  verifyWalletSignatureStrict,
  requireFeatureEnabled('ORDERS_WRITE_ENABLED', { feature: 'orders.write' }),
  requireStakeEligible('orders.cancel'),
  requireIdempotencyKey,
  walletActionRateLimit('orders.cancel'),
  async (c) => {
    const walletAddress = getAuthContext(c)?.walletAddress?.trim();
    if (!walletAddress) return c.json({ error: 'Missing signed wallet auth' }, 401);
    const idempotencyKey = getIdempotencyKey(c);
    if (!idempotencyKey) return c.json({ error: 'Missing idempotency key' }, 400);

    const existing = await executionService.getExecutionByIdempotencyKey(idempotencyKey);
    if (existing?.response) {
      return c.json(existing.response, existing.status === 'confirmed' ? 200 : 409);
    }

    const orderId = c.req.param('id');
    const txSignatureHeader = c.req.header('x-tx-signature')?.trim();
    const execution = await executionService.begin({
      idempotencyKey,
      walletAddress,
      action: 'orders.cancel',
      resourceId: orderId,
    });

    if (orderId.startsWith('pnpord_')) {
      await executionService.complete({
        executionId: execution.id,
        status: 'failed',
        error: 'PNP market orders are immediate fills and cannot be cancelled',
      });
      return c.json({ error: 'PNP market orders are immediate fills and cannot be cancelled' }, 400);
    }

    if (isLimitlessLocalOrderId(orderId)) {
      const blocked = requireProviderAllowed(c, 'limitless', 'trade_close');
      if (blocked) {
        await executionService.complete({
          executionId: execution.id,
          status: 'failed',
          error: 'Limitless trade_close blocked by region policy',
        });
        return blocked;
      }

      try {
        const cancelled = await limitlessExecutionService.cancelOrder(walletAddress, orderId);
        const response = {
          ...cancelled,
          executionId: execution.id,
          idempotencyKey,
        };
        await executionService.complete({
          executionId: execution.id,
          status: 'confirmed',
          response: response as unknown as Record<string, unknown>,
        });
        return c.json(response);
      } catch (err) {
        const parsed = parseLimitlessError(err);
        const message = getErrorMessage(err, 'Order cannot be cancelled');
        await executionService.complete({
          executionId: execution.id,
          status: 'failed',
          error: message,
        });
        return c.json(parsed.payload, parsed.status);
      }
    }

    if (jupiterPredictionService.isLocalOrderId(orderId)) {
      const blocked = requireProviderAllowed(c, 'jupiter_prediction', 'trade_close');
      if (blocked) {
        await executionService.complete({
          executionId: execution.id,
          status: 'failed',
          error: 'Jupiter Prediction trade_close blocked by region policy',
        });
        return blocked;
      }

      try {
        const closed = await jupiterPredictionService.closeOrder(orderId, walletAddress);
        const response = {
          success: true,
          provider: 'jupiter_prediction' as const,
          orderId,
          orderPubkey: closed.orderPubkey,
          transaction: closed.response.transaction ?? undefined,
          blockhash: closed.response.blockhash ?? undefined,
          latestBlockhash: closed.response.latestBlockhash ?? undefined,
          lastValidBlockHeight: closed.response.lastValidBlockHeight ?? undefined,
          executionId: execution.id,
          idempotencyKey,
        };
        await executionService.complete({
          executionId: execution.id,
          status: 'confirmed',
          response: response as unknown as Record<string, unknown>,
        });
        return c.json(response);
      } catch (err) {
        const parsed = parseJupiterError(err);
        const message = getErrorMessage(err, 'Order cannot be cancelled');
        await executionService.complete({
          executionId: execution.id,
          status: 'failed',
          error: message,
        });
        return c.json(parsed.payload, parsed.status);
      }
    }

    if (requireSignedTxInProduction && !txSignatureHeader) {
      await executionService.complete({
        executionId: execution.id,
        status: 'failed',
        error: 'x-tx-signature is required in production',
      });
      return c.json({ error: 'x-tx-signature is required in production' }, 400);
    }

    const result = await tradingLedgerService.cancelOrder(walletAddress, orderId, txSignatureHeader);
    if (!result.success) {
      await executionService.complete({
        executionId: execution.id,
        status: 'failed',
        error: 'Order cannot be cancelled',
      });
      return c.json({ error: 'Order cannot be cancelled' }, 400);
    }

    const response = {
      ...result,
      executionId: execution.id,
      idempotencyKey,
    };
    await executionService.complete({
      executionId: execution.id,
      status: 'confirmed',
      txSignature: result.txSignature,
      response: response as unknown as Record<string, unknown>,
    });
    return c.json(response);
  }
);
