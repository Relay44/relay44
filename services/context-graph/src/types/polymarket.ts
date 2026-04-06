export interface OddsSnapshot {
  timestamp: string;
  outcomes: Record<string, number>;
  volume24h: number;
}

export interface MarketInput {
  marketUrl?: string;
  conditionId?: string;
  slug?: string;
  depth?: 'quick' | 'full';
}

export interface MarketContext {
  conditionId: string;
  question: string;
  description: string;
  outcomes: string[];
  currentOdds: Record<string, number>;
  volume24h: number;
  totalLiquidity: number;
  movementHistory: OddsSnapshot[];
  category: string;
  createdAt: string;
  endDate: string;
  slug: string;
  active: boolean;
}

export interface GammaMarketResponse {
  id: string;
  question: string;
  description: string;
  outcomes: string;
  outcomePrices: string;
  volume: string;
  liquidity: string;
  slug: string;
  category: string;
  startDate: string;
  endDate: string;
  active: boolean;
  conditionId: string;
  clobTokenIds: string;
}

export interface ClobPriceHistory {
  history: Array<{
    t: number;
    p: number;
  }>;
}

export interface ClobOrderbook {
  market: string;
  asset_id: string;
  bids: Array<{ price: string; size: string }>;
  asks: Array<{ price: string; size: string }>;
}
