export type MarketStatus = 'active' | 'paused' | 'closed' | 'resolved' | 'cancelled';
export type Outcome = 'yes' | 'no';
export type MarketSource = 'all' | 'internal' | 'limitless' | 'polymarket';
export type TradableFilter = 'all' | 'user' | 'agent';
export type AgentStatus = 'ready' | 'cooldown' | 'inactive';
export type OrderSide = 'buy' | 'sell';
export type OrderStatus = 'open' | 'partially_filled' | 'filled' | 'cancelled' | 'expired';
export type OrderType = 'limit' | 'market';
export type TransactionType = 'deposit' | 'withdraw' | 'buy' | 'sell' | 'claim' | 'mint' | 'redeem';
export type MarketFrequency = 'daily' | 'weekly' | 'monthly' | 'annually' | 'one-time';

export interface MarketOutcome {
  label: string;
  probability: number;
}

export interface Market {
  id: string;
  address: string;
  source: MarketSource;
  provider: string;
  isExternal: boolean;
  externalUrl?: string;
  chainId: number;
  requiresCredentials: boolean;
  executionUsers: boolean;
  executionAgents: boolean;
  isSyntheticTrades: boolean;
  question: string;
  description: string;
  category: string;
  status: MarketStatus;
  yesPrice: number;
  noPrice: number;
  yesSupply: number;
  noSupply: number;
  volume24h: number;
  totalVolume: number;
  totalCollateral: number;
  feeBps: number;
  oracle: string;
  collateralMint: string;
  yesMint: string;
  noMint: string;
  resolutionDeadline: string;
  tradingEnd: string;
  resolvedOutcome?: Outcome;
  createdAt: string;
  resolvedAt?: string;
  outcomes?: MarketOutcome[];
  frequency?: MarketFrequency;
  imageUrl?: string;
}

export interface Order {
  id: string;
  orderId: number;
  marketId: string;
  owner: string;
  side: OrderSide;
  outcome: Outcome;
  orderType: OrderType;
  price: number;
  priceBps: number;
  quantity: number;
  filledQuantity: number;
  remainingQuantity: number;
  status: OrderStatus;
  isPrivate: boolean;
  txSignature?: string;
  createdAt: string;
  updatedAt: string;
  expiresAt?: string;
}

export interface Agent {
  id: string;
  owner: string;
  marketId: string;
  isYes: boolean;
  priceBps: number;
  size: string;
  cadence: number;
  expiryWindow: number;
  lastExecutedAt: string;
  nextExecutionAt: string;
  canExecute: boolean;
  active: boolean;
  status: AgentStatus;
  strategy: string;
  identityId?: string;
  identityTier?: number;
  identityActive?: boolean;
  identityUpdatedAt?: string;
  reputationScoreBps?: number;
  reputationConfidenceBps?: number;
  reputationEvents?: number;
  reputationNotionalMicrousdc?: string;
}

export interface Position {
  marketId: string;
  marketQuestion: string;
  owner: string;
  yesBalance: number;
  noBalance: number;
  claimable: number;
  avgYesCost: number;
  avgNoCost: number;
  currentYesPrice: number;
  currentNoPrice: number;
  unrealizedPnl: number;
  realizedPnl: number;
  totalDeposited: number;
  totalWithdrawn: number;
  openOrderCount: number;
  totalTrades: number;
  createdAt: string;
}

export interface Trade {
  id: string;
  marketId: string;
  outcome: Outcome;
  price: number;
  quantity: number;
  buyer: string;
  seller: string;
  txSignature: string;
  createdAt: string;
}

export interface OrderBookLevel {
  price: number;
  quantity: number;
  orders: number;
}

export interface OrderBook {
  marketId: string;
  outcome: Outcome;
  bids: OrderBookLevel[];
  asks: OrderBookLevel[];
  lastUpdated: string;
}

export interface User {
  wallet: string;
  username?: string;
  createdAt: string;
  stats: UserStats;
  settings: UserSettings;
}

export interface UserStats {
  totalTrades: number;
  totalVolume: number;
  winRate: number;
  pnl30d: number;
  pnlAllTime: number;
}

export interface UserSettings {
  defaultPrivacyMode: string;
  notificationsEnabled: boolean;
}

export interface Transaction {
  id: string;
  owner: string;
  txType: TransactionType;
  marketId?: string;
  amount: number;
  fee: number;
  txSignature?: string;
  status: string;
  createdAt: string;
}

// API Request/Response types
export interface PlaceOrderRequest {
  marketId: string;
  side: OrderSide;
  outcome: Outcome;
  orderType: OrderType;
  price?: number;
  quantity: number;
  expiresIn?: number;
  isPrivate?: boolean;
}

export interface PlaceOrderResponse {
  orderId: string;
  status: string;
  txSignature?: string;
}

export interface CancelOrderResponse {
  success: boolean;
  txSignature?: string;
}

export interface ClaimWinningsResponse {
  marketId: string;
  claimedAmount: number;
  winningOutcome: Outcome;
  winningTokensBurned: number;
  txSignature: string;
}

export interface AuthTokens {
  accessToken: string;
  refreshToken: string;
  expiresIn: number;
}

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  limit: number;
  offset: number;
  hasMore: boolean;
}

export interface MarketFilters {
  source?: MarketSource;
  tradable?: TradableFilter;
  includeLowLiquidity?: boolean;
  status?: MarketStatus;
  category?: string;
  limit?: number;
  offset?: number;
  sort?: 'volume' | 'newest' | 'ending';
  order?: 'asc' | 'desc';
}

export interface OrderFilters {
  marketId?: string;
  status?: OrderStatus;
  limit?: number;
  offset?: number;
}

export interface AgentFilters {
  owner?: string;
  marketId?: string;
  active?: boolean;
  limit?: number;
  offset?: number;
}

// Wallet types
export type DepositSource = 'wallet';
export type WalletWriteMode = 'prepare' | 'relay' | 'confirm';

export interface PreparedWalletTransaction {
  step: string;
  to: string;
  data: `0x${string}`;
  value: `0x${string}`;
}

export interface WalletBalance {
  available: number;
  locked: number;
  claimable: number;
  total: number;
  pendingDeposits: number;
  pendingWithdrawals: number;
  sourceBlock: number;
}

export interface DepositAddress {
  address: string;
  mint: string;
  memoRequired: boolean;
  memoFormat: string;
  network: string;
  minimumAmount: number;
}

export interface DepositRequest {
  amount: number;
  mode?: WalletWriteMode;
  intentId?: string;
  rawTx?: string;
  txSignature?: string;
  source?: DepositSource;
}

export interface DepositResponse {
  transactionId: string;
  status: string;
  phase: string;
  amount: number;
  depositAddress?: string;
  intentId?: string;
  preparedTransactions?: PreparedWalletTransaction[];
  txSignature?: string;
}

export interface WithdrawRequest {
  amount: number;
  mode?: WalletWriteMode;
  intentId?: string;
  rawTx?: string;
  destination?: string;
  txSignature?: string;
}

export interface WithdrawResponse {
  transactionId: string;
  status: string;
  phase: string;
  amount: number;
  fee: number;
  netAmount: number;
  estimatedCompletion: string;
  intentId?: string;
  preparedTransactions?: PreparedWalletTransaction[];
  txSignature?: string;
}

// Notification types
export type NotificationType =
  | 'order_filled'
  | 'order_cancelled'
  | 'market_resolved'
  | 'position_liquidated'
  | 'deposit_confirmed'
  | 'withdrawal_completed'
  | 'price_alert'
  | 'decision_recommendation_changed'
  | 'decision_threshold_crossed'
  | 'decision_confidence_dropped'
  | 'system';

export interface Notification {
  id: string;
  type: NotificationType;
  title: string;
  message: string;
  read: boolean;
  marketId?: string;
  orderId?: string;
  decisionCellId?: string;
  metadata?: Record<string, unknown>;
  createdAt: string;
}

export interface NotificationPreferences {
  orderFills: boolean;
  marketResolutions: boolean;
  priceAlerts: boolean;
  systemAnnouncements: boolean;
  decisionAlerts: boolean;
  emailNotifications: boolean;
  pushNotifications: boolean;
}

export type DecisionType = 'timing' | 'choice' | 'hedge' | 'allocation';
export type DecisionNodeSourceType = 'internal_market' | 'external_market' | 'draft_market';
export type DecisionNodeEffect = 'support' | 'oppose' | 'neutral';
export type DecisionTriggerMode =
  | 'on_recommendation_gain'
  | 'on_threshold_cross'
  | 'on_confidence_gain';

export interface DecisionActionScore {
  actionId: string;
  label: string;
  rank: number;
  scoreBps: number;
}
