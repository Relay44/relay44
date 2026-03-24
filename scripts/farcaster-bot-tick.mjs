#!/usr/bin/env node

import {
  loginAdmin,
  fetchMarkets,
  publishCast,
  loadState,
  saveState,
  composeNewMarketCast,
  composeResolutionCast,
  composeMovementCast,
  getNextTechPost,
  siteUrl,
  PRICE_CHANGE_THRESHOLD,
  VOLUME_THRESHOLD,
  MAX_CASTS_PER_TICK,
  COOLDOWN_MS,
  TECH_POST_COOLDOWN_MS,
} from './farcaster-bot-lib.mjs';

function isEnabled(raw, fallback = true) {
  if (raw == null || raw === '') return fallback;
  return ['1', 'true', 'yes', 'on'].includes(String(raw).trim().toLowerCase());
}

async function main() {
  if (!isEnabled(process.env.FARCASTER_BOT_ENABLED, false)) {
    console.log(JSON.stringify({ ok: true, skipped: true, reason: 'farcaster bot disabled' }));
    return;
  }

  const accessToken = await loginAdmin();
  const state = loadState();
  const now = Date.now();
  const casts = [];

  // Fetch active markets
  const activeResponse = await fetchMarkets(accessToken, {
    limit: '200',
    source: 'all',
    tradable: 'all',
  });
  const activeMarkets = activeResponse?.markets || activeResponse?.data || [];

  // Fetch recently resolved markets
  let resolvedMarkets = [];
  try {
    const resolvedResponse = await fetchMarkets(accessToken, {
      limit: '50',
      status: 'resolved',
    });
    resolvedMarkets = resolvedResponse?.markets || resolvedResponse?.data || [];
  } catch {
    // resolved endpoint may not exist or return differently
  }

  const knownIds = new Set(state.knownMarketIds);
  const lastPrices = state.lastPrices || {};
  const postedResolutions = new Set(state.postedResolutions || []);
  const cooldowns = state.cooldowns || {};

  // ── Detect new markets ────────────────────────────────────────────────────
  for (const market of activeMarkets) {
    if (casts.length >= MAX_CASTS_PER_TICK) break;
    if (knownIds.has(market.id)) continue;

    // Only post about markets created in the last 10 minutes (avoid flooding on first run)
    const createdAt = new Date(market.created_at || market.createdAt || 0).getTime();
    if (state.lastTickAt && createdAt > new Date(state.lastTickAt).getTime()) {
      const cast = composeNewMarketCast(market);
      casts.push({ type: 'new_market', marketId: market.id, ...cast });
    }
  }

  // ── Detect resolutions ────────────────────────────────────────────────────
  for (const market of resolvedMarkets) {
    if (casts.length >= MAX_CASTS_PER_TICK) break;
    if (postedResolutions.has(market.id)) continue;

    const cast = composeResolutionCast(market);
    casts.push({ type: 'resolution', marketId: market.id, ...cast });
    postedResolutions.add(market.id);
  }

  // ── Detect price movements ────────────────────────────────────────────────
  for (const market of activeMarkets) {
    if (casts.length >= MAX_CASTS_PER_TICK) break;

    const yesPrice = market.outcomes?.[0]?.probability ?? market.yes_price ?? null;
    if (yesPrice == null) continue;

    const prevPrice = lastPrices[market.id];
    if (prevPrice == null) continue;

    const change = Math.abs(yesPrice - prevPrice);
    if (change < PRICE_CHANGE_THRESHOLD) continue;

    // Check cooldown
    const lastPosted = cooldowns[market.id];
    if (lastPosted && now - lastPosted < COOLDOWN_MS) continue;

    const direction = yesPrice > prevPrice ? 'up' : 'down';
    const changePercent = Math.round(change * 100);
    const cast = composeMovementCast(market, changePercent, direction);
    casts.push({ type: 'movement', marketId: market.id, ...cast });
    cooldowns[market.id] = now;
  }

  // ── Detect volume spikes ──────────────────────────────────────────────────
  for (const market of activeMarkets) {
    if (casts.length >= MAX_CASTS_PER_TICK) break;

    const vol = market.volume_24h || market.volume24h || 0;
    if (vol < VOLUME_THRESHOLD) continue;

    // Only post if we haven't posted about this market recently
    const lastPosted = cooldowns[market.id];
    if (lastPosted && now - lastPosted < COOLDOWN_MS) continue;

    // Skip if we already queued a cast for this market in this tick
    if (casts.some((c) => c.marketId === market.id)) continue;

    const yes = Math.round((market.outcomes?.[0]?.probability ?? market.yes_price ?? 0.5) * 100);
    casts.push({
      type: 'volume_spike',
      marketId: market.id,
      text: `Trending: ${market.question}\n\nYES ${yes}% | 24h volume: $${vol.toLocaleString()}\n\nTrade on relay44`,
      embedUrl: `https://relay44.com/miniapp/market/${encodeURIComponent(market.id)}`,
    });
    cooldowns[market.id] = now;
  }

  // ── Tech post (once per hour) ─────────────────────────────────────────────
  const lastTechPostAt = state.lastTechPostAt || 0;
  const techPostIndex = state.techPostIndex ?? -1;
  if (now - new Date(lastTechPostAt).getTime() >= TECH_POST_COOLDOWN_MS) {
    const { text, index } = getNextTechPost(techPostIndex);
    casts.push({ type: 'tech_post', marketId: null, text, embedUrl: siteUrl });
    state._nextTechPostIndex = index;
  }

  // ── Publish casts ─────────────────────────────────────────────────────────
  const results = [];
  for (const cast of casts) {
    try {
      const result = await publishCast(cast.text, cast.embedUrl);
      results.push({ ok: true, type: cast.type, marketId: cast.marketId, hash: result?.cast?.hash });
    } catch (err) {
      results.push({ ok: false, type: cast.type, marketId: cast.marketId, error: err.message });
    }
  }

  // ── Update state ──────────────────────────────────────────────────────────
  const updatedPrices = {};
  for (const market of activeMarkets) {
    const yesPrice = market.outcomes?.[0]?.probability ?? market.yes_price ?? null;
    if (yesPrice != null) {
      updatedPrices[market.id] = yesPrice;
    }
  }

  // Clean old cooldowns (older than 24h)
  const cleanedCooldowns = {};
  for (const [id, ts] of Object.entries(cooldowns)) {
    if (now - ts < 24 * 60 * 60 * 1000) {
      cleanedCooldowns[id] = ts;
    }
  }

  saveState({
    lastTickAt: new Date().toISOString(),
    knownMarketIds: activeMarkets.map((m) => m.id),
    lastPrices: updatedPrices,
    postedResolutions: [...postedResolutions].slice(-500),
    cooldowns: cleanedCooldowns,
    lastTechPostAt: state._nextTechPostIndex != null ? new Date().toISOString() : (state.lastTechPostAt || null),
    techPostIndex: state._nextTechPostIndex ?? (state.techPostIndex ?? -1),
  });

  console.log(
    JSON.stringify(
      {
        ok: true,
        activeMarkets: activeMarkets.length,
        resolvedMarkets: resolvedMarkets.length,
        castsAttempted: casts.length,
        results,
      },
      null,
      2,
    ),
  );
}

main().catch((error) => {
  console.error(
    JSON.stringify(
      {
        ok: false,
        message: error.message,
        status: error.status || null,
        details: error.payload || null,
      },
      null,
      2,
    ),
  );
  process.exit(1);
});
