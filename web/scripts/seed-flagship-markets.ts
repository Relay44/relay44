#!/usr/bin/env npx tsx
/**
 * Seed script: creates 4 flagship distribution markets via the Relay44 API.
 *
 * Usage:
 *   npx tsx scripts/seed-flagship-markets.ts
 *   npx tsx scripts/seed-flagship-markets.ts --dry-run
 *
 * Env vars:
 *   API_URL            — defaults to https://relay44.com/api
 *   RELAY44_AUTH_TOKEN — required (Bearer token)
 */

const API_URL = process.env.API_URL ?? "https://relay44.com/api";
const AUTH_TOKEN = process.env.RELAY44_AUTH_TOKEN ?? "";
const DRY_RUN = process.argv.includes("--dry-run");

interface MarketPayload {
  marketId: string;
  question: string;
  description: string;
  category: string;
  outcomeMin: number;
  outcomeMax: number;
  outcomeUnit: string;
  tradingEnd: string;
  liquidityParam: number;
  feeBps: number;
  collateralToken: string;
  useOracle: boolean;
  oracleFeedId?: string;
  resolutionDeadline: string;
}

const markets: MarketPayload[] = [
  {
    marketId: "dist-btc-eoy-2026",
    question: "What will the price of BTC be at the end of 2026?",
    description:
      "Resolves to the BTC/USD spot price at 23:59 UTC on Dec 31 2026, sourced from the Pyth BTC/USD price feed. Settlement is paid out as the expected value of the realized point estimate under each position's curve.",
    category: "crypto",
    outcomeMin: 40000,
    outcomeMax: 300000,
    outcomeUnit: "USD",
    tradingEnd: "2026-12-30T23:00:00.000Z",
    liquidityParam: 300,
    feeBps: 100,
    collateralToken: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
    useOracle: true,
    oracleFeedId:
      "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43",
    resolutionDeadline: "2026-12-31T23:59:59.000Z",
  },
  {
    marketId: "dist-fed-sept-2026",
    question:
      "What will the Fed funds upper-bound target rate be after the September 2026 FOMC meeting?",
    description:
      "Resolves to the upper bound of the federal funds target range announced at the September 2026 FOMC meeting, as published in the official FOMC statement. Manual resolution within 48 hours of statement release. Source: federalreserve.gov/monetarypolicy/fomccalendars.htm",
    category: "finance",
    outcomeMin: 2.0,
    outcomeMax: 6.0,
    outcomeUnit: "%",
    tradingEnd: "2026-09-16T17:00:00.000Z",
    liquidityParam: 300,
    feeBps: 100,
    collateralToken: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
    useOracle: false,
    resolutionDeadline: "2026-09-18T17:00:00.000Z",
  },
  {
    marketId: "dist-cpi-next-2026",
    question: "What will US CPI YoY (headline) print for the next BLS release?",
    description:
      "Resolves to the headline US CPI year-over-year percentage change as published by the Bureau of Labor Statistics in the next scheduled CPI release. Manual resolution within 24 hours of BLS release. Source: bls.gov/cpi.",
    category: "finance",
    outcomeMin: 1.0,
    outcomeMax: 6.0,
    outcomeUnit: "% YoY",
    tradingEnd: "2026-05-12T23:00:00.000Z",
    liquidityParam: 300,
    feeBps: 100,
    collateralToken: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
    useOracle: false,
    resolutionDeadline: "2026-05-14T23:00:00.000Z",
  },
  {
    marketId: "dist-ethbtc-eoy-2026",
    question: "What will the ETH/BTC ratio be at the end of 2026?",
    description:
      "Resolves to the ETH/USD price divided by the BTC/USD price at 23:59 UTC on Dec 31 2026, both sourced from Pyth price feeds on Base. Settlement is paid out as the expected value of the realized point estimate under each position's curve.",
    category: "crypto",
    outcomeMin: 0.02,
    outcomeMax: 0.1,
    outcomeUnit: "ETH/BTC",
    tradingEnd: "2026-12-30T23:00:00.000Z",
    liquidityParam: 300,
    feeBps: 100,
    collateralToken: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
    useOracle: false,
    resolutionDeadline: "2026-12-31T23:59:59.000Z",
  },
];

async function createMarket(market: MarketPayload): Promise<void> {
  const url = `${API_URL}/v1/distribution/markets`;

  const res = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${AUTH_TOKEN}`,
    },
    body: JSON.stringify(market),
  });

  if (!res.ok) {
    const text = await res.text().catch(() => "(no body)");
    throw new Error(`HTTP ${res.status}: ${text}`);
  }
}

async function main(): Promise<void> {
  if (DRY_RUN) {
    console.log("--- DRY RUN: payloads that would be POSTed ---\n");
    for (const market of markets) {
      console.log(JSON.stringify(market, null, 2));
      console.log();
    }
    console.log(`Endpoint: POST ${API_URL}/v1/distribution/markets`);
    return;
  }

  if (!AUTH_TOKEN) {
    console.error("Error: RELAY44_AUTH_TOKEN env var is not set.");
    process.exit(1);
  }

  let anyFailed = false;

  for (const market of markets) {
    process.stdout.write(`Creating ${market.marketId} ... `);
    try {
      await createMarket(market);
      console.log("OK");
    } catch (err) {
      console.log("FAILED");
      console.error(`  -> ${err instanceof Error ? err.message : String(err)}`);
      anyFailed = true;
    }
  }

  if (anyFailed) {
    console.error("\nOne or more markets failed to create.");
    process.exit(1);
  }

  console.log("\nAll markets created successfully.");
}

main();
