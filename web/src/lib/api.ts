import type {
  Agent,
  AgentFilters,
  Market,
  Order,
  Position,
  OrderBook,
  Trade,
  User,
  Transaction,
  PlaceOrderRequest,
  PlaceOrderResponse,
  CancelOrderResponse,
  ClaimWinningsResponse,
  PaginatedResponse,
  MarketFilters,
  OrderFilters,
  Outcome,
  WalletBalance,
  DepositAddress,
  DepositRequest,
  DepositResponse,
  WithdrawRequest,
  WithdrawResponse,
  DecisionCell,
  DecisionCellListItem,
  DecisionNodeEffect,
  DecisionNodeSourceType,
  DecisionTriggerMode,
  Notification,
  NotificationPreferences,
  Leaderboard,
  LeaderboardPeriod,
  LeaderboardMetric,
  PublicProfile,
  ProfileActivity,
} from '@/types';
import { CURATED_MARKETS_BY_ID } from '@/lib/curatedMarkets';
import {
  isReadOnlyMode,
  readOnlyPreviewEnabled,
  setRuntimeCapabilities,
} from '@/lib/runtimeMode';

const PRIMARY_API_BASE =
  process.env.NEXT_PUBLIC_API_PROXY_URL?.trim() ||
  process.env.NEXT_PUBLIC_API_URL?.trim() ||
  '/api/proxy';
const FALLBACK_API_BASE = process.env.NEXT_PUBLIC_API_FALLBACK_URL?.trim() || '';

export interface BaseTokenState {
  chain_id: number;
  token_address: string;
  total_supply_hex: string;
  decimals: number;
}

export interface BaseValidationStatus {
  request_hash: string;
  validator: string;
  agent_id: string;
  response: number;
  response_hash: string;
  tag: string;
  last_update: number;
  responded: boolean;
  source: string;
}

export interface BaseMarketSnapshot {
  id: string;
  question_hash: string;
  question: string;
  description: string;
  category: string;
  resolution_source: string;
  resolver: string;
  close_time: number;
  resolve_time: number;
  resolved: boolean;
  outcome?: 'yes' | 'no' | null;
  status: string;
  source?: string;
  provider?: string;
  is_external?: boolean;
  external_url?: string | null;
  chain_id?: number;
  requires_credentials?: boolean;
  execution_users?: boolean;
  execution_agents?: boolean;
  outcomes?: Array<{ label: string; probability: number }>;
}

export interface BaseMarketsResponse {
  markets: BaseMarketSnapshot[];
  total: number;
  limit: number;
  offset: number;
}

interface BaseOrderBookLevel {
  price: number;
  quantity: number;
  orders: number;
}

interface BaseOrderBookResponse {
  market_id: string;
  outcome: 'yes' | 'no';
  bids: BaseOrderBookLevel[];
  asks: BaseOrderBookLevel[];
  last_updated: string;
  provider?: string;
  chain_id?: number;
  provider_market_ref?: string;
  is_synthetic?: boolean;
}

interface BaseTradeSnapshot {
  id: string;
  market_id: string;
  outcome: 'yes' | 'no';
  price: number;
  price_bps: number;
  quantity: number;
  tx_hash: string;
  block_number: number;
  created_at: string;
}

interface BaseTradesResponse {
  trades: BaseTradeSnapshot[];
  total: number;
  limit: number;
  offset: number;
  has_more: boolean;
  provider?: string;
  chain_id?: number;
  provider_market_ref?: string;
  is_synthetic?: boolean;
}

interface BaseAgentSnapshot {
  id: string;
  owner: string;
  market_id: string;
  is_yes: boolean;
  price_bps: number;
  size: string;
  cadence: number;
  expiry_window: number;
  last_executed_at: number;
  next_execution_at: number;
  can_execute: boolean;
  active: boolean;
  status: Agent['status'];
  strategy: string;
  identity_id?: string;
  identity_tier?: number;
  identity_active?: boolean;
  identity_updated_at?: number;
  reputation_score_bps?: number;
  reputation_confidence_bps?: number;
  reputation_events?: number;
  reputation_notional_microusdc?: string;
}

interface BaseAgentsResponse {
  agents: BaseAgentSnapshot[];
  total: number;
  limit: number;
  offset: number;
}

export interface ExternalCredential {
  id: string;
  provider: 'limitless' | 'polymarket';
  label: string;
  key_id: string;
  created_at: string;
  updated_at: string;
  credentials: Record<string, unknown>;
}

export interface ExternalCredentialCheck {
  code: string;
  ok: boolean;
  message: string;
}

export interface ExternalCredentialStatus {
  provider: 'limitless' | 'polymarket';
  credential_id?: string | null;
  ready: boolean;
  base_wallet?: string | null;
  profile_status?: string | null;
  checks: ExternalCredentialCheck[];
}

interface ExternalCredentialsListResponse {
  credentials: ExternalCredential[];
  total: number;
}

export interface ExternalOrderIntent {
  id: string;
  provider: 'limitless' | 'polymarket';
  market_id: string;
  preflight: Record<string, unknown>;
  typed_data?: Record<string, unknown>;
  typedData?: Record<string, unknown>;
  status: string;
  expires_at: string;
}

export interface ExternalOrderRecord {
  id: string;
  provider: 'limitless' | 'polymarket';
  market_id: string;
  provider_order_id: string;
  status: string;
  created_at: string;
  updated_at: string;
  response_payload: Record<string, unknown>;
  error_message?: string | null;
}

interface ExternalOrdersListResponse {
  orders: ExternalOrderRecord[];
  total: number;
  limit: number;
  offset: number;
}

export interface ExternalAgentRecord {
  id: string;
  owner: string;
  name: string;
  provider: 'limitless' | 'polymarket';
  market_id: string;
  outcome: 'yes' | 'no';
  side: 'buy' | 'sell';
  price: number;
  quantity: number;
  cadence_seconds: number;
  strategy: string;
  credential_id?: string | null;
  active: boolean;
  last_executed_at?: string | null;
  next_execution_at: string;
  created_at: string;
  updated_at: string;
}

interface ExternalAgentsListResponse {
  agents: ExternalAgentRecord[];
  total: number;
  limit: number;
  offset: number;
}

interface DecisionCellsListResponse {
  data: DecisionCellListItem[];
  total: number;
  limit: number;
  offset: number;
  has_more: boolean;
}

interface DecisionEventsResponse {
  data: Array<{
    id: string;
    nodeId?: string;
    kind: string;
    payload: Record<string, unknown>;
    createdAt: string;
  }>;
}

export interface CreateDecisionCellRequest {
  title: string;
  statement: string;
  decisionType: 'timing' | 'choice' | 'hedge' | 'allocation';
  horizonAt?: string;
  actions?: string[];
}

export interface UpdateDecisionCellRequest {
  title?: string;
  statement?: string;
  horizonAt?: string;
  status?: string;
  automationEnabled?: boolean;
}

export interface CreateDecisionNodeRequest {
  label: string;
  description?: string;
  weightBps?: number;
  sourceType?: DecisionNodeSourceType;
  sourceRef?: string;
  status?: string;
  actionEffects?: Record<string, DecisionNodeEffect>;
}

export interface UpdateDecisionNodeRequest {
  label?: string;
  description?: string;
  weightBps?: number;
  sourceType?: DecisionNodeSourceType;
  sourceRef?: string;
  status?: string;
  actionEffects?: Record<string, DecisionNodeEffect>;
}

export interface UpdateDecisionAutomationRequest {
  automationEnabled?: boolean;
  maxAgentNotionalUsdc?: number;
  maxTriggersPerDay?: number;
  minTriggerIntervalSeconds?: number;
  allowedProvider?: 'limitless' | 'polymarket';
  requireConfidenceBps?: number;
  active?: boolean;
}

export interface Web4Capabilities {
  project: string;
  mode: string;
  chain_mode: string;
  api_base: string;
  runtime: {
    evm_reads_enabled: boolean;
    evm_writes_enabled: boolean;
    solana_reads_enabled: boolean;
    solana_writes_enabled: boolean;
    external_markets_enabled: boolean;
    external_trading_enabled: boolean;
    external_agents_enabled: boolean;
    limitless_enabled: boolean;
    polymarket_enabled: boolean;
  };
  wallet?: {
    read_enabled: boolean;
    deposit_enabled: boolean;
    withdraw_enabled: boolean;
    claim_enabled: boolean;
    deposit_mode: 'chain' | 'disabled';
    withdraw_mode: 'chain' | 'disabled';
  };
  launch?: {
    beta: boolean;
    limitless_trading_ready: boolean;
    polymarket_trading_ready: boolean;
  };
}

export interface PreparedEvmWriteTx {
  chain_id: number;
  from?: string;
  to: string;
  data: `0x${string}`;
  value: `0x${string}`;
  method: string;
}

export interface RelayRawTxResponse {
  chain_id: number;
  tx_hash: string;
}

export class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message);
    this.name = 'ApiError';
  }
}

function toNumber(value: unknown, fallback = 0): number {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return fallback;
}

function toIsoString(value: unknown): string {
  if (typeof value === 'string' && value.length > 0) return value;
  return new Date().toISOString();
}

function fromUnixSeconds(value: number | undefined): string {
  if (!value || !Number.isFinite(value) || value <= 0) {
    return new Date().toISOString();
  }
  return new Date(value * 1000).toISOString();
}

function fromUnixSecondsOptional(value: number | undefined): string {
  if (!value || !Number.isFinite(value) || value <= 0) {
    return '';
  }
  return new Date(value * 1000).toISOString();
}

function normalizeMarketStatus(value: unknown): Market['status'] {
  if (
    value === 'active' ||
    value === 'paused' ||
    value === 'closed' ||
    value === 'resolved' ||
    value === 'cancelled'
  ) {
    return value;
  }
  return 'active';
}

function normalizeMarket(raw: Record<string, unknown>): Market {
  const yesPrice = toNumber(raw.yesPrice ?? raw.yes_price, 0.5);
  const noPrice = toNumber(raw.noPrice ?? raw.no_price, 1 - yesPrice);
  const sourceRaw = String(raw.source ?? 'internal').toLowerCase();
  const source =
    sourceRaw === 'limitless' || sourceRaw === 'polymarket' || sourceRaw === 'all'
      ? sourceRaw
      : 'internal';

  return {
    id: String(raw.id ?? ''),
    address: String(raw.address ?? raw.id ?? ''),
    source,
    provider: String(raw.provider ?? 'internal'),
    isExternal: Boolean(raw.isExternal ?? raw.is_external ?? false),
    externalUrl: String(raw.externalUrl ?? raw.external_url ?? '') || undefined,
    chainId: toNumber(raw.chainId ?? raw.chain_id, 8453),
    requiresCredentials: Boolean(raw.requiresCredentials ?? raw.requires_credentials ?? false),
    executionUsers: Boolean(raw.executionUsers ?? raw.execution_users ?? true),
    executionAgents: Boolean(raw.executionAgents ?? raw.execution_agents ?? true),
    isSyntheticTrades: Boolean(raw.isSyntheticTrades ?? raw.is_synthetic_trades ?? false),
    question: String(raw.question ?? ''),
    description: String(raw.description ?? ''),
    category: String(raw.category ?? 'unknown'),
    status: normalizeMarketStatus(raw.status),
    yesPrice,
    noPrice,
    yesSupply: toNumber(raw.yesSupply ?? raw.yes_supply),
    noSupply: toNumber(raw.noSupply ?? raw.no_supply),
    volume24h: toNumber(raw.volume24h ?? raw.volume_24h),
    totalVolume: toNumber(raw.totalVolume ?? raw.total_volume),
    totalCollateral: toNumber(raw.totalCollateral ?? raw.total_collateral),
    feeBps: toNumber(raw.feeBps ?? raw.fee_bps),
    oracle: String(raw.oracle ?? ''),
    collateralMint: String(raw.collateralMint ?? raw.collateral_mint ?? ''),
    yesMint: String(raw.yesMint ?? raw.yes_mint ?? ''),
    noMint: String(raw.noMint ?? raw.no_mint ?? ''),
    resolutionDeadline: toIsoString(raw.resolutionDeadline ?? raw.resolution_deadline),
    tradingEnd: toIsoString(raw.tradingEnd ?? raw.trading_end),
    resolvedOutcome: (raw.resolvedOutcome ?? raw.resolved_outcome) as Market['resolvedOutcome'],
    createdAt: toIsoString(raw.createdAt ?? raw.created_at),
    resolvedAt: raw.resolvedAt || raw.resolved_at ? toIsoString(raw.resolvedAt ?? raw.resolved_at) : undefined,
  };
}

export function mapBaseSnapshotToMarket(snapshot: BaseMarketSnapshot): Market {
  const curated = CURATED_MARKETS_BY_ID[Number(snapshot.id)];
  const resolvedOutcome = snapshot.outcome === 'yes' || snapshot.outcome === 'no'
    ? snapshot.outcome
    : undefined;

  const yesPrice = resolvedOutcome === 'yes' ? 1 : resolvedOutcome === 'no' ? 0 : 0.5;
  const noPrice = 1 - yesPrice;

  const tradingEnd = fromUnixSeconds(snapshot.close_time);
  const resolutionDeadline = fromUnixSeconds(snapshot.resolve_time || snapshot.close_time);
  const question = snapshot.question?.trim() || curated?.question || `Base market #${snapshot.id}`;
  const description = snapshot.description?.trim()
    || (curated ? `Outcomes: ${curated.outcomes}. Context: ${curated.rationale}` : `Question hash: ${snapshot.question_hash}`);
  const category = snapshot.category?.trim() || curated?.category || 'base';
  const sourceRaw = String(snapshot.source || 'internal').toLowerCase();
  const source =
    sourceRaw === 'limitless' || sourceRaw === 'polymarket' || sourceRaw === 'all'
      ? sourceRaw
      : 'internal';

  return {
    id: snapshot.id,
    address: `base-market-${snapshot.id}`,
    source,
    provider: snapshot.provider || (snapshot.is_external ? source : 'internal'),
    isExternal: Boolean(snapshot.is_external),
    externalUrl: snapshot.external_url || undefined,
    chainId: toNumber(snapshot.chain_id, 8453),
    requiresCredentials: Boolean(snapshot.requires_credentials),
    executionUsers: snapshot.execution_users ?? true,
    executionAgents: snapshot.execution_agents ?? true,
    isSyntheticTrades: false,
    question,
    description,
    category,
    status: normalizeMarketStatus(snapshot.status),
    yesPrice,
    noPrice,
    yesSupply: 0,
    noSupply: 0,
    volume24h: 0,
    totalVolume: 0,
    totalCollateral: 0,
    feeBps: 0,
    oracle: snapshot.resolver,
    collateralMint: '',
    yesMint: '',
    noMint: '',
    resolutionDeadline,
    tradingEnd,
    resolvedOutcome,
    createdAt: tradingEnd,
    resolvedAt: snapshot.resolved ? fromUnixSeconds(snapshot.resolve_time) : undefined,
    outcomes: snapshot.outcomes && snapshot.outcomes.length > 0
      ? snapshot.outcomes
      : undefined,
  };
}

export function normalizeBaseMarketsResponse(
  response: BaseMarketsResponse
): PaginatedResponse<Market> {
  const data = response.markets.map(mapBaseSnapshotToMarket);
  const total = toNumber(response.total, data.length);
  const limit = toNumber(response.limit, data.length);
  const offset = toNumber(response.offset, 0);

  return {
    data,
    total,
    limit,
    offset,
    hasMore: offset + limit < total,
  };
}

function normalizeOutcome(value: unknown): Outcome {
  return value === 'no' ? 'no' : 'yes';
}

function normalizeTransactionType(value: unknown): Transaction['txType'] {
  switch (value) {
    case 'deposit':
    case 'withdraw':
    case 'buy':
    case 'sell':
    case 'claim':
    case 'mint':
    case 'redeem':
      return value;
    default:
      return 'deposit';
  }
}

function normalizeTransaction(raw: Record<string, unknown>): Transaction {
  const txSignature = String(raw.txSignature ?? raw.tx_signature ?? '');

  return {
    id: String(raw.id ?? ''),
    owner: String(raw.owner ?? ''),
    txType: normalizeTransactionType(raw.txType ?? raw.tx_type),
    marketId: raw.marketId ?? raw.market_id ? String(raw.marketId ?? raw.market_id) : undefined,
    amount: toNumber(raw.amount),
    fee: toNumber(raw.fee),
    txSignature: txSignature || undefined,
    status: String(raw.status ?? 'pending'),
    createdAt: toIsoString(raw.createdAt ?? raw.created_at),
  };
}

function normalizeTrade(raw: Record<string, unknown>): Trade {
  return {
    id: String(raw.id ?? ''),
    marketId: String(raw.marketId ?? raw.market_id ?? ''),
    outcome: normalizeOutcome(raw.outcome),
    price: toNumber(raw.price),
    quantity: toNumber(raw.quantity),
    buyer: String(raw.buyer ?? ''),
    seller: String(raw.seller ?? ''),
    txSignature: String(raw.txSignature ?? raw.tx_signature ?? raw.tx_hash ?? ''),
    createdAt: toIsoString(raw.createdAt ?? raw.created_at),
  };
}

function mapBaseTradeToTrade(snapshot: BaseTradeSnapshot): Trade {
  return {
    id: snapshot.id,
    marketId: snapshot.market_id,
    outcome: snapshot.outcome,
    price: snapshot.price,
    quantity: snapshot.quantity,
    buyer: '',
    seller: '',
    txSignature: snapshot.tx_hash,
    createdAt: snapshot.created_at,
  };
}

function mapBaseAgentToAgent(snapshot: BaseAgentSnapshot): Agent {
  return {
    id: snapshot.id,
    owner: snapshot.owner,
    marketId: snapshot.market_id,
    isYes: snapshot.is_yes,
    priceBps: toNumber(snapshot.price_bps),
    size: String(snapshot.size ?? '0'),
    cadence: toNumber(snapshot.cadence),
    expiryWindow: toNumber(snapshot.expiry_window),
    lastExecutedAt: fromUnixSecondsOptional(snapshot.last_executed_at),
    nextExecutionAt: fromUnixSecondsOptional(snapshot.next_execution_at),
    canExecute: Boolean(snapshot.can_execute),
    active: Boolean(snapshot.active),
    status: snapshot.status ?? 'inactive',
    strategy: String(snapshot.strategy ?? ''),
    identityId: snapshot.identity_id ? String(snapshot.identity_id) : undefined,
    identityTier: typeof snapshot.identity_tier === 'number' ? snapshot.identity_tier : undefined,
    identityActive: typeof snapshot.identity_active === 'boolean' ? snapshot.identity_active : undefined,
    identityUpdatedAt: typeof snapshot.identity_updated_at === 'number'
      ? fromUnixSecondsOptional(snapshot.identity_updated_at)
      : undefined,
    reputationScoreBps:
      typeof snapshot.reputation_score_bps === 'number' ? snapshot.reputation_score_bps : undefined,
    reputationConfidenceBps:
      typeof snapshot.reputation_confidence_bps === 'number'
        ? snapshot.reputation_confidence_bps
        : undefined,
    reputationEvents: typeof snapshot.reputation_events === 'number' ? snapshot.reputation_events : undefined,
    reputationNotionalMicrousdc: snapshot.reputation_notional_microusdc
      ? String(snapshot.reputation_notional_microusdc)
      : undefined,
  };
}

// Access token stored in memory only (XSS-safe)
// Refresh token stored in httpOnly cookie (handled by /api/auth)
class ApiClient {
  private accessToken: string | null = null;
  private tokenExpiresAt: number | null = null;
  private refreshPromise: Promise<void> | null = null;
  private capabilities: Web4Capabilities | null = null;
  private capabilitiesPromise: Promise<Web4Capabilities | null> | null = null;

  setAccessToken(accessToken: string, expiresAt?: number) {
    this.accessToken = accessToken;
    this.tokenExpiresAt = expiresAt || Date.now() + 15 * 60 * 1000; // Default 15 min
  }

  clearAccessToken() {
    this.accessToken = null;
    this.tokenExpiresAt = null;
  }

  private setCapabilities(capabilities: Web4Capabilities | null) {
    this.capabilities = capabilities;
    setRuntimeCapabilities(capabilities);
  }

  isAuthenticated(): boolean {
    return !!this.accessToken;
  }

  isTokenExpiringSoon(): boolean {
    if (!this.tokenExpiresAt) return true;
    // Refresh if less than 1 minute remaining
    return Date.now() > this.tokenExpiresAt - 60 * 1000;
  }

  // Check if we have a refresh token (httpOnly cookie)
  async checkSession(): Promise<boolean> {
    try {
      const res = await fetch('/api/auth', { method: 'GET' });
      const data = await res.json();
      return data.hasRefreshToken;
    } catch {
      return false;
    }
  }

  private async request<T>(
    path: string,
    options: RequestInit = {},
    skipRefresh = false,
    skipWriteGuard = false
  ): Promise<T> {
    const method = String(options.method || 'GET').toUpperCase();

    if (!skipWriteGuard) {
      await this.assertRequestWritable(method);
    }

    // Auto-refresh token if expiring soon
    if (!skipRefresh && this.accessToken && this.isTokenExpiringSoon()) {
      await this.refreshSession();
    }

    const headers: HeadersInit = {
      ...options.headers,
    };

    if (options.body !== undefined && !('Content-Type' in (headers as Record<string, string>))) {
      (headers as Record<string, string>)['Content-Type'] = 'application/json';
    }

    if (this.accessToken) {
      (headers as Record<string, string>)['Authorization'] = `Bearer ${this.accessToken}`;
    }
    const canUseFallback =
      method === 'GET' &&
      !this.accessToken &&
      !!FALLBACK_API_BASE &&
      FALLBACK_API_BASE !== PRIMARY_API_BASE;
    const requestOptions: RequestInit = {
      ...options,
      headers,
    };

    const fetchFromBase = async (base: string) => {
      return fetch(`${base}${path}`, requestOptions);
    };

    let res: Response;
    try {
      res = await fetchFromBase(PRIMARY_API_BASE);
    } catch (error) {
      if (!canUseFallback) {
        throw error;
      }
      res = await fetchFromBase(FALLBACK_API_BASE);
    }

    if (
      canUseFallback &&
      res.status >= 500 &&
      PRIMARY_API_BASE !== FALLBACK_API_BASE
    ) {
      try {
        const fallback = await fetchFromBase(FALLBACK_API_BASE);
        if (fallback.ok || fallback.status < 500) {
          res = fallback;
        }
      } catch {
        // Keep primary response path when fallback is unreachable.
      }
    }

    // Handle 401 by attempting token refresh
    if (res.status === 401 && !skipRefresh && this.accessToken) {
      try {
        await this.refreshSession();
        // Retry request with new token
        return this.request(path, options, true, skipWriteGuard);
      } catch {
        this.clearAccessToken();
        throw new ApiError(401, 'Session expired');
      }
    }

    if (!res.ok) {
      const text = await res.text();
      throw new ApiError(res.status, text || res.statusText);
    }

    if (res.status === 204) {
      return {} as T;
    }

    return res.json();
  }

  private async assertRequestWritable(method: string) {
    if (method === 'GET' || method === 'HEAD' || method === 'OPTIONS') {
      return;
    }

    if (readOnlyPreviewEnabled || isReadOnlyMode(this.capabilities)) {
      throw new ApiError(403, 'This action is disabled in read-only mode');
    }

    const capabilities = await this.loadCapabilities();
    if (isReadOnlyMode(capabilities)) {
      throw new ApiError(403, 'This action is disabled in read-only mode');
    }
  }

  private async loadCapabilities(): Promise<Web4Capabilities | null> {
    if (this.capabilities) {
      return this.capabilities;
    }

    if (this.capabilitiesPromise) {
      return this.capabilitiesPromise;
    }

    this.capabilitiesPromise = (async () => {
      try {
        const capabilities = await this.request<Web4Capabilities>(
          '/web4/capabilities',
          {},
          true,
          true
        );
        this.setCapabilities(capabilities);
        return capabilities;
      } catch {
        return null;
      } finally {
        this.capabilitiesPromise = null;
      }
    })();

    return this.capabilitiesPromise;
  }

  // Refresh access token using httpOnly cookie
  private async refreshSession(): Promise<void> {
    // Prevent concurrent refresh calls
    if (this.refreshPromise) {
      return this.refreshPromise;
    }

    this.refreshPromise = (async () => {
      try {
        const res = await fetch('/api/auth', { method: 'PUT' });
        if (!res.ok) {
          this.clearAccessToken();
          throw new Error('Refresh failed');
        }
        const data = await res.json();
        this.setAccessToken(data.accessToken, data.expiresAt);
      } finally {
        this.refreshPromise = null;
      }
    })();

    return this.refreshPromise;
  }

  private buildQuery(params: Record<string, unknown> | object): string {
    const filtered = Object.entries(params).filter(
      ([, v]) => v !== undefined && v !== null
    );
    if (filtered.length === 0) return '';
    return '?' + new URLSearchParams(
      filtered.map(([k, v]) => [k, String(v)])
    ).toString();
  }

  // Markets
  async getMarkets(filters?: MarketFilters): Promise<PaginatedResponse<Market>> {
    const query = this.buildQuery(filters || {});
    const response = await this.request<{
      markets?: Record<string, unknown>[];
      data?: Record<string, unknown>[];
      total?: number;
      limit?: number;
      offset?: number;
      hasMore?: boolean;
    }>(`/markets${query}`);

    const marketsRaw = response.markets ?? response.data ?? [];
    const data = marketsRaw.map((market) => normalizeMarket(market));
    const total = toNumber(response.total, data.length);
    const limit = toNumber(response.limit, data.length);
    const offset = toNumber(response.offset, 0);

    return {
      data,
      total,
      limit,
      offset,
      hasMore: response.hasMore ?? offset + limit < total,
    };
  }

  async getMarket(id: string): Promise<Market> {
    const response = await this.request<Record<string, unknown>>(`/markets/${id}`);
    return normalizeMarket(response);
  }

  async getOrderBook(
    marketId: string,
    outcome: Outcome,
    depth = 20
  ): Promise<OrderBook> {
    const response = await this.request<{
      marketId?: string;
      market_id?: string;
      outcome?: Outcome;
      bids?: BaseOrderBookLevel[];
      asks?: BaseOrderBookLevel[];
      timestamp?: string;
      lastUpdated?: string;
      last_updated?: string;
    }>(
      `/markets/${marketId}/orderbook?outcome=${outcome}&depth=${depth}`
    );

    return {
      marketId: String(response.marketId ?? response.market_id ?? marketId),
      outcome: response.outcome === 'no' ? 'no' : 'yes',
      bids: response.bids ?? [],
      asks: response.asks ?? [],
      lastUpdated: toIsoString(response.lastUpdated ?? response.last_updated ?? response.timestamp),
    };
  }

  async getTrades(
    marketId: string,
    params?: { outcome?: Outcome; limit?: number; before?: string }
  ): Promise<PaginatedResponse<Trade>> {
    return this.getBaseTrades(marketId, params);
  }

  // Orders
  async getOrders(filters?: OrderFilters): Promise<PaginatedResponse<Order>> {
    const query = this.buildQuery(filters || {});
    return this.request(`/orders${query}`);
  }

  async getOrder(orderId: string): Promise<Order> {
    return this.request(`/orders/${orderId}`);
  }

  async placeOrder(data: PlaceOrderRequest): Promise<PlaceOrderResponse> {
    return this.request('/orders', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async cancelOrder(orderId: string): Promise<CancelOrderResponse> {
    return this.request(`/orders/${orderId}`, {
      method: 'DELETE',
    });
  }

  // Positions
  async getPositions(): Promise<PaginatedResponse<Position>> {
    return this.request<PaginatedResponse<Position>>('/positions');
  }

  async getPosition(marketId: string): Promise<Position> {
    return this.request(`/positions/${marketId}`);
  }

  async claimWinnings(marketId: string, txSignature: string): Promise<ClaimWinningsResponse> {
    return this.request(`/positions/${marketId}/claim`, {
      method: 'POST',
      body: JSON.stringify({ txSignature }),
    });
  }

  // User
  async getProfile(): Promise<User> {
    return this.request('/user/profile');
  }

  async getTransactions(params?: {
    limit?: number;
    offset?: number;
    txType?: string;
  }): Promise<PaginatedResponse<Transaction>> {
    const query = this.buildQuery(params || {});
    const response = await this.request<{
      transactions?: Record<string, unknown>[];
      data?: Record<string, unknown>[];
      total?: number;
      limit?: number;
      offset?: number;
      hasMore?: boolean;
    }>(`/user/transactions${query}`);

    const data = (response.transactions ?? response.data ?? []).map((entry) =>
      normalizeTransaction(entry)
    );
    const total = toNumber(response.total, data.length);
    const limit = toNumber(response.limit, params?.limit ?? data.length);
    const offset = toNumber(response.offset, params?.offset ?? 0);

    return {
      data,
      total,
      limit,
      offset,
      hasMore: response.hasMore ?? offset + data.length < total,
    };
  }

  // Wallet
  async getWalletBalance(): Promise<WalletBalance> {
    return this.request('/wallet/balance');
  }

  async getWeb4Capabilities(): Promise<Web4Capabilities> {
    const capabilities = await this.request<Web4Capabilities>('/web4/capabilities');
    this.setCapabilities(capabilities);
    return capabilities;
  }

  async getBaseMarkets(params?: {
    limit?: number;
    offset?: number;
    source?: 'all' | 'internal' | 'limitless' | 'polymarket';
    tradable?: 'all' | 'user' | 'agent';
    includeLowLiquidity?: boolean;
  }): Promise<PaginatedResponse<Market>> {
    const query = this.buildQuery(params || {});
    const response = await this.request<BaseMarketsResponse>(`/evm/markets${query}`);
    return normalizeBaseMarketsResponse(response);
  }

  async getBaseOrderBook(
    marketId: string,
    outcome: Outcome,
    depth = 20
  ): Promise<OrderBook> {
    const query = this.buildQuery({ outcome, depth });
    const encodedMarketId = encodeURIComponent(marketId);
    const response = await this.request<BaseOrderBookResponse>(
      `/evm/markets/${encodedMarketId}/orderbook${query}`
    );

    return {
      marketId: response.market_id,
      outcome: response.outcome,
      bids: response.bids ?? [],
      asks: response.asks ?? [],
      lastUpdated: toIsoString(response.last_updated),
    };
  }

  async getBaseTrades(
    marketId: string,
    params?: { outcome?: Outcome; limit?: number; before?: string; offset?: number }
  ): Promise<PaginatedResponse<Trade>> {
    const query = this.buildQuery({
      outcome: params?.outcome,
      limit: params?.limit,
      offset: params?.offset,
    });
    const encodedMarketId = encodeURIComponent(marketId);
    const response = await this.request<BaseTradesResponse>(
      `/evm/markets/${encodedMarketId}/trades${query}`
    );
    const data = (response.trades ?? []).map(mapBaseTradeToTrade);
    const total = toNumber(response.total, data.length);
    const limit = toNumber(response.limit, params?.limit ?? data.length);
    const offset = toNumber(response.offset, params?.offset ?? 0);

    return {
      data,
      total,
      limit,
      offset,
      hasMore: response.has_more ?? offset + limit < total,
    };
  }

  async getBaseMarket(id: string): Promise<Market> {
    const response = await this.request<BaseMarketSnapshot>(`/evm/markets/${encodeURIComponent(id)}`);
    return mapBaseSnapshotToMarket(response);
  }

  async getBaseAgents(filters?: AgentFilters): Promise<PaginatedResponse<Agent>> {
    const query = this.buildQuery({
      limit: filters?.limit,
      offset: filters?.offset,
      owner: filters?.owner,
      market_id: filters?.marketId,
      active: filters?.active,
    });
    const response = await this.request<BaseAgentsResponse>(`/evm/agents${query}`);
    const data = (response.agents ?? []).map(mapBaseAgentToAgent);
    const total = toNumber(response.total, data.length);
    const limit = toNumber(response.limit, filters?.limit ?? data.length);
    const offset = toNumber(response.offset, filters?.offset ?? 0);

    return {
      data,
      total,
      limit,
      offset,
      hasMore: offset + limit < total,
    };
  }

  async getBaseAgent(id: string): Promise<Agent> {
    const parsedId = Number(id);
    if (!Number.isInteger(parsedId) || parsedId < 1) {
      throw new ApiError(404, 'Agent not found');
    }
    const response = await this.request<BaseAgentSnapshot>(`/evm/agents/${parsedId}`);
    return mapBaseAgentToAgent(response);
  }

  async getBaseTokenState(): Promise<BaseTokenState> {
    return this.request('/evm/token/state');
  }

  async getBaseValidationStatus(requestHash: string): Promise<BaseValidationStatus> {
    return this.request(`/evm/validation/${encodeURIComponent(requestHash)}`);
  }

  async getExternalCredentials(provider?: 'limitless' | 'polymarket'): Promise<ExternalCredential[]> {
    const query = this.buildQuery({ provider });
    const response = await this.request<ExternalCredentialsListResponse>(`/external/credentials${query}`);
    return response.credentials ?? [];
  }

  async getExternalCredentialStatus(
    provider: 'limitless' | 'polymarket',
    credentialId?: string
  ): Promise<ExternalCredentialStatus> {
    const query = this.buildQuery({ provider, credentialId });
    return this.request(`/external/credentials/status${query}`);
  }

  async upsertExternalCredential(data: {
    provider: 'limitless' | 'polymarket';
    label?: string;
    credentials: Record<string, unknown>;
  }): Promise<ExternalCredential> {
    return this.request('/external/credentials', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async deleteExternalCredential(credentialId: string): Promise<{ ok: boolean }> {
    return this.request(`/external/credentials/${credentialId}`, {
      method: 'DELETE',
    });
  }

  async bindLimitlessWallet(data: {
    credentialId: string;
    baseWallet: string;
  }): Promise<ExternalCredentialStatus> {
    return this.request('/external/credentials/limitless/wallet-bind', {
      method: 'POST',
      body: JSON.stringify({
        credentialId: data.credentialId,
        baseWallet: data.baseWallet,
      }),
    });
  }

  async createExternalOrderIntent(data: {
    provider: 'limitless' | 'polymarket';
    marketId: string;
    outcome: 'yes' | 'no';
    side: 'buy' | 'sell';
    price: number;
    quantity: number;
    credentialId?: string;
  }): Promise<ExternalOrderIntent> {
    return this.request('/external/orders/intent', {
      method: 'POST',
      body: JSON.stringify({
        provider: data.provider,
        marketId: data.marketId,
        outcome: data.outcome,
        side: data.side,
        price: data.price,
        quantity: data.quantity,
        credentialId: data.credentialId,
      }),
    });
  }

  async submitExternalOrder(data: {
    intentId: string;
    signedOrder: Record<string, unknown>;
    credentialId?: string;
  }): Promise<ExternalOrderRecord> {
    return this.request('/external/orders/submit', {
      method: 'POST',
      body: JSON.stringify({
        intentId: data.intentId,
        signedOrder: data.signedOrder,
        credentialId: data.credentialId,
      }),
    });
  }

  async cancelExternalOrder(data: {
    provider: 'limitless' | 'polymarket';
    providerOrderId: string;
    credentialId?: string;
    payload?: Record<string, unknown>;
  }): Promise<{ ok: boolean }> {
    return this.request('/external/orders/cancel', {
      method: 'POST',
      body: JSON.stringify({
        provider: data.provider,
        providerOrderId: data.providerOrderId,
        credentialId: data.credentialId,
        payload: data.payload,
      }),
    });
  }

  async listExternalOrders(params?: {
    provider?: 'limitless' | 'polymarket';
    limit?: number;
    offset?: number;
  }): Promise<ExternalOrdersListResponse> {
    const query = this.buildQuery(params || {});
    return this.request(`/external/orders${query}`);
  }

  async listExternalAgents(params?: {
    provider?: 'limitless' | 'polymarket';
    active?: boolean;
    limit?: number;
    offset?: number;
  }): Promise<ExternalAgentsListResponse> {
    const query = this.buildQuery(params || {});
    return this.request(`/external/agents${query}`);
  }

  async createExternalAgent(data: {
    name: string;
    provider: 'limitless' | 'polymarket';
    marketId: string;
    outcome: 'yes' | 'no';
    side: 'buy' | 'sell';
    price: number;
    quantity: number;
    cadenceSeconds: number;
    strategy: string;
    credentialId?: string;
    active?: boolean;
  }): Promise<ExternalAgentRecord> {
    return this.request('/external/agents', {
      method: 'POST',
      body: JSON.stringify({
        name: data.name,
        provider: data.provider,
        marketId: data.marketId,
        outcome: data.outcome,
        side: data.side,
        price: data.price,
        quantity: data.quantity,
        cadenceSeconds: data.cadenceSeconds,
        strategy: data.strategy,
        credentialId: data.credentialId,
        active: data.active,
      }),
    });
  }

  async updateExternalAgent(
    agentId: string,
    data: Partial<{
      name: string;
      outcome: 'yes' | 'no';
      side: 'buy' | 'sell';
      price: number;
      quantity: number;
      cadenceSeconds: number;
      strategy: string;
      credentialId: string;
      active: boolean;
    }>,
  ): Promise<ExternalAgentRecord> {
    return this.request(`/external/agents/${agentId}`, {
      method: 'PATCH',
      body: JSON.stringify(data),
    });
  }

  async executeExternalAgent(
    agentId: string,
    data?: { force?: boolean; signedOrder?: Record<string, unknown> },
  ): Promise<Record<string, unknown>> {
    return this.request(`/external/agents/${agentId}/execute`, {
      method: 'POST',
      body: JSON.stringify(data || {}),
    });
  }

  async listDecisionCells(params?: {
    limit?: number;
    offset?: number;
    status?: string;
  }): Promise<PaginatedResponse<DecisionCellListItem>> {
    const query = this.buildQuery(params || {});
    const response = await this.request<DecisionCellsListResponse>(`/decisions${query}`);
    return {
      data: response.data,
      total: response.total,
      limit: response.limit,
      offset: response.offset,
      hasMore: response.has_more,
    };
  }

  async createDecisionCell(data: CreateDecisionCellRequest): Promise<DecisionCell> {
    return this.request('/decisions', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async getDecisionCell(cellId: string): Promise<DecisionCell> {
    return this.request(`/decisions/${encodeURIComponent(cellId)}`);
  }

  async updateDecisionCell(cellId: string, data: UpdateDecisionCellRequest): Promise<DecisionCell> {
    return this.request(`/decisions/${encodeURIComponent(cellId)}`, {
      method: 'PATCH',
      body: JSON.stringify(data),
    });
  }

  async addDecisionAction(cellId: string, label: string): Promise<DecisionCell> {
    return this.request(`/decisions/${encodeURIComponent(cellId)}/actions`, {
      method: 'POST',
      body: JSON.stringify({ label }),
    });
  }

  async addDecisionNode(cellId: string, data: CreateDecisionNodeRequest): Promise<DecisionCell> {
    return this.request(`/decisions/${encodeURIComponent(cellId)}/nodes`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async updateDecisionNode(
    cellId: string,
    nodeId: string,
    data: UpdateDecisionNodeRequest,
  ): Promise<DecisionCell> {
    return this.request(
      `/decisions/${encodeURIComponent(cellId)}/nodes/${encodeURIComponent(nodeId)}`,
      {
        method: 'PATCH',
        body: JSON.stringify(data),
      },
    );
  }

  async attachDecisionMarket(
    cellId: string,
    nodeId: string,
    data: {
      sourceType: DecisionNodeSourceType;
      sourceRef: string;
    },
  ): Promise<DecisionCell> {
    return this.request(
      `/decisions/${encodeURIComponent(cellId)}/nodes/${encodeURIComponent(nodeId)}/attach-market`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      },
    );
  }

  async attachDecisionAgent(
    cellId: string,
    nodeId: string,
    data: {
      externalAgentId: string;
      triggerMode: DecisionTriggerMode;
      active?: boolean;
    },
  ): Promise<DecisionCell> {
    return this.request(
      `/decisions/${encodeURIComponent(cellId)}/nodes/${encodeURIComponent(nodeId)}/attach-agent`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      },
    );
  }

  async recalculateDecisionCell(cellId: string): Promise<DecisionCell> {
    return this.request(`/decisions/${encodeURIComponent(cellId)}/recalculate`, {
      method: 'POST',
      body: JSON.stringify({}),
    });
  }

  async updateDecisionAutomation(
    cellId: string,
    data: UpdateDecisionAutomationRequest,
  ): Promise<DecisionCell> {
    return this.request(`/decisions/${encodeURIComponent(cellId)}/automation`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async upsertDecisionAlert(
    cellId: string,
    data: {
      kind: string;
      threshold?: Record<string, unknown>;
      active?: boolean;
    },
  ): Promise<DecisionCell> {
    return this.request(`/decisions/${encodeURIComponent(cellId)}/alerts`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async getDecisionEvents(cellId: string): Promise<DecisionCell['events']> {
    const response = await this.request<DecisionEventsResponse>(
      `/decisions/${encodeURIComponent(cellId)}/events`,
    );
    return response.data;
  }

  async prepareBaseCreateMarket(data: {
    from?: string;
    question: string;
    description?: string;
    category?: string;
    resolutionSource?: string;
    closeTime: number;
    resolver: string;
  }): Promise<PreparedEvmWriteTx> {
    return this.request('/evm/write/markets/create', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async prepareBasePlaceOrder(data: {
    from?: string;
    marketId: number;
    outcome: Outcome;
    priceBps: number;
    size: string;
    expiry: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request('/evm/write/orders/place', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async prepareBaseCancelOrder(data: {
    from?: string;
    orderId: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request('/evm/write/orders/cancel', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async prepareBaseClaim(data: {
    from?: string;
    marketId: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request('/evm/write/positions/claim', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async prepareBaseClaimFor(data: {
    from?: string;
    user: string;
    marketId: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request('/evm/write/positions/claim-for', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async prepareBaseMatchOrders(data: {
    from?: string;
    firstOrderId: number;
    secondOrderId: number;
    fillSize: string;
  }): Promise<PreparedEvmWriteTx> {
    return this.request('/evm/write/orders/match', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async prepareBaseCreateAgent(data: {
    from?: string;
    marketId: number;
    isYes: boolean;
    priceBps: number;
    size: string;
    cadence: number;
    expiryWindow: number;
    strategy: string;
  }): Promise<PreparedEvmWriteTx> {
    return this.request('/evm/write/agents/create', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async prepareBaseExecuteAgent(data: {
    from?: string;
    agentId: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request('/evm/write/agents/execute', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async prepareBaseRegisterIdentity(data: {
    from?: string;
    wallet: string;
    tier: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request('/evm/write/identity/register', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async prepareBaseSetIdentityTier(data: {
    from?: string;
    wallet: string;
    tier: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request('/evm/write/identity/tier', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async prepareBaseSetIdentityActive(data: {
    from?: string;
    wallet: string;
    active: boolean;
  }): Promise<PreparedEvmWriteTx> {
    return this.request('/evm/write/identity/active', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async prepareBaseSubmitReputationOutcome(data: {
    from?: string;
    wallet: string;
    success: boolean;
    notionalMicrousdc: string;
    confidenceWeightBps: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request('/evm/write/reputation/outcome', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async prepareBaseValidationRequest(data: {
    from?: string;
    validator: string;
    agentId: string;
    requestUri: string;
    requestHash?: string;
  }): Promise<PreparedEvmWriteTx> {
    return this.request('/evm/write/validation/request', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async prepareBaseValidationResponse(data: {
    from?: string;
    requestHash: string;
    response: number;
    responseUri: string;
    responseHash: string;
    tag: string;
  }): Promise<PreparedEvmWriteTx> {
    return this.request('/evm/write/validation/response', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async relayBaseRawTransaction(rawTx: string): Promise<RelayRawTxResponse> {
    return this.request('/evm/write/relay', {
      method: 'POST',
      body: JSON.stringify({ rawTx }),
    });
  }

  async getDepositAddress(): Promise<DepositAddress> {
    return this.request('/wallet/deposit/address');
  }

  async deposit(data: DepositRequest): Promise<DepositResponse> {
    return this.request('/wallet/deposit', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async withdraw(data: WithdrawRequest): Promise<WithdrawResponse> {
    return this.request('/wallet/withdraw', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  // Auth
  async getNonce(): Promise<string> {
    return this.getSiweNonce();
  }

  async getSiweNonce(): Promise<string> {
    const res = await this.request<{ nonce: string }>('/auth/siwe/nonce', {}, true);
    return res.nonce;
  }

  async getSolanaNonce(): Promise<string> {
    const res = await this.request<{ nonce: string }>('/auth/solana/nonce', {}, true);
    return res.nonce;
  }

  async getFarcasterNonce(): Promise<string> {
    const res = await this.request<{ nonce: string }>('/auth/farcaster/nonce', {}, true);
    return res.nonce;
  }

  async login(
    wallet: string,
    signature: string,
    message: string
  ): Promise<{ accessToken: string; expiresAt: number }> {
    return this.loginSiwe(wallet, signature, message);
  }

  async loginSiwe(
    wallet: string,
    signature: string,
    message: string
  ): Promise<{ accessToken: string; expiresAt: number }> {
    const res = await fetch('/api/auth', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ wallet, signature, message, flow: 'siwe' }),
    });

    if (!res.ok) {
      const data = await res.json();
      throw new ApiError(res.status, data.error || 'SIWE login failed');
    }

    const data = await res.json();
    this.setAccessToken(data.accessToken, data.expiresAt);
    return data;
  }

  async loginSolana(
    wallet: string,
    signature: string,
    message: string
  ): Promise<{ accessToken: string; expiresAt: number }> {
    const res = await fetch('/api/auth', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ wallet, signature, message, flow: 'solana' }),
    });

    if (!res.ok) {
      const data = await res.json();
      throw new ApiError(res.status, data.error || 'Solana login failed');
    }

    const data = await res.json();
    this.setAccessToken(data.accessToken, data.expiresAt);
    return data;
  }

  async loginFarcaster(
    message: string,
    signature: string,
    nonce: string
  ): Promise<{ accessToken: string; expiresAt: number }> {
    const res = await fetch('/api/auth', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message, signature, nonce, flow: 'farcaster' }),
    });

    if (!res.ok) {
      const data = await res.json();
      throw new ApiError(res.status, data.error || 'Farcaster login failed');
    }

    const data = await res.json();
    this.setAccessToken(data.accessToken, data.expiresAt);
    return data;
  }

  async post<T = unknown>(path: string, body: Record<string, unknown>): Promise<T> {
    const res = await fetch(`${PRIMARY_API_BASE}${path}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...(this.accessToken ? { Authorization: `Bearer ${this.accessToken}` } : {}),
      },
      body: JSON.stringify(body),
    });

    if (!res.ok) {
      const data = await res.json().catch(() => ({}));
      throw new ApiError(res.status, (data as Record<string, string>).error || 'Request failed');
    }

    return res.json();
  }

  async refresh(): Promise<{ accessToken: string; expiresAt: number }> {
    const res = await fetch('/api/auth', { method: 'PUT' });

    if (!res.ok) {
      this.clearAccessToken();
      throw new ApiError(res.status, 'Token refresh failed');
    }

    const data = await res.json();
    this.setAccessToken(data.accessToken, data.expiresAt);
    return data;
  }

  async logout(): Promise<void> {
    try {
      await fetch('/api/auth', { method: 'DELETE' });
    } finally {
      this.clearAccessToken();
    }
  }

  // Restore session on page load (if refresh token exists)
  async restoreSession(): Promise<boolean> {
    const hasToken = await this.checkSession();
    if (hasToken) {
      try {
        await this.refresh();
        return true;
      } catch {
        return false;
      }
    }
    return false;
  }

  // Notifications
  async getNotifications(params?: {
    limit?: number;
    offset?: number;
    unreadOnly?: boolean;
  }): Promise<PaginatedResponse<Notification>> {
    const query = this.buildQuery(params || {});
    return this.request(`/notifications${query}`);
  }

  async getUnreadCount(): Promise<{ count: number }> {
    return this.request('/notifications/unread-count');
  }

  async markAsRead(notificationId: string): Promise<void> {
    return this.request(`/notifications/${notificationId}/read`, {
      method: 'PUT',
    });
  }

  async markAllAsRead(): Promise<void> {
    return this.request('/notifications/read-all', {
      method: 'PUT',
    });
  }

  async getNotificationPreferences(): Promise<NotificationPreferences> {
    return this.request('/notifications/preferences');
  }

  async updateNotificationPreferences(
    prefs: Partial<NotificationPreferences>
  ): Promise<NotificationPreferences> {
    return this.request('/notifications/preferences', {
      method: 'PUT',
      body: JSON.stringify(prefs),
    });
  }

  // Leaderboards
  async getLeaderboard(
    period: LeaderboardPeriod = 'weekly',
    metric: LeaderboardMetric = 'pnl',
    limit = 100
  ): Promise<Leaderboard> {
    return this.request(`/leaderboard?period=${period}&metric=${metric}&limit=${limit}`);
  }

  async getUserRank(
    wallet: string,
    period: LeaderboardPeriod = 'weekly',
    metric: LeaderboardMetric = 'pnl'
  ): Promise<{ rank: number; value: number }> {
    return this.request(`/leaderboard/rank/${wallet}?period=${period}&metric=${metric}`);
  }

  // Public profiles
  async getPublicProfile(wallet: string): Promise<PublicProfile> {
    return this.request(`/profiles/${wallet}`);
  }

  async getProfileActivity(
    wallet: string,
    params?: { limit?: number; offset?: number }
  ): Promise<PaginatedResponse<ProfileActivity>> {
    const query = this.buildQuery(params || {});
    return this.request(`/profiles/${wallet}/activity${query}`);
  }

  async getProfilePositions(wallet: string): Promise<PaginatedResponse<Position>> {
    return this.request(`/profiles/${wallet}/positions`);
  }
}

export const api = new ApiClient();

