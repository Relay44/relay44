export type Address = `0x${string}`;

export enum PositionSizing {
  Fixed = 0,
  Kelly = 1,
  Proportional = 2,
}

export enum AgentStatus {
  Active = 0,
  Paused = 1,
  Stopped = 2,
}

export enum Outcome {
  Yes = 0,
  No = 1,
}

export enum OrderType {
  Limit = 0,
  Market = 1,
}

export interface RiskParams {
  maxDrawdownBps: number;
  maxDailyLoss: bigint;
  minEdgeBps: number;
  positionSizing: PositionSizing;
  sizingParam: bigint;
}

export interface TradingAgentConfig {
  owner: Address;
  status: AgentStatus;
  maxPositionSize: bigint;
  maxTotalExposure: bigint;
  riskParams: RiskParams;
  availableBalance: bigint;
  lockedBalance: bigint;
  totalPnl: bigint;
  highWaterMark: bigint;
  currentDrawdown: bigint;
  dailyLoss: bigint;
}

export interface OrderParams {
  marketId: bigint;
  outcome: Outcome;
  priceBps: number;
  quantity: bigint;
  orderType: OrderType;
  expirySeconds?: number;
}

export interface Signal {
  marketId: bigint;
  direction: 'buy_yes' | 'buy_no';
  confidence: number;
  targetPriceBps: number;
  reason?: string;
}

export interface MarketData {
  marketId: bigint;
  yesPriceBps: number;
  noPriceBps: number;
  matchedVolume: bigint;
  lastUpdate: number;
  resolved: boolean;
  outcome?: Outcome;
}

export interface TradeResult {
  success: boolean;
  txHash?: string;
  orderId?: bigint;
  error?: string;
}

export interface AgentMetrics {
  totalPnl: bigint;
  winRate: number;
  tradesCount: bigint;
  avgPnlPerTrade: bigint;
  maxDrawdownBps: number;
}
