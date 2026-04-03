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
  liquidityMode?: 'clob_only' | 'bootstrap_hybrid';
  bootstrapStatus?: string;
  bootstrapActive?: boolean;
  bootstrapSeedUsdc?: number;
  bootstrapManager?: string;
  bootstrapPreset?: 'tight' | 'balanced' | 'wide';
  bootstrapStrategy?: string;
  bootstrapLevels?: number;
  bootstrapInitialYesBps?: number;
  bootstrapBaseSpreadBps?: number;
  bootstrapStepBps?: number;
  bootstrapCadenceSeconds?: number;
  bootstrapExpirySeconds?: number;
  bootstrapPauseReason?: string;
  bootstrapReservedUsdc?: number;
  bootstrapAvailableUsdc?: number;
  bootstrapActiveSlots?: number;
  bootstrapOrganicDepthRatio?: number;
  bootstrapConsecutiveFailures?: number;
  bootstrapGraduatedAt?: string;
  bootstrapLaunchTxHash?: string;
  bootstrapLastReconciledAt?: string;
  bootstrapLastError?: string;
  bootstrapInventoryYesUsdc?: number;
  bootstrapInventoryNoUsdc?: number;
  bootstrapInventoryTotalUsdc?: number;
  bootstrapInventoryNetUsdc?: number;
  mirrorLinkCount?: number;
  mirrorActiveLinkCount?: number;
  mirrorLastMirrorAt?: string;
  mirrorLastHedgeAt?: string;
  mirrorFreshnessSeconds?: number;
  mirrorPendingHedges?: number;
  mirrorLinksWithErrors?: number;
  mirrorHedgeErrors?: number;
  mirrorTotalMirroredUsdc?: number;
  mirrorTotalHedgedUsdc?: number;
  mirrorNetExposureUsdc?: number;
  tradabilityScore?: number;
  // Oracle resolver fields
  oracleFeedType?: 'chainlink' | 'manual';
  oracleFeedAddress?: string;
  oracleComparison?: 'gt' | 'gte' | 'lt' | 'lte' | 'eq';
  oracleTargetValue?: number;
  oracleTargetCurrency?: string;
  oracleKeeperEnabled?: boolean;
  oracleResolvedAt?: string;
  oracleConfigureTx?: string;
  oracleResolveTx?: string;
  // KYC tier requirement
  requiredKycTier?: number;
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
  manager?: string;
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
  provider?: string;
  providerMarketRef?: string;
  chainId?: number;
  isSynthetic?: boolean;
  blockNumber?: number;
  createdAt: string;
}

export type CreatorChartRange = '7d' | '30d' | '90d';

export interface CreatorEconomicsOverview {
  creator: string;
  activeSeededMarkets: number;
  totalSeedDeployedUsdc: number;
  currentCapitalValueUsdc: number;
  netLiquidityPnlUsdc: number;
  subsidyBurnUsdc: number;
  realizedResolutionPnlUsdc: number;
  graduationSuccessRate: number;
  staleErrorMirrorCount: number;
}

export interface CreatorEconomicsMarketSummary {
  marketId: string;
  marketQuestion: string;
  status: string;
  liquidityMode: string;
  bootstrapStatus: string;
  seedUsdc: number;
  reservedBudgetUsdc: number;
  availableBudgetUsdc: number;
  inventoryYesUsdc: number;
  inventoryNoUsdc: number;
  inventoryNetUsdc: number;
  currentCapitalValueUsdc: number;
  cumulativeBootstrapFillsUsdc: number;
  subsidyBurnUsdc: number;
  netLiquidityPnlUsdc: number;
  roiBps: number;
  organicReplacementRatio: number;
  graduationState: string;
  graduationReason?: string;
  mirrorFreshnessSeconds?: number;
  mirrorPendingHedges: number;
  mirrorErrorCount: number;
  mirrorLinksWithErrors: number;
  realizedResolutionPnlUsdc: number;
  graduatedAt?: string;
  lastReconciledAt?: string;
}

export interface CreatorEconomicsPoint {
  day: string;
  cumulativeBootstrapFillsUsdc: number;
  subsidyBurnUsdc: number;
  inventoryMarkValueUsdc: number;
  organicReplacementRatio: number;
  mirrorFreshnessSeconds?: number;
  mirrorPendingHedges: number;
  mirrorErrorCount: number;
  graduationRetention24h?: number;
  graduationRetention7d?: number;
}

export interface CreatorEconomicsMarketDetail extends CreatorEconomicsMarketSummary {
  points: CreatorEconomicsPoint[];
  window: CreatorChartRange;
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
  includesBootstrap?: boolean;
  includesMirror?: boolean;
  bootstrapDepth?: number;
  organicDepth?: number;
  mirrorDepth?: number;
  bootstrapInventoryYesUsdc?: number;
  bootstrapInventoryNoUsdc?: number;
  bootstrapInventoryTotalUsdc?: number;
  bootstrapInventoryNetUsdc?: number;
  mirrorLinkCount?: number;
  mirrorActiveLinkCount?: number;
  mirrorLastMirrorAt?: string;
  mirrorLastHedgeAt?: string;
  mirrorFreshnessSeconds?: number;
  mirrorPendingHedges?: number;
  mirrorLinksWithErrors?: number;
  mirrorHedgeErrors?: number;
  mirrorTotalMirroredUsdc?: number;
  mirrorTotalHedgedUsdc?: number;
  mirrorNetExposureUsdc?: number;
  tradabilityScore?: number;
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

export interface BootstrapOperatorStatus {
  operator: string;
  owner: string;
  approved: boolean;
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

export interface DecisionContributor {
  nodeId: string;
  label: string;
  actionLabel: string;
  scoreBps: number;
  probabilityBps: number;
  deltaBps?: number;
  sourceRef?: string;
}

export interface DecisionRecommendation {
  state: string;
  confidenceBps: number;
  whyChanged: string;
  liveNodes: number;
  totalNodes: number;
  topActionLeadBps: number;
  actionScores: DecisionActionScore[];
  topContributors: DecisionContributor[];
  lastChangedNode?: DecisionContributor;
}

export interface DecisionAction {
  id: string;
  label: string;
  rank: number;
  scoreBps: number;
}

export interface DecisionNodeAgent {
  id: string;
  externalAgentId: string;
  triggerMode: DecisionTriggerMode;
  active: boolean;
  name?: string;
  provider?: 'limitless' | 'polymarket';
  agentActive?: boolean;
}

export interface DecisionNode {
  id: string;
  label: string;
  description: string;
  weightBps: number;
  sourceType: DecisionNodeSourceType;
  sourceRef?: string;
  status: string;
  lastProbabilityBps?: number;
  lastMarketSnapshot: Record<string, unknown>;
  actionEffects: Record<string, DecisionNodeEffect>;
  createdAt: string;
  updatedAt: string;
  agents: DecisionNodeAgent[];
}

export interface DecisionAlert {
  id: string;
  kind: string;
  threshold: Record<string, unknown>;
  active: boolean;
  lastTriggeredAt?: string;
}

export interface DecisionAutomationPolicy {
  automationEnabled: boolean;
  maxAgentNotionalUsdc: number;
  maxTriggersPerDay: number;
  minTriggerIntervalSeconds: number;
  allowedProvider?: 'limitless' | 'polymarket';
  requireConfidenceBps: number;
  active: boolean;
}

export interface DecisionEvent {
  id: string;
  nodeId?: string;
  kind: string;
  payload: Record<string, unknown>;
  createdAt: string;
}

export interface DecisionCellListItem {
  id: string;
  title: string;
  statement: string;
  decisionType: DecisionType;
  horizonAt?: string;
  status: string;
  automationEnabled: boolean;
  linkedMarketRefs: string[];
  recommendation: DecisionRecommendation;
  createdAt: string;
  updatedAt: string;
}

export interface DecisionCell {
  id: string;
  owner: string;
  title: string;
  statement: string;
  decisionType: DecisionType;
  horizonAt?: string;
  status: string;
  automationEnabled: boolean;
  recommendation: DecisionRecommendation;
  actions: DecisionAction[];
  nodes: DecisionNode[];
  alerts: DecisionAlert[];
  automationPolicy: DecisionAutomationPolicy;
  events: DecisionEvent[];
  createdAt: string;
  updatedAt: string;
}

// Leaderboard types
export type LeaderboardPeriod = 'daily' | 'weekly' | 'monthly' | 'all_time';
export type LeaderboardMetric = 'pnl' | 'volume' | 'trades' | 'win_rate';

export interface LeaderboardEntry {
  rank: number;
  wallet: string;
  username?: string;
  value: number;
  change?: number;
  previousRank?: number;
}

export interface Leaderboard {
  period: LeaderboardPeriod;
  metric: LeaderboardMetric;
  entries: LeaderboardEntry[];
  updatedAt: string;
}

// Public profile types
export interface PublicProfile {
  wallet: string;
  username?: string;
  bio?: string;
  avatarUrl?: string;
  joinedAt: string;
  stats: PublicProfileStats;
  badges: ProfileBadge[];
}

export interface PublicProfileStats {
  totalTrades: number;
  totalVolume: number;
  winRate: number;
  pnl30d: number;
  pnlAllTime: number;
  marketsTraded: number;
  bestTrade: number;
  worstTrade: number;
  currentStreak: number;
  longestStreak: number;
}

export interface ProfileBadge {
  id: string;
  name: string;
  description: string;
  icon: string;
  earnedAt: string;
}

export interface ProfileActivity {
  id: string;
  type: 'trade' | 'position_opened' | 'position_closed' | 'market_resolved';
  marketId: string;
  marketQuestion: string;
  outcome?: Outcome;
  amount?: number;
  pnl?: number;
  createdAt: string;
}

// Hackathon types

export type HackathonStatus = 'upcoming' | 'active' | 'completed' | 'cancelled';

export interface Hackathon {
  id: string;
  name: string;
  description: string;
  prizePoolUsdc: number;
  startTime: string;
  endTime: string;
  status: HackathonStatus;
  scoringMethod: string;
  createdBy: string;
  rulesJson: Record<string, unknown>;
  participantCount: number;
  agentCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface HackathonRegistration {
  hackathonId: string;
  walletAddress: string;
  identityId?: string;
  registeredAt: string;
  status: string;
  agentCount: number;
}

export interface HackathonLeaderboardEntry {
  rank: number;
  walletAddress: string;
  netPnlUsdc: number;
  totalVolumeUsdc: number;
  winRateBps: number;
  positionCount: number;
  tradeCount: number;
  snapshotTime: string;
}

export interface HackathonLeaderboard {
  hackathonId: string;
  entries: HackathonLeaderboardEntry[];
  updatedAt: string | null;
  total: number;
}

export interface HackathonSnapshot {
  walletAddress: string;
  netPnlUsdc: number;
  totalVolumeUsdc: number;
  rank: number;
  snapshotTime: string;
}

// KYC types

export type KycTier = 0 | 2 | 3;
export type KycProvider = 'world_id' | 'persona' | null;

export interface KycStatus {
  tier: KycTier;
  tierLabel: 'Unverified' | 'Verified' | 'Institutional';
  provider: KycProvider;
  verifiedAt: string | null;
}

// Oracle config types

export type OracleFeedType = 'chainlink' | 'manual';
export type OracleComparison = 'gt' | 'gte' | 'lt' | 'lte' | 'eq';

export interface OracleConfig {
  feedType: OracleFeedType;
  feedAddress: string;
  comparison: OracleComparison;
  targetValue: number;
  targetCurrency: string;
  category?: string;
  resolutionHint?: string;
  keeperEnabled: boolean;
}

export interface OracleMarketConfig extends OracleConfig {
  marketId: number;
  configureTx?: string;
  resolveTx?: string;
  resolvedAt?: string;
  lastCheckedAt?: string;
  lastError?: string;
}

// Social types

export interface TraderFollow {
  follower: string;
  following: string;
  createdAt: string;
}

export interface FollowerCounts {
  followersCount: number;
  followingCount: number;
}

export interface MarketComment {
  id: string;
  marketId: string;
  wallet: string;
  text: string;
  parentId?: string;
  farcasterHash?: string;
  username?: string;
  avatarUrl?: string;
  createdAt: string;
}

export interface CopyTradingSubscription {
  id: string;
  subscriber: string;
  targetWallet: string;
  agentId?: string;
  allocationUsdc: number;
  maxPositionUsdc: number;
  active: boolean;
  createdAt: string;
}

export type SignalDirection = 'yes' | 'no' | 'neutral';

export interface TradingSignal {
  id: string;
  publisher: string;
  marketId: string;
  direction: SignalDirection;
  confidenceBps: number;
  rationale?: string;
  validUntil: string;
  isAgent: boolean;
  agentId?: string;
  subscriberCount: number;
  createdAt: string;
  resolvedAt?: string;
  outcomeCorrect?: boolean;
}
