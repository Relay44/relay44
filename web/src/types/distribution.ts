export interface DistributionMarket {
  id: string;
  question: string;
  description?: string;
  category?: string;
  status: 'active' | 'paused' | 'closed' | 'resolved' | 'cancelled';
  outcomeMin: number;
  outcomeMax: number;
  outcomeUnit?: string;
  liquidityParam: number;
  marketMu?: number;
  marketSigma?: number;
  stiffness?: number;
  peakDensity?: number;
  headroomPct?: number;
  lambda?: number;
  collateralToken: string;
  totalCollateral: number;
  totalVolume: number;
  volume24h: number;
  feeBps: number;
  resolver?: string;
  useOracle: boolean;
  oracleFeedId?: string;
  resolvedValue?: number;
  tradingEnd?: string;
  resolutionDeadline?: string;
  createdAt: string;
  resolvedAt?: string;
}

export interface DistributionPosition {
  id: number;
  positionId: number;
  marketId: string;
  owner: string;
  mu: number;
  sigma: number;
  size: number;
  collateral: number;
  costBasis?: number;
  status: 'open' | 'closed' | 'resolved' | 'claimed';
  payout?: number;
  pnl?: number;
  createdAt: string;
  closedAt?: string;
}

export interface DistributionQuote {
  cost: number;
  collateralToken: string;
  newMarketMu: number;
  newMarketSigma: number;
  deltaMu: number;
  deltaSigma: number;
  stiffness: number;
  peakDensity: number;
  headroomPct: number;
  lambda: number;
  fees: number;
  minFx: number;
  argMinX: number;
}

export interface CurvePoint {
  x: number;
  marketPdf: number;
  proposalPdf?: number;
  cdf: number;
}

export interface CurveSnapshot {
  marketMu: number;
  marketSigma: number;
  totalCollateral: number;
  positionCount: number;
  capturedAt: string;
}
