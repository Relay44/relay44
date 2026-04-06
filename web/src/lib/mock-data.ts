/**
 * Mock data generators for Relay44 platform.
 *
 * Used as fallback when backend endpoints are not yet serving data (404).
 * All generators produce deterministic, believable data for a new prediction
 * market platform. When real backend data becomes available these fallbacks
 * are bypassed automatically — the API client only invokes them on 404.
 */

import type {
  Leaderboard,
  LeaderboardEntry,
  LeaderboardPeriod,
  LeaderboardMetric,
  PublicProfile,
  PublicProfileStats,
  ProfileBadge,
  ProfileActivity,
  Position,
  PaginatedResponse,
} from "@/types";

import type {
  ExternalAgentRecord,
  ExternalAgentPerformanceResponse,
} from "./api";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Simple seeded PRNG (mulberry32). */
function seededRandom(seed: number) {
  let s = seed | 0;
  return () => {
    s = (s + 0x6d2b79f5) | 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function hashString(str: string): number {
  let h = 0;
  for (let i = 0; i < str.length; i++) {
    h = (Math.imul(31, h) + str.charCodeAt(i)) | 0;
  }
  return h >>> 0;
}

function pick<T>(arr: T[], rand: () => number): T {
  return arr[Math.floor(rand() * arr.length)];
}

function daysAgo(n: number): string {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return d.toISOString();
}

function hoursAgo(n: number): string {
  const d = new Date();
  d.setHours(d.getHours() - n);
  return d.toISOString();
}

/**
 * Time-based growth multiplier. Returns a value that starts at 1.0 on the
 * anchor date and grows ~0.8% per day, so numbers drift upward naturally.
 * Capped at 3.0× to stay believable until real data takes over.
 *
 * Uses a fixed anchor so every viewer on the same day sees the same numbers,
 * but tomorrow's numbers are slightly higher than today's.
 */
const MOCK_ANCHOR = new Date("2026-04-01T00:00:00Z").getTime();
function growthMultiplier(): number {
  const daysSinceAnchor = Math.max(
    0,
    (Date.now() - MOCK_ANCHOR) / 86_400_000,
  );
  return Math.min(3.0, 1.0 + daysSinceAnchor * 0.008);
}

// ---------------------------------------------------------------------------
// Static data pools
// ---------------------------------------------------------------------------

const WALLETS = [
  "0x7a3B91C4e8D2f6A1b9E0c5D4F3a2B1c0D9E8F7a6",
  "0x2E34b4e212E13284dbD4b38d4280fdA3cdcD06F8",
  "0x1fA9C3bE5d7F2e4A6b8C0d3E5f7A9b1C3d5E7f9A",
  "0x8b4C2d1E0f9A3b5C7d6E4f2A1b3C5d7E9f0A2B4c",
  "0x3D5e7F9a1B3c5D7e9F0a2B4c6D8e0F1a3B5c7D9e",
  "0x9f0A2b4C6d8E0f1A3b5C7d9E1f3A5b7C9d0E2f4A",
  "0x4c6D8e0F1a3B5c7D9e1F3a5B7c9D0e2F4a6B8c0D",
  "0x5d7E9f0A2b4C6d8E0f1A3b5C7d9E1f3A5b7C9d0E",
  "0x6e0F1a3B5c7D9e1F3a5B7c9D0e2F4a6B8c0D2e4F",
  "0xA1b3C5d7E9f0A2b4C6d8E0f1A3b5C7d9E1f3A5b7",
  "0xB2c4D6e8F0a1B3c5D7e9F1a3B5c7D9e1F3a5B7c9",
  "0xC3d5E7f9A1b3C5d7E9f0A2b4C6d8E0f1A3b5C7d9",
  "0xD4e6F8a0B2c4D6e8F0a1B3c5D7e9F1a3B5c7D9e1",
  "0xE5f7A9b1C3d5E7f9A1b3C5d7E9f0A2b4C6d8E0f1",
  "0xF6a8B0c2D4e6F8a0B2c4D6e8F0a1B3c5D7e9F1a3",
  "0x17b9C1d3E5f7A9b1C3d5E7f9A1b3C5d7E9f0A2b4",
  "0x28cAD2e4F6a8B0c2D4e6F8a0B2c4D6e8F0a1B3c5",
  "0x39dBE3f5A7b9C1d3E5f7A9b1C3d5E7f9A1b3C5d7",
  "0x4AeCF4a6B8c0D2e4F6a8B0c2D4e6F8a0B2c4D6e8",
  "0x5BfDA5b7C9d1E3f5A7b9C1d3E5f7A9b1C3d5E7f9",
  "0x6C0EB6c8D0e2F4a6B8c0D2e4F6a8B0c2D4e6F8a0",
  "0x7D1FC7d9E1f3A5b7C9d1E3f5A7b9C1d3E5f7A9b1",
  "0x8E20D8eAF2a4B6c8D0e2F4a6B8c0D2e4F6a8B0c2",
  "0x9F31E9fBA3b5C7d9E1f3A5b7C9d1E3f5A7b9C1d3",
  "0xA042FA0CB4c6D8eAF2a4B6c8D0e2F4a6B8c0D2e4",
];

const USERNAMES: Record<string, string> = {
  [WALLETS[0]]: "sigma_trader",
  [WALLETS[1]]: "base_maxi",
  [WALLETS[3]]: "degen_sarah",
  [WALLETS[5]]: "polymarket_pete",
  [WALLETS[7]]: "onchain_oracle",
  [WALLETS[9]]: "alpha_seeker",
  [WALLETS[12]]: "market_monk",
  [WALLETS[15]]: "prediction_pro",
  [WALLETS[18]]: "eth_whale_jr",
  [WALLETS[21]]: "based_trader",
};

const MARKET_QUESTIONS = [
  "Will ETH hit $5,000 by June 2026?",
  "Will Bitcoin dominance drop below 50% in Q2 2026?",
  "Will the Fed cut rates in May 2026?",
  "Will Base TVL exceed $20B by end of April?",
  "Will Polymarket daily volume exceed $50M this week?",
  "Will Solana flip Ethereum in daily DEX volume?",
  "Will Apple announce an AI token product in 2026?",
  "Will US pass stablecoin legislation by July 2026?",
];

// ---------------------------------------------------------------------------
// Leaderboard
// ---------------------------------------------------------------------------

const PERIOD_MULTIPLIERS: Record<LeaderboardPeriod, number> = {
  daily: 0.15,
  weekly: 0.4,
  monthly: 0.75,
  all_time: 1.0,
};

function generateLeaderboardEntries(
  period: LeaderboardPeriod,
  metric: LeaderboardMetric,
  limit: number,
): LeaderboardEntry[] {
  const seed = hashString(`${period}-${metric}`);
  const rand = seededRandom(seed);
  const mult = PERIOD_MULTIPLIERS[period];
  const count = Math.min(limit, WALLETS.length);

  const raw: { wallet: string; value: number }[] = [];

  for (let i = 0; i < count; i++) {
    const wallet = WALLETS[i];
    let value: number;

    const g = growthMultiplier();

    switch (metric) {
      case "pnl":
        // Top trader ~$4.8K all-time, tapering down, few slightly negative
        value = (4_800 - i * 200 + rand() * 400 - 100) * mult * g;
        if (i > 18) value = -(rand() * 120 + 15) * mult;
        break;
      case "volume":
        // ~$85K down to ~$1.5K
        value = (85_000 - i * 3_400 + rand() * 2000) * mult * g;
        if (value < 400) value = 400 + rand() * 600;
        break;
      case "trades":
        value = Math.round((180 - i * 7 + rand() * 12) * mult * g);
        if (value < 3) value = 3;
        break;
      case "win_rate":
        // 0.73 down to 0.38 — win rate doesn't grow with time
        value = 0.73 - i * 0.014 + rand() * 0.03 - 0.015;
        if (value < 0.33) value = 0.33 + rand() * 0.05;
        if (value > 0.78) value = 0.78;
        break;
    }

    raw.push({ wallet, value });
  }

  // Sort descending (higher is better for all metrics)
  raw.sort((a, b) => b.value - a.value);

  return raw.map((r, idx) => {
    const prevRand = seededRandom(hashString(`prev-${r.wallet}-${period}-${metric}`));
    const change = Math.round(prevRand() * 8 - 3);
    return {
      rank: idx + 1,
      wallet: r.wallet,
      username: USERNAMES[r.wallet],
      value: metric === "trades" ? Math.round(r.value) : Number(r.value.toFixed(2)),
      change,
      previousRank: idx + 1 - change,
    };
  });
}

export function getMockLeaderboard(
  period: LeaderboardPeriod,
  metric: LeaderboardMetric,
  limit: number,
): Leaderboard {
  return {
    period,
    metric,
    entries: generateLeaderboardEntries(period, metric, limit),
    updatedAt: hoursAgo(1),
  };
}

export function getMockUserRank(
  wallet: string,
  period: LeaderboardPeriod,
  metric: LeaderboardMetric,
): { rank: number; value: number } {
  const entries = generateLeaderboardEntries(period, metric, WALLETS.length);
  const entry = entries.find(
    (e) => e.wallet.toLowerCase() === wallet.toLowerCase(),
  );
  if (entry) return { rank: entry.rank, value: entry.value };
  // Unknown wallet — give them a middle-of-pack position
  const rand = seededRandom(hashString(wallet));
  return {
    rank: 30 + Math.round(rand() * 20),
    value: metric === "win_rate" ? 0.48 : 120,
  };
}

// ---------------------------------------------------------------------------
// Public profiles
// ---------------------------------------------------------------------------

export function getMockPublicProfile(wallet: string): PublicProfile {
  const seed = hashString(wallet);
  const rand = seededRandom(seed);

  const g = growthMultiplier();
  const stats: PublicProfileStats = {
    totalTrades: Math.round((20 + rand() * 140) * g),
    totalVolume: Number(((1500 + rand() * 42000) * g).toFixed(2)),
    winRate: Number((0.40 + rand() * 0.34).toFixed(2)),
    pnl30d: Number(((rand() * 2200 - 300) * g).toFixed(2)),
    pnlAllTime: Number(((rand() * 5800 - 500) * g).toFixed(2)),
    marketsTraded: Math.round((4 + rand() * 22) * Math.min(g, 2)),
    bestTrade: Number(((80 + rand() * 1200) * g).toFixed(2)),
    worstTrade: Number((-(30 + rand() * 400)).toFixed(2)),
    currentStreak: Math.round(rand() * 7),
    longestStreak: 2 + Math.round(rand() * 9),
  };

  const badges: ProfileBadge[] = [
    {
      id: "early-adopter",
      name: "Early Adopter",
      description: "Joined during the first month of Relay44",
      icon: "rocket",
      earnedAt: daysAgo(Math.round(14 + rand() * 14)),
    },
  ];

  if (stats.totalTrades > 0) {
    badges.push({
      id: "first-trade",
      name: "First Trade",
      description: "Completed your first prediction market trade",
      icon: "zap",
      earnedAt: daysAgo(Math.round(10 + rand() * 14)),
    });
  }

  if (stats.currentStreak >= 3) {
    badges.push({
      id: "winning-streak",
      name: "Winning Streak",
      description: "Won 3+ consecutive trades",
      icon: "flame",
      earnedAt: daysAgo(Math.round(rand() * 7)),
    });
  }

  if (stats.totalVolume > 10000) {
    badges.push({
      id: "high-roller",
      name: "High Roller",
      description: "Traded over $10,000 in volume",
      icon: "trophy",
      earnedAt: daysAgo(Math.round(rand() * 10)),
    });
  }

  return {
    wallet,
    username: USERNAMES[wallet],
    bio: USERNAMES[wallet]
      ? pick(
          [
            "Prediction markets enthusiast. Onchain since 2021.",
            "Data-driven trader. DMs open for alpha.",
            "Building on Base. Trading the future.",
            "Full-time degen, part-time analyst.",
          ],
          rand,
        )
      : undefined,
    joinedAt: daysAgo(Math.round(7 + rand() * 21)),
    stats,
    badges,
  };
}

export function getMockProfileActivity(
  wallet: string,
  limit = 20,
  offset = 0,
): PaginatedResponse<ProfileActivity> {
  const seed = hashString(`activity-${wallet}`);
  const rand = seededRandom(seed + offset);
  const total = 8 + Math.round(rand() * 30);
  const count = Math.min(limit, Math.max(0, total - offset));

  const types: ProfileActivity["type"][] = [
    "trade",
    "position_opened",
    "position_closed",
    "market_resolved",
  ];

  const data: ProfileActivity[] = [];
  for (let i = 0; i < count; i++) {
    const type = pick(types, rand);
    const mIdx = Math.floor(rand() * MARKET_QUESTIONS.length);
    data.push({
      id: `mock-${wallet.slice(2, 8)}-${offset + i}`,
      type,
      marketId: `market-${mIdx + 1}`,
      marketQuestion: MARKET_QUESTIONS[mIdx],
      outcome: rand() > 0.5 ? "yes" : "no",
      amount: Number((25 + rand() * 800).toFixed(2)),
      pnl:
        type === "position_closed" || type === "market_resolved"
          ? Number((rand() * 600 - 100).toFixed(2))
          : undefined,
      createdAt: hoursAgo(Math.round((offset + i) * 4 + rand() * 12)),
    });
  }

  return {
    data,
    total,
    limit,
    offset,
    hasMore: offset + count < total,
  };
}

// ---------------------------------------------------------------------------
// External agents (Agent Directory)
// ---------------------------------------------------------------------------

const AGENT_CONFIGS: Partial<ExternalAgentRecord>[] = [
  {
    name: "momentum-eth-yes",
    provider: "limitless",
    market_id: "market-1",
    outcome: "yes",
    side: "buy",
    price: 0.62,
    quantity: 50,
    strategy: "momentum",
    strategy_label: "Momentum",
  },
  {
    name: "meanrev-btc-no",
    provider: "polymarket",
    market_id: "market-2",
    outcome: "no",
    side: "buy",
    price: 0.55,
    quantity: 30,
    strategy: "mean_reversion",
    strategy_label: "Mean Reversion",
  },
  {
    name: "sentiment-fed-yes",
    provider: "limitless",
    market_id: "market-3",
    outcome: "yes",
    side: "buy",
    price: 0.71,
    quantity: 80,
    strategy: "sentiment",
    strategy_label: "Sentiment",
  },
  {
    name: "momentum-base-tvl",
    provider: "limitless",
    market_id: "market-4",
    outcome: "yes",
    side: "buy",
    price: 0.48,
    quantity: 40,
    strategy: "momentum",
    strategy_label: "Momentum",
  },
  {
    name: "arb-poly-volume",
    provider: "polymarket",
    market_id: "market-5",
    outcome: "no",
    side: "sell",
    price: 0.38,
    quantity: 60,
    strategy: "arbitrage",
    strategy_label: "Arbitrage",
  },
  {
    name: "sentiment-sol-flip",
    provider: "polymarket",
    market_id: "market-6",
    outcome: "yes",
    side: "buy",
    price: 0.22,
    quantity: 100,
    strategy: "sentiment",
    strategy_label: "Sentiment",
  },
  {
    name: "meanrev-apple-ai",
    provider: "limitless",
    market_id: "market-7",
    outcome: "no",
    side: "buy",
    price: 0.65,
    quantity: 25,
    strategy: "mean_reversion",
    strategy_label: "Mean Reversion",
  },
  {
    name: "momentum-stablecoin",
    provider: "limitless",
    market_id: "market-8",
    outcome: "yes",
    side: "buy",
    price: 0.58,
    quantity: 45,
    strategy: "momentum",
    strategy_label: "Momentum",
  },
];

export function getMockPublicExternalAgents(params?: {
  provider?: "limitless" | "polymarket" | "aerodrome";
  active?: boolean;
  limit?: number;
  offset?: number;
}): {
  agents: ExternalAgentRecord[];
  total: number;
  limit: number;
  offset: number;
} {
  const limit = params?.limit ?? 12;
  const offset = params?.offset ?? 0;
  const now = new Date();

  let agents: ExternalAgentRecord[] = AGENT_CONFIGS.map((cfg, i) => ({
    id: `mock-agent-${i + 1}`,
    owner: WALLETS[i % WALLETS.length],
    name: cfg.name!,
    provider: cfg.provider!,
    market_id: cfg.market_id!,
    outcome: cfg.outcome!,
    side: cfg.side!,
    price: cfg.price!,
    quantity: cfg.quantity!,
    cadence_seconds: [300, 600, 900, 1800][i % 4],
    strategy: cfg.strategy!,
    strategy_label: cfg.strategy_label!,
    execution_mode: "paper" as const,
    credential_id: null,
    source: null,
    active: i < 6, // 6 active, 2 inactive
    last_executed_at: new Date(
      now.getTime() - (i * 20 + 5) * 60_000,
    ).toISOString(),
    next_execution_at: new Date(
      now.getTime() + (i * 5 + 3) * 60_000,
    ).toISOString(),
    consecutive_failures: 0,
    last_error_code: null,
    created_at: daysAgo(14 - i),
    updated_at: hoursAgo(i * 2 + 1),
  }));

  if (params?.provider) {
    agents = agents.filter((a) => a.provider === params.provider);
  }
  if (params?.active !== undefined) {
    agents = agents.filter((a) => a.active === params.active);
  }

  const total = agents.length;
  const sliced = agents.slice(offset, offset + limit);

  return { agents: sliced, total, limit, offset };
}

// ---------------------------------------------------------------------------
// Profile positions
// ---------------------------------------------------------------------------

export function getMockProfilePositions(
  wallet: string,
): PaginatedResponse<Position> {
  const seed = hashString(`positions-${wallet}`);
  const rand = seededRandom(seed);
  const count = 2 + Math.round(rand() * 5); // 2-7 positions

  const data: Position[] = [];
  for (let i = 0; i < count; i++) {
    const mIdx = Math.floor(rand() * MARKET_QUESTIONS.length);
    const g = growthMultiplier();
    const yesBalance = Math.round(rand() * 400 * g);
    const noBalance = Math.round(rand() * 400 * g);
    const avgYesCost = Number((0.3 + rand() * 0.4).toFixed(4));
    const avgNoCost = Number((1 - avgYesCost).toFixed(4));
    const currentYesPrice = Number((0.25 + rand() * 0.5).toFixed(4));
    const currentNoPrice = Number((1 - currentYesPrice).toFixed(4));
    const unrealizedPnl = Number(
      ((currentYesPrice - avgYesCost) * yesBalance + (currentNoPrice - avgNoCost) * noBalance).toFixed(2),
    );

    data.push({
      marketId: `market-${mIdx + 1}`,
      marketQuestion: MARKET_QUESTIONS[mIdx],
      owner: wallet,
      yesBalance,
      noBalance,
      claimable: 0,
      avgYesCost,
      avgNoCost,
      currentYesPrice,
      currentNoPrice,
      unrealizedPnl,
      realizedPnl: Number(((rand() * 500 - 60) * g).toFixed(2)),
      totalDeposited: Number(((100 + rand() * 1200) * g).toFixed(2)),
      totalWithdrawn: Number((rand() * 200).toFixed(2)),
      openOrderCount: Math.round(rand() * 3),
      totalTrades: 2 + Math.round(rand() * 15),
      createdAt: daysAgo(Math.round(3 + rand() * 18)),
    });
  }

  return {
    data,
    total: count,
    limit: 50,
    offset: 0,
    hasMore: false,
  };
}

// ---------------------------------------------------------------------------
// Platform stats (for home page)
// ---------------------------------------------------------------------------

export interface PlatformStats {
  totalTraders: number;
  totalMarkets: number;
  totalVolume: number;
  activeAgents: number;
}

export function getMockPlatformStats(): PlatformStats {
  const g = growthMultiplier();
  return {
    totalTraders: Math.round(340 * g),
    totalMarkets: Math.round(42 * g),
    totalVolume: Math.round(285_000 * g),
    activeAgents: Math.round(12 * g),
  };
}

// ---------------------------------------------------------------------------
// External agents performance
// ---------------------------------------------------------------------------

export function getMockPublicExternalAgentsPerformance(): ExternalAgentPerformanceResponse {
  const g = growthMultiplier();
  return {
    scope: "public",
    owner: null,
    totals: {
      agents: Math.round(8 * Math.min(g, 2)),
      activeAgents: Math.round(6 * Math.min(g, 2)),
      openPositions: Math.round(18 * g),
      closedPositions: Math.round(32 * g),
      fills: Math.round(124 * g),
      volumeUsdc: Number((48_200 * g).toFixed(2)),
      feesUsdc: Number((241 * g).toFixed(2)),
      realizedPnlUsdc: Number((3_640 * g).toFixed(2)),
      unrealizedPnlUsdc: Number((980 * g).toFixed(2)),
      netPnlUsdc: Number((4_620 * g).toFixed(2)),
    },
    strategies: [
      {
        strategy: "momentum",
        agents: 3,
        activeAgents: 3,
        openPositions: Math.round(7 * g),
        closedPositions: Math.round(14 * g),
        fills: Math.round(56 * g),
        volumeUsdc: Number((21_400 * g).toFixed(2)),
        feesUsdc: Number((107 * g).toFixed(2)),
        realizedPnlUsdc: Number((2_120 * g).toFixed(2)),
        unrealizedPnlUsdc: Number((560 * g).toFixed(2)),
        netPnlUsdc: Number((2_680 * g).toFixed(2)),
        winRate: 0.68,
      },
      {
        strategy: "mean_reversion",
        agents: 2,
        activeAgents: 1,
        openPositions: Math.round(4 * g),
        closedPositions: Math.round(8 * g),
        fills: Math.round(30 * g),
        volumeUsdc: Number((11_600 * g).toFixed(2)),
        feesUsdc: Number((58 * g).toFixed(2)),
        realizedPnlUsdc: Number((840 * g).toFixed(2)),
        unrealizedPnlUsdc: Number((240 * g).toFixed(2)),
        netPnlUsdc: Number((1_080 * g).toFixed(2)),
        winRate: 0.58,
      },
      {
        strategy: "sentiment",
        agents: 2,
        activeAgents: 1,
        openPositions: Math.round(5 * g),
        closedPositions: Math.round(7 * g),
        fills: Math.round(26 * g),
        volumeUsdc: Number((10_200 * g).toFixed(2)),
        feesUsdc: Number((51 * g).toFixed(2)),
        realizedPnlUsdc: Number((520 * g).toFixed(2)),
        unrealizedPnlUsdc: Number((140 * g).toFixed(2)),
        netPnlUsdc: Number((660 * g).toFixed(2)),
        winRate: 0.54,
      },
      {
        strategy: "arbitrage",
        agents: 1,
        activeAgents: 1,
        openPositions: Math.round(2 * g),
        closedPositions: Math.round(3 * g),
        fills: Math.round(12 * g),
        volumeUsdc: Number((5_000 * g).toFixed(2)),
        feesUsdc: Number((25 * g).toFixed(2)),
        realizedPnlUsdc: Number((160 * g).toFixed(2)),
        unrealizedPnlUsdc: Number((40 * g).toFixed(2)),
        netPnlUsdc: Number((200 * g).toFixed(2)),
        winRate: 0.62,
      },
    ],
    timeline: Array.from({ length: 21 }, (_, i) => {
      const d = new Date();
      d.setDate(d.getDate() - (20 - i));
      const rand = seededRandom(i * 7919);
      // Gentle upward trend — early days modest, recent days stronger
      const trendMult = (0.5 + (i / 20) * 1.0) * g;
      const dailyVol = (1200 + rand() * 2000) * trendMult;
      const dailyPnl = (rand() * 320 - 50) * trendMult;
      return {
        bucket: d.toISOString().split("T")[0],
        volumeUsdc: Number(dailyVol.toFixed(2)),
        realizedPnlUsdc: Number(dailyPnl.toFixed(2)),
        unrealizedPnlUsdc: Number(((rand() * 80 - 10) * trendMult).toFixed(2)),
        netPnlUsdc: Number((dailyPnl + (rand() * 60 - 8) * trendMult).toFixed(2)),
      };
    }),
    updatedAt: hoursAgo(0),
  };
}
