#!/usr/bin/env node

import {
  loginAdmin,
  fetchResolvedMarkets,
  fetchActiveMarkets,
  buildCalibrationCurve,
  generateCalibrationSignals,
  generateTimeDecaySignals,
  persistSignals,
  persistCalibrationCurve,
} from './edge-scanner-lib.mjs';

function isEnabled(raw, fallback = true) {
  if (raw == null || raw === '') return fallback;
  return ['1', 'true', 'yes', 'on'].includes(String(raw).trim().toLowerCase());
}

async function collectResolvedMarkets() {
  const pageSize = 100;
  const maxPages = Number(process.env.EDGE_SCANNER_MAX_RESOLVED_PAGES || 10);
  const all = [];

  for (let page = 0; page < maxPages; page++) {
    const batch = await fetchResolvedMarkets({
      limit: pageSize,
      offset: page * pageSize,
    });
    const markets = Array.isArray(batch) ? batch : [];
    all.push(...markets);
    if (markets.length < pageSize) break;
  }

  return all;
}

async function collectActiveMarkets() {
  const pageSize = 100;
  const maxPages = Number(process.env.EDGE_SCANNER_MAX_ACTIVE_PAGES || 5);
  const all = [];

  for (let page = 0; page < maxPages; page++) {
    const batch = await fetchActiveMarkets({
      limit: pageSize,
      offset: page * pageSize,
    });
    const markets = Array.isArray(batch) ? batch : [];
    all.push(...markets);
    if (markets.length < pageSize) break;
  }

  return all;
}

export async function runEdgeScannerTick() {
  if (!isEnabled(process.env.EDGE_SCANNER_ENABLED, true)) {
    return { ok: true, skipped: true, reason: 'edge scanner disabled' };
  }

  const startMs = Date.now();
  const { accessToken } = await loginAdmin();

  const [resolvedMarkets, activeMarkets] = await Promise.all([
    collectResolvedMarkets(),
    collectActiveMarkets(),
  ]);

  const curve = buildCalibrationCurve(resolvedMarkets);
  const calibrationSignals = generateCalibrationSignals(curve, activeMarkets);
  const decaySignals = generateTimeDecaySignals(activeMarkets, curve);
  const allSignals = [...calibrationSignals, ...decaySignals];

  let persistResult = { persisted: 0 };
  let curveResult = { persisted: 0 };

  const dryRun = isEnabled(process.env.EDGE_SCANNER_DRY_RUN, false);

  if (!dryRun && allSignals.length > 0) {
    [persistResult, curveResult] = await Promise.all([
      persistSignals(accessToken, allSignals).catch((err) => ({
        persisted: 0,
        error: err.message,
      })),
      persistCalibrationCurve(accessToken, curve).catch((err) => ({
        persisted: 0,
        error: err.message,
      })),
    ]);
  }

  const durationMs = Date.now() - startMs;

  return {
    ok: true,
    dryRun,
    durationMs,
    resolvedMarketsScanned: resolvedMarkets.length,
    activeMarketsScanned: activeMarkets.length,
    calibrationBuckets: curve.length,
    calibrationBucketsWithEdge: curve.filter(
      (b) => Math.abs(b.edgeBps) >= Number(process.env.EDGE_SCANNER_MIN_CALIBRATION_EDGE_BPS || 500),
    ).length,
    signals: {
      calibration: calibrationSignals.length,
      timeDecay: decaySignals.length,
      total: allSignals.length,
      persisted: persistResult.persisted || 0,
    },
    curve: dryRun ? curve : undefined,
    topSignals: dryRun
      ? allSignals.slice(0, 20).map((s) => ({
          strategy: s.strategy,
          market: s.metadata.question?.slice(0, 80),
          direction: s.direction,
          edgeBps: s.edgeBps,
          kelly: s.kellyFraction,
          price: s.marketPrice,
          fair: s.fairValue,
          daysLeft: s.daysRemaining,
        }))
      : undefined,
    persistResult: dryRun ? undefined : persistResult,
    curveResult: dryRun ? undefined : curveResult,
  };
}

async function main() {
  const result = await runEdgeScannerTick();
  console.log(JSON.stringify(result, null, 2));

  if (result?.skipped) return;

  const intervalMs = Number(process.env.EDGE_SCANNER_INTERVAL_MS || 0);
  if (intervalMs <= 0) return;

  for (;;) {
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
    const tick = await runEdgeScannerTick();
    console.log(JSON.stringify(tick, null, 2));
  }
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
