#!/usr/bin/env node

import crypto from "node:crypto";
import pg from "pg";

import {
  closeOpsState,
  connectOpsState,
  reportRunnerFailure,
  reportRunnerStarted,
  reportRunnerSuccess,
} from "./ops-state.mjs";

const { Client } = pg;

const RUNNER_NAME = "creator_economics_materializer";

function isEnabled(raw, fallback = false) {
  if (raw == null || String(raw).trim() === "") {
    return fallback;
  }

  return ["1", "true", "yes", "on"].includes(
    String(raw).trim().toLowerCase(),
  );
}

function parseArgs(argv) {
  const options = {
    owner: null,
    marketId: null,
    limit: 200,
    apiOnly: false,
    dbOnly: false,
  };

  for (const arg of argv) {
    if (arg.startsWith("--owner=")) {
      options.owner = normalizeWallet(arg.slice("--owner=".length));
      continue;
    }
    if (arg.startsWith("--market-id=")) {
      const parsed = Number.parseInt(arg.slice("--market-id=".length), 10);
      if (!Number.isInteger(parsed) || parsed < 1) {
        throw new Error("--market-id must be a positive integer");
      }
      options.marketId = parsed;
      continue;
    }
    if (arg.startsWith("--limit=")) {
      const parsed = Number.parseInt(arg.slice("--limit=".length), 10);
      if (!Number.isInteger(parsed) || parsed < 1 || parsed > 1000) {
        throw new Error("--limit must be between 1 and 1000");
      }
      options.limit = parsed;
      continue;
    }
    if (arg === "--api-only") {
      options.apiOnly = true;
      continue;
    }
    if (arg === "--db-only") {
      options.dbOnly = true;
      continue;
    }

    throw new Error(`Unknown argument: ${arg}`);
  }

  if (options.apiOnly && options.dbOnly) {
    throw new Error("--api-only and --db-only are mutually exclusive");
  }

  return options;
}

function normalizeWallet(value) {
  const wallet = String(value || "").trim().toLowerCase();
  if (!/^0x[0-9a-f]{40}$/.test(wallet)) {
    throw new Error("owner must be a valid 0x EVM address");
  }
  return wallet;
}

function normalizeApiBase(raw) {
  const trimmed = String(raw || "").trim().replace(/\/+$/, "");
  if (!trimmed) {
    return null;
  }
  if (trimmed.endsWith("/v1")) {
    return trimmed;
  }
  if (trimmed.endsWith("/v1/creator")) {
    return trimmed.replace(/\/creator$/, "");
  }
  return `${trimmed}/v1`;
}

function buildPgConfig(connectionString) {
  const url = new URL(connectionString);
  const sslmode = (url.searchParams.get("sslmode") || "").trim().toLowerCase();
  const needsSsl =
    ["require", "verify-ca", "verify-full", "prefer"].includes(sslmode) ||
    /\.render\.com$/i.test(url.hostname);

  return needsSsl
    ? {
        connectionString,
        connectionTimeoutMillis: 10_000,
        keepAlive: true,
        ssl: { rejectUnauthorized: false },
      }
    : {
        connectionString,
        connectionTimeoutMillis: 10_000,
        keepAlive: true,
      };
}

async function connectDb(connectionString) {
  const client = new Client(buildPgConfig(connectionString));
  await client.connect();
  return client;
}

async function closeDb(client) {
  if (!client) {
    return;
  }
  try {
    await client.end();
  } catch {
    // best effort
  }
}

function normalizeOutcome(value) {
  const normalized = String(value ?? "").trim().toLowerCase();
  if (normalized === "yes" || normalized === "0" || normalized === "true") {
    return "yes";
  }
  return "no";
}

function toNumber(value, fallback = 0) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function computeCapitalValue(availableUsdc, reservedUsdc, inventoryMarkValueUsdc) {
  return availableUsdc + reservedUsdc + inventoryMarkValueUsdc;
}

function computeNetLiquidityPnl(seedUsdc, capitalValueUsdc) {
  return capitalValueUsdc - seedUsdc;
}

function computeSubsidyBurn(seedUsdc, capitalValueUsdc) {
  return Math.max(seedUsdc - capitalValueUsdc, 0);
}

function computeRoiBps(seedUsdc, netLiquidityPnlUsdc) {
  if (seedUsdc <= 0) {
    return 0;
  }
  return (netLiquidityPnlUsdc * 10_000) / seedUsdc;
}

function computeInventoryValues(position, yesPrice, noPrice) {
  if (!position) {
    return {
      inventoryYesUsdc: 0,
      inventoryNoUsdc: 0,
      inventoryNetUsdc: 0,
      inventoryMarkValueUsdc: 0,
      realizedResolutionPnlUsdc: 0,
    };
  }

  const yesBalance = toNumber(position.yes_balance);
  const noBalance = toNumber(position.no_balance);
  const inventoryYesUsdc = yesBalance * yesPrice;
  const inventoryNoUsdc = noBalance * noPrice;

  return {
    inventoryYesUsdc,
    inventoryNoUsdc,
    inventoryNetUsdc: inventoryYesUsdc - inventoryNoUsdc,
    inventoryMarkValueUsdc: inventoryYesUsdc + inventoryNoUsdc,
    realizedResolutionPnlUsdc: toNumber(position.realized_pnl),
  };
}

function classifyBootstrapFill(creator, trade, agents) {
  const buyIsCreator = String(trade.buy.owner || "").toLowerCase() === creator;
  const sellIsCreator = String(trade.sell.owner || "").toLowerCase() === creator;
  if (buyIsCreator === sellIsCreator) {
    return null;
  }

  const maker = buyIsCreator ? trade.buy : trade.sell;
  const taker = buyIsCreator ? trade.sell : trade.buy;
  if (new Date(maker.createdAt).getTime() >= new Date(taker.createdAt).getTime()) {
    return null;
  }

  const matches = agents.filter((agent) => {
    if (trade.outcome !== agent.side) {
      return false;
    }
    return maker.priceBps === agent.priceBps && maker.quantity === agent.size;
  });

  if (matches.length !== 1) {
    return null;
  }

  const agent = matches[0];
  const side = buyIsCreator ? "buy" : "sell";
  const rawKey = `${creator}:${trade.marketId}:${trade.tradeId}:internal_orderbook`;

  return {
    id: crypto.createHash("sha256").update(rawKey).digest("hex").slice(0, 64),
    marketId: trade.marketId,
    creator,
    tradeId: trade.tradeId,
    source: "internal_orderbook",
    agentId: agent.agentId,
    makerOrderId: maker.orderId,
    outcome: trade.outcome,
    side,
    price: trade.price,
    quantity: trade.quantity,
    notionalUsdc: trade.price * trade.quantity,
    occurredAt: trade.occurredAt,
    raw: {
      tradeId: trade.tradeId,
      marketId: trade.marketId,
      outcome: trade.outcome,
      price: trade.price,
      quantity: trade.quantity,
      makerOrderId: maker.orderId,
      makerSide: side,
      takerOrderId: taker.orderId,
      bootstrapAgentId: agent.agentId,
      bootstrapAgentPriceBps: agent.priceBps,
      bootstrapAgentSize: agent.size,
    },
  };
}

async function fetchTargets(client, options) {
  const { rows } = await client.query(
    `
      select
        market_id,
        creator,
        liquidity_mode,
        status,
        seed_usdc,
        available_usdc,
        reserved_usdc,
        organic_depth_ratio
      from base_market_bootstrap_configs
      where liquidity_mode = 'bootstrap_hybrid'
        and seed_usdc > 0
        and ($1::text is null or lower(creator) = lower($1))
        and ($2::bigint is null or market_id = $2)
      order by market_id asc
      limit $3
    `,
    [options.owner, options.marketId, options.limit],
  );

  return rows.map((row) => ({
    marketId: toNumber(row.market_id),
    creator: normalizeWallet(row.creator),
    status: String(row.status || ""),
    liquidityMode: String(row.liquidity_mode || ""),
    seedUsdc: toNumber(row.seed_usdc),
    availableUsdc: toNumber(row.available_usdc),
    reservedUsdc: toNumber(row.reserved_usdc),
    organicDepthRatio: toNumber(row.organic_depth_ratio),
  }));
}

async function fetchAgents(client, marketId) {
  const { rows } = await client.query(
    `
      select
        agent_id,
        side,
        coalesce(current_price_bps, desired_price_bps) as price_bps,
        coalesce(current_size, desired_size) as size
      from base_market_bootstrap_agents
      where market_id = $1
        and active = true
        and agent_id is not null
      order by side asc, level_index asc
    `,
    [marketId],
  );

  return rows.map((row) => ({
    agentId: toNumber(row.agent_id),
    side: String(row.side || "").toLowerCase(),
    priceBps: toNumber(row.price_bps),
    size: toNumber(row.size),
  }));
}

async function fetchTrades(client, marketId) {
  const { rows } = await client.query(
    `
      select
        t.id as trade_id,
        cast(t.market_id as bigint) as market_id,
        t.outcome as outcome,
        t.price as price,
        t.quantity as quantity,
        t.created_at as occurred_at,
        bo.id as buy_order_id,
        bo.owner as buy_owner,
        bo.price_bps as buy_price_bps,
        bo.quantity as buy_quantity,
        bo.created_at as buy_created_at,
        so.id as sell_order_id,
        so.owner as sell_owner,
        so.price_bps as sell_price_bps,
        so.quantity as sell_quantity,
        so.created_at as sell_created_at
      from trades t
      inner join orders bo on bo.id = t.buy_order_id
      inner join orders so on so.id = t.sell_order_id
      where t.market_id = $1
      order by t.created_at asc, t.id asc
    `,
    [String(marketId)],
  );

  return rows.map((row) => ({
    tradeId: String(row.trade_id || ""),
    marketId: toNumber(row.market_id),
    outcome: normalizeOutcome(row.outcome),
    price: toNumber(row.price),
    quantity: toNumber(row.quantity),
    occurredAt: row.occurred_at,
    buy: {
      orderId: String(row.buy_order_id || ""),
      owner: String(row.buy_owner || "").toLowerCase(),
      priceBps: toNumber(row.buy_price_bps),
      quantity: toNumber(row.buy_quantity),
      createdAt: row.buy_created_at,
    },
    sell: {
      orderId: String(row.sell_order_id || ""),
      owner: String(row.sell_owner || "").toLowerCase(),
      priceBps: toNumber(row.sell_price_bps),
      quantity: toNumber(row.sell_quantity),
      createdAt: row.sell_created_at,
    },
  }));
}

async function upsertBootstrapFillEvent(client, draft) {
  const { rows } = await client.query(
    `
      insert into bootstrap_fill_events (
        id, market_id, creator, trade_id, source, agent_id, maker_order_id,
        outcome, side, price, quantity, notional_usdc, occurred_at, raw
      )
      values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
      on conflict (trade_id, source) do update set
        market_id = excluded.market_id,
        creator = excluded.creator,
        agent_id = excluded.agent_id,
        maker_order_id = excluded.maker_order_id,
        outcome = excluded.outcome,
        side = excluded.side,
        price = excluded.price,
        quantity = excluded.quantity,
        notional_usdc = excluded.notional_usdc,
        occurred_at = excluded.occurred_at,
        raw = excluded.raw,
        updated_at = now()
      returning (xmax = 0) as inserted
    `,
    [
      draft.id,
      draft.marketId,
      draft.creator,
      draft.tradeId,
      draft.source,
      draft.agentId,
      draft.makerOrderId,
      draft.outcome,
      draft.side,
      draft.price,
      draft.quantity,
      draft.notionalUsdc,
      draft.occurredAt,
      JSON.stringify(draft.raw),
    ],
  );

  return rows[0]?.inserted === true;
}

async function fetchFillEvents(client, creator, marketId) {
  const { rows } = await client.query(
    `
      select trade_id, occurred_at, notional_usdc
      from bootstrap_fill_events
      where lower(creator) = lower($1)
        and market_id = $2
      order by occurred_at asc, trade_id asc
    `,
    [creator, marketId],
  );

  return rows.map((row) => ({
    tradeId: String(row.trade_id || ""),
    occurredAt: new Date(row.occurred_at),
    notionalUsdc: toNumber(row.notional_usdc),
  }));
}

async function fetchMarketSnapshot(client, marketId) {
  const { rows } = await client.query(
    `
      select id, question, status, yes_price, no_price
      from markets
      where id = $1
      limit 1
    `,
    [String(marketId)],
  );

  return rows[0] || null;
}

async function fetchPosition(client, creator, marketId) {
  const { rows } = await client.query(
    `
      select yes_balance, no_balance, realized_pnl
      from positions
      where lower(owner) = lower($1)
        and market_id = $2
      limit 1
    `,
    [creator, String(marketId)],
  );

  return rows[0] || null;
}

async function fetchMirrorMetrics(client, marketId) {
  const summary = await client.query(
    `
      select
        count(*) filter (where mirror_error is not null)::bigint as mirror_error_count,
        count(*) filter (where hedge_error is not null)::bigint as hedge_error_count,
        max(last_mirror_at) as last_mirror_at
      from mirror_market_links
      where internal_market_id = $1
    `,
    [marketId],
  );
  const pending = await client.query(
    `
      select count(*)::bigint as cnt
      from mirror_hedge_log log
      inner join mirror_market_links link on link.id = log.mirror_link_id
      where link.internal_market_id = $1
        and log.hedge_status = 'pending'
    `,
    [marketId],
  );

  const row = summary.rows[0] || {};
  const lastMirrorAt = row.last_mirror_at ? new Date(row.last_mirror_at) : null;
  const freshnessSeconds = lastMirrorAt
    ? Math.max(0, Math.floor((Date.now() - lastMirrorAt.getTime()) / 1000))
    : null;

  return {
    freshnessSeconds,
    pendingHedges: toNumber(pending.rows[0]?.cnt),
    errorCount:
      toNumber(row.mirror_error_count) + toNumber(row.hedge_error_count),
  };
}

async function upsertTodayRow(client, payload) {
  const { rows } = await client.query(
    `
      insert into creator_market_economics_daily (
        market_id, creator, day, seed_usdc, available_usdc, reserved_usdc,
        inventory_yes, inventory_no, inventory_mark_value_usdc,
        cumulative_bootstrap_fills_usdc, net_liquidity_pnl_usdc, subsidy_burn_usdc,
        roi_bps, realized_resolution_pnl_usdc, organic_depth_ratio, graduated,
        graduation_retention_24h, graduation_retention_7d, mirror_freshness_seconds,
        mirror_pending_hedges, mirror_error_count
      )
      values (
        $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21
      )
      on conflict (market_id, creator, day) do update set
        seed_usdc = excluded.seed_usdc,
        available_usdc = excluded.available_usdc,
        reserved_usdc = excluded.reserved_usdc,
        inventory_yes = excluded.inventory_yes,
        inventory_no = excluded.inventory_no,
        inventory_mark_value_usdc = excluded.inventory_mark_value_usdc,
        cumulative_bootstrap_fills_usdc = excluded.cumulative_bootstrap_fills_usdc,
        net_liquidity_pnl_usdc = excluded.net_liquidity_pnl_usdc,
        subsidy_burn_usdc = excluded.subsidy_burn_usdc,
        roi_bps = excluded.roi_bps,
        realized_resolution_pnl_usdc = excluded.realized_resolution_pnl_usdc,
        organic_depth_ratio = excluded.organic_depth_ratio,
        graduated = excluded.graduated,
        graduation_retention_24h = excluded.graduation_retention_24h,
        graduation_retention_7d = excluded.graduation_retention_7d,
        mirror_freshness_seconds = excluded.mirror_freshness_seconds,
        mirror_pending_hedges = excluded.mirror_pending_hedges,
        mirror_error_count = excluded.mirror_error_count
      returning (xmax = 0) as inserted
    `,
    [
      payload.marketId,
      payload.creator,
      payload.day,
      payload.seedUsdc,
      payload.availableUsdc,
      payload.reservedUsdc,
      payload.inventoryYesUsdc,
      payload.inventoryNoUsdc,
      payload.inventoryMarkValueUsdc,
      payload.cumulativeBootstrapFillsUsdc,
      payload.netLiquidityPnlUsdc,
      payload.subsidyBurnUsdc,
      payload.roiBps,
      payload.realizedResolutionPnlUsdc,
      payload.organicDepthRatio,
      payload.graduated,
      null,
      null,
      payload.mirrorFreshnessSeconds,
      payload.mirrorPendingHedges,
      payload.mirrorErrorCount,
    ],
  );

  return rows[0]?.inserted === true;
}

async function materializeMarket(client, config) {
  const agents = await fetchAgents(client, config.marketId);
  const trades = await fetchTrades(client, config.marketId);
  let backfilledFillEvents = 0;

  for (const trade of trades) {
    const draft = classifyBootstrapFill(config.creator, trade, agents);
    if (!draft) {
      continue;
    }
    if (await upsertBootstrapFillEvent(client, draft)) {
      backfilledFillEvents += 1;
    }
  }

  const events = await fetchFillEvents(client, config.creator, config.marketId);
  const today = new Date().toISOString().slice(0, 10);
  let cumulativeBootstrapFillsUsdc = 0;
  for (const event of events) {
    if (event.occurredAt.toISOString().slice(0, 10) <= today) {
      cumulativeBootstrapFillsUsdc += event.notionalUsdc;
    }
  }

  const market = await fetchMarketSnapshot(client, config.marketId);
  if (!market) {
    throw new Error(`market ${config.marketId} not found`);
  }

  const position = await fetchPosition(client, config.creator, config.marketId);
  const mirror = await fetchMirrorMetrics(client, config.marketId);
  const yesPrice = toNumber(market.yes_price, 0.5);
  const noPrice = toNumber(market.no_price, 0.5);
  const inventory = computeInventoryValues(position, yesPrice, noPrice);
  const currentCapitalValueUsdc = computeCapitalValue(
    config.availableUsdc,
    config.reservedUsdc,
    inventory.inventoryMarkValueUsdc,
  );
  const netLiquidityPnlUsdc = computeNetLiquidityPnl(
    config.seedUsdc,
    currentCapitalValueUsdc,
  );
  const subsidyBurnUsdc = computeSubsidyBurn(
    config.seedUsdc,
    currentCapitalValueUsdc,
  );
  const roiBps = computeRoiBps(config.seedUsdc, netLiquidityPnlUsdc);
  const realizedResolutionPnlUsdc =
    toNumber(market.status) === 3 ? inventory.realizedResolutionPnlUsdc : 0;

  await upsertTodayRow(client, {
    marketId: config.marketId,
    creator: config.creator,
    day: today,
    seedUsdc: config.seedUsdc,
    availableUsdc: config.availableUsdc,
    reservedUsdc: config.reservedUsdc,
    inventoryYesUsdc: inventory.inventoryYesUsdc,
    inventoryNoUsdc: inventory.inventoryNoUsdc,
    inventoryMarkValueUsdc: inventory.inventoryMarkValueUsdc,
    cumulativeBootstrapFillsUsdc,
    netLiquidityPnlUsdc,
    subsidyBurnUsdc,
    roiBps,
    realizedResolutionPnlUsdc,
    organicDepthRatio: config.organicDepthRatio,
    graduated: String(config.status || "").toLowerCase() === "graduated",
    mirrorFreshnessSeconds: mirror.freshnessSeconds,
    mirrorPendingHedges: mirror.pendingHedges,
    mirrorErrorCount: mirror.errorCount,
  });

  return {
    marketId: config.marketId,
    creator: config.creator,
    day: today,
    backfilledFillEvents,
    materializedRows: 1,
  };
}

async function materializeViaDb(env, options) {
  const connectionString = String(env.DATABASE_URL || "").trim();
  if (!connectionString) {
    throw new Error("DATABASE_URL is required for DB materialization");
  }

  const client = await connectDb(connectionString);
  try {
    const targets = await fetchTargets(client, options);
    const markets = [];
    let backfilledFillEvents = 0;
    let materializedRows = 0;

    for (const target of targets) {
      const result = await materializeMarket(client, target);
      markets.push(result);
      backfilledFillEvents += result.backfilledFillEvents;
      materializedRows += result.materializedRows;
    }

    return {
      mode: "db",
      day: new Date().toISOString().slice(0, 10),
      processedMarkets: markets.length,
      backfilledFillEvents,
      materializedRows,
      markets,
    };
  } finally {
    await closeDb(client);
  }
}

async function tryApiMaterialize(env, options) {
  const apiBase = normalizeApiBase(
    env.CREATOR_ECONOMICS_MATERIALIZER_API_URL ||
      env.CREATOR_ECONOMICS_API_URL ||
      env.API_URL,
  );
  const adminKey = String(
    env.CREATOR_ECONOMICS_MATERIALIZER_ADMIN_KEY || env.ADMIN_CONTROL_KEY || "",
  ).trim();
  if (!apiBase || !adminKey) {
    return null;
  }

  const internalKey = (process.env.INTERNAL_SERVICE_KEY || "").trim();
  const response = await fetch(`${apiBase}/evm/creator/materializer/run`, {
    method: "POST",
    headers: {
      Accept: "application/json",
      "Content-Type": "application/json",
      "x-admin-key": adminKey,
      ...(internalKey ? { "x-internal-service-key": internalKey } : {}),
    },
    body: JSON.stringify({
      owner: options.owner,
      market_id: options.marketId,
      limit: options.limit,
      window_days: 30,
    }),
  });

  if (response.status === 404) {
    return null;
  }

  const text = await response.text();
  let payload = null;
  if (text) {
    try {
      payload = JSON.parse(text);
    } catch {
      payload = { raw: text };
    }
  }
  if (!response.ok) {
    const error = new Error(
      payload?.message || payload?.error || `${response.status} ${response.statusText}`,
    );
    error.status = response.status;
    error.payload = payload;
    throw error;
  }

  return {
    mode: "api",
    day: new Date().toISOString().slice(0, 10),
    processedMarkets: payload?.materializedMarkets ?? 0,
    backfilledFillEvents: 0,
    materializedRows: payload?.rowsLoaded ?? 0,
    failures: Array.isArray(payload?.failures) ? payload.failures : [],
    raw: payload,
  };
}

async function main() {
  if (!isEnabled(process.env.CREATOR_ECONOMICS_MATERIALIZER_ENABLED, false)) {
    console.log(
      JSON.stringify(
        {
          ok: true,
          skipped: true,
          reason: "creator economics materializer disabled",
        },
        null,
        2,
      ),
    );
    return;
  }

  const options = parseArgs(process.argv.slice(2));
  const opsState = await connectOpsState(process.env).catch(() => null);
  const startedAt = new Date().toISOString();

  try {
    await reportRunnerStarted(opsState, RUNNER_NAME, {
      startedAt,
      owner: options.owner,
      marketId: options.marketId,
      limit: options.limit,
    });

    let result = null;
    if (!options.dbOnly) {
      result = await tryApiMaterialize(process.env, options);
    }
    if (!result && !options.apiOnly) {
      result = await materializeViaDb(process.env, options);
    }
    if (!result) {
      throw new Error(
        "creator economics materializer has neither a reachable API hook nor DATABASE_URL",
      );
    }

    await reportRunnerSuccess(opsState, RUNNER_NAME, {
      startedAt,
      owner: options.owner,
      marketId: options.marketId,
      limit: options.limit,
      mode: result.mode,
      processedMarkets: result.processedMarkets ?? 0,
      backfilledFillEvents: result.backfilledFillEvents ?? 0,
      materializedRows: result.materializedRows ?? 0,
      day: result.day || null,
    });

    console.log(JSON.stringify({ ok: true, ...result }, null, 2));
  } catch (error) {
    await reportRunnerFailure(
      opsState,
      RUNNER_NAME,
      error?.status ? `http_${error.status}` : "materializer_failed",
      error instanceof Error ? error.message : String(error),
      {
        startedAt,
        owner: options.owner,
        marketId: options.marketId,
        limit: options.limit,
        details: error?.payload || null,
      },
    );
    throw error;
  } finally {
    await closeOpsState(opsState);
  }
}

main().catch((error) => {
  console.error(
    JSON.stringify(
      {
        ok: false,
        message: error instanceof Error ? error.message : String(error),
        status: error?.status || null,
        details: error?.payload || null,
      },
      null,
      2,
    ),
  );
  process.exit(1);
});
