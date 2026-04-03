import type { PaymentRequired } from "@x402/core/types";
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
  BootstrapOperatorStatus,
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
  Hackathon,
  HackathonRegistration,
  HackathonLeaderboard,
  HackathonSnapshot,
  CreatorChartRange,
  CreatorEconomicsOverview,
  CreatorEconomicsMarketSummary,
  CreatorEconomicsMarketDetail,
  CreatorEconomicsPoint,
} from "@/types";
import { CURATED_MARKETS_BY_ID } from "@/lib/curatedMarkets";
import {
  isReadOnlyMode,
  readOnlyPreviewEnabled,
  setRuntimeCapabilities,
} from "@/lib/runtimeMode";

const PRIMARY_API_BASE =
  process.env.NEXT_PUBLIC_API_PROXY_URL?.trim() ||
  process.env.NEXT_PUBLIC_API_URL?.trim() ||
  "/api/proxy";
const FALLBACK_API_BASE =
  process.env.NEXT_PUBLIC_API_FALLBACK_URL?.trim() || "";
const LOCAL_BASE_READ_API_BASE =
  process.env.NEXT_PUBLIC_LOCAL_BASE_READ_API_URL?.trim() || "";

export function resolveApiUrl(path: string): string {
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  return `${PRIMARY_API_BASE}${normalizedPath}`;
}

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
  outcome?: "yes" | "no" | null;
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
  yes_price?: number;
  no_price?: number;
  volume?: number;
  provider_market_ref?: string;
  liquidity_mode?: "clob_only" | "bootstrap_hybrid";
  bootstrap_status?: string;
  bootstrap_active?: boolean;
  bootstrap_seed_usdc?: number;
  bootstrap_manager?: string;
  bootstrap_preset?: "tight" | "balanced" | "wide";
  bootstrap_strategy?: string;
  bootstrap_levels?: number;
  bootstrap_initial_yes_bps?: number;
  bootstrap_base_spread_bps?: number;
  bootstrap_step_bps?: number;
  bootstrap_cadence_seconds?: number;
  bootstrap_expiry_seconds?: number;
  bootstrap_pause_reason?: string;
  bootstrap_reserved_usdc?: number;
  bootstrap_available_usdc?: number;
  bootstrap_active_slots?: number;
  bootstrap_organic_depth_ratio?: number;
  bootstrap_consecutive_failures?: number;
  bootstrap_graduated_at?: string;
  bootstrap_launch_tx_hash?: string;
  bootstrap_last_reconciled_at?: string;
  bootstrap_last_error?: string;
  bootstrap_inventory_yes_usdc?: number;
  bootstrap_inventory_no_usdc?: number;
  bootstrap_inventory_total_usdc?: number;
  bootstrap_inventory_net_usdc?: number;
  mirror_link_count?: number;
  mirror_active_link_count?: number;
  mirror_last_mirror_at?: string;
  mirror_last_hedge_at?: string;
  mirror_freshness_seconds?: number;
  mirror_pending_hedges?: number;
  mirror_links_with_errors?: number;
  mirror_hedge_errors?: number;
  mirror_total_mirrored_usdc?: number;
  mirror_total_hedged_usdc?: number;
  mirror_net_exposure_usdc?: number;
  tradability_score?: number;
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

export interface BaseOrderBookResponse {
  market_id: string;
  outcome: "yes" | "no";
  bids: BaseOrderBookLevel[];
  asks: BaseOrderBookLevel[];
  last_updated: string;
  provider?: string;
  chain_id?: number;
  provider_market_ref?: string;
  is_synthetic?: boolean;
  includes_bootstrap?: boolean;
  includes_mirror?: boolean;
  bootstrap_depth?: number;
  organic_depth?: number;
  mirror_depth?: number;
  bootstrap_inventory_yes_usdc?: number;
  bootstrap_inventory_no_usdc?: number;
  bootstrap_inventory_total_usdc?: number;
  bootstrap_inventory_net_usdc?: number;
  mirror_link_count?: number;
  mirror_active_link_count?: number;
  mirror_last_mirror_at?: string | null;
  mirror_last_hedge_at?: string | null;
  mirror_freshness_seconds?: number | null;
  mirror_pending_hedges?: number | null;
  mirror_links_with_errors?: number | null;
  mirror_hedge_errors?: number | null;
  mirror_total_mirrored_usdc?: number | null;
  mirror_total_hedged_usdc?: number | null;
  mirror_net_exposure_usdc?: number | null;
  tradability_score?: number;
}

interface BaseTradeSnapshot {
  id: string;
  market_id: string;
  outcome: "yes" | "no";
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

interface CreatorEconomicsOverviewResponse {
  creator?: string;
  activeSeededMarkets?: number;
  totalSeedDeployedUsdc?: number;
  currentCapitalValueUsdc?: number;
  netLiquidityPnlUsdc?: number;
  subsidyBurnUsdc?: number;
  realizedResolutionPnlUsdc?: number;
  graduationSuccessRate?: number;
  staleErrorMirrorCount?: number;
}

interface CreatorEconomicsMarketSummaryResponse {
  marketId?: string | number;
  marketQuestion?: string;
  status?: string;
  liquidityMode?: string;
  bootstrapStatus?: string;
  seedUsdc?: number;
  availableUsdc?: number;
  reservedUsdc?: number;
  inventoryYesUsdc?: number;
  inventoryNoUsdc?: number;
  inventoryNetUsdc?: number;
  currentCapitalValueUsdc?: number;
  netLiquidityPnlUsdc?: number;
  subsidyBurnUsdc?: number;
  roiBps?: number;
  cumulativeBootstrapFillsUsdc?: number;
  organicReplacementRatio?: number;
  graduationState?: string;
  graduationReason?: string;
  mirrorFreshnessSeconds?: number;
  mirrorPendingHedges?: number;
  mirrorErrorCount?: number;
  mirrorLinksWithErrors?: number;
  realizedResolutionPnlUsdc?: number;
  graduatedAt?: string;
  lastReconciledAt?: string;
}

interface CreatorEconomicsPointResponse {
  day?: string;
  cumulativeBootstrapFillsUsdc?: number;
  subsidyBurnUsdc?: number;
  inventoryMarkValueUsdc?: number;
  organicReplacementRatio?: number;
  mirrorFreshnessSeconds?: number;
  mirrorPendingHedges?: number;
  mirrorErrorCount?: number;
  graduationRetention24h?: number;
  graduationRetention7d?: number;
}

interface CreatorEconomicsTimeseriesResponse {
  window?: CreatorChartRange;
  points?: CreatorEconomicsPointResponse[];
}

interface SwarmMessageResponse {
  id: string;
  sender: string;
  message: string;
  created_at: string;
}

interface SwarmMessagesResponse {
  data: SwarmMessageResponse[];
  total_returned: number;
  limit: number;
  offset: number;
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
  status: Agent["status"];
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
  provider: "limitless" | "polymarket" | "aerodrome";
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
  provider: "limitless" | "polymarket" | "aerodrome";
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
  provider: "limitless" | "polymarket" | "aerodrome";
  market_id: string;
  preflight: Record<string, unknown>;
  typed_data?: Record<string, unknown>;
  typedData?: Record<string, unknown>;
  status: string;
  expires_at: string;
}

export interface ExternalOrderRecord {
  id: string;
  provider: "limitless" | "polymarket" | "aerodrome";
  market_id: string;
  provider_order_id: string;
  status: string;
  created_at: string;
  updated_at: string;
  response_payload: Record<string, unknown>;
  error_message?: string | null;
}

export interface PreparedExternalProviderRequest {
  provider: "limitless" | "polymarket" | "aerodrome";
  url: string;
  method: "POST" | "DELETE";
  headers: Record<string, string>;
  body: string;
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
  provider: "limitless" | "polymarket" | "aerodrome";
  market_id: string;
  outcome: "yes" | "no";
  side: "buy" | "sell";
  price: number;
  quantity: number;
  cadence_seconds: number;
  strategy: string;
  strategy_label: string;
  paper_performance?: ExternalAgentPaperPerformance | null;
  execution_mode: "live" | "paper";
  credential_id?: string | null;
  source?: string | null;
  active: boolean;
  last_executed_at?: string | null;
  next_execution_at: string;
  consecutive_failures: number;
  last_error_code?: string | null;
  created_at: string;
  updated_at: string;
}

export interface ExternalAgentPaperPerformance {
  openPositions: number;
  closedPositions: number;
  fills: number;
  volumeUsdc: number;
  feesUsdc: number;
  realizedPnlUsdc: number;
  unrealizedPnlUsdc: number;
  netPnlUsdc: number;
  maxDrawdownUsdc: number;
}

interface ExternalAgentsListResponse {
  agents: ExternalAgentRecord[];
  total: number;
  limit: number;
  offset: number;
}

export interface ExternalAgentPerformanceTotals {
  agents: number;
  activeAgents: number;
  openPositions: number;
  closedPositions: number;
  fills: number;
  volumeUsdc: number;
  feesUsdc: number;
  realizedPnlUsdc: number;
  unrealizedPnlUsdc: number;
  netPnlUsdc: number;
}

export interface ExternalAgentStrategyPerformance {
  strategy: string;
  agents: number;
  activeAgents: number;
  openPositions: number;
  closedPositions: number;
  fills: number;
  volumeUsdc: number;
  feesUsdc: number;
  realizedPnlUsdc: number;
  unrealizedPnlUsdc: number;
  netPnlUsdc: number;
  winRate: number;
}

export interface ExternalAgentPerformancePoint {
  bucket: string;
  volumeUsdc: number;
  realizedPnlUsdc: number;
  unrealizedPnlUsdc: number;
  netPnlUsdc: number;
}

export interface ExternalAgentPerformanceResponse {
  scope: string;
  owner?: string | null;
  totals: ExternalAgentPerformanceTotals;
  strategies: ExternalAgentStrategyPerformance[];
  timeline: ExternalAgentPerformancePoint[];
  updatedAt: string;
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
  decisionType: "timing" | "choice" | "hedge" | "allocation";
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
  allowedProvider?: "limitless" | "polymarket" | "aerodrome";
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
    deposit_mode: "chain" | "disabled";
    withdraw_mode: "chain" | "disabled";
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

export interface ProviderRailCapabilities {
  feed: boolean;
  marketData: boolean;
  tradeOpen: boolean;
  tradeClose: boolean;
  legacyCloseOnly: boolean;
}

export interface CompliancePolicy {
  mode: string;
  blockedCountries: string[];
  writesRestricted: boolean;
  country?: string;
  regionClass: string;
  routingMode: string;
  rails: Record<string, ProviderRailCapabilities>;
  legacyCloseOnly: boolean;
}

type ApiErrorPayload =
  | string
  | {
      code?: unknown;
      message?: unknown;
      details?: unknown;
      error?:
        | string
        | {
            code?: unknown;
            message?: unknown;
            details?: unknown;
          };
    }
  | null;

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
    public code?: string,
    public details?: unknown,
    public payload?: unknown,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

function parseApiErrorPayload(raw: string, fallback: string): {
  message: string;
  code?: string;
  details?: unknown;
  payload?: unknown;
} {
  const trimmed = raw.trim();
  if (!trimmed) {
    return { message: fallback };
  }

  try {
    const parsed = JSON.parse(trimmed) as ApiErrorPayload;

    if (typeof parsed === "string" && parsed.trim()) {
      return {
        message: parsed,
        payload: parsed,
      };
    }

    if (parsed && typeof parsed === "object") {
      const nestedError =
        parsed.error && typeof parsed.error === "object" ? parsed.error : null;
      const code =
        typeof parsed.code === "string" && parsed.code.trim()
          ? parsed.code
          : nestedError && typeof nestedError.code === "string" && nestedError.code.trim()
            ? nestedError.code
            : undefined;
      const details =
        parsed.details !== undefined
          ? parsed.details
          : nestedError?.details !== undefined
            ? nestedError.details
            : undefined;

      if (typeof parsed.message === "string" && parsed.message.trim()) {
        return {
          message: parsed.message,
          code,
          details,
          payload: parsed,
        };
      }

      if (typeof parsed.error === "string" && parsed.error.trim()) {
        return {
          message: parsed.error,
          code,
          details,
          payload: parsed,
        };
      }

      if (nestedError && typeof nestedError.message === "string" && nestedError.message.trim()) {
        return {
          message: nestedError.message,
          code,
          details,
          payload: parsed,
        };
      }

      return {
        message: fallback,
        code,
        details,
        payload: parsed,
      };
    }
  } catch {
    // Keep the raw response text below.
  }

  return {
    message: trimmed || fallback,
    payload: trimmed,
  };
}

function toNumber(value: unknown, fallback = 0): number {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return fallback;
}

function toOptionalNumber(value: unknown): number | undefined {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return undefined;
}

function toIsoString(value: unknown): string {
  if (typeof value === "string" && value.length > 0) return value;
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
    return "";
  }
  return new Date(value * 1000).toISOString();
}

function toRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

export interface TradingRestrictionNotice {
  title: string;
  message: string;
  actionLabel?: string;
  actionHref?: string;
}

export function describeTradingRestriction(error: unknown): TradingRestrictionNotice | null {
  if (!(error instanceof ApiError)) {
    return null;
  }

  if (
    error.status === 403 &&
    error.message.toLowerCase() === "this action is unavailable in this environment"
  ) {
    return {
      title: "Read-only runtime",
      message: "Trading is disabled in this environment. Market data stays live, but writes are blocked.",
      actionLabel: "Browse markets",
      actionHref: "/markets",
    };
  }

  if (error.status === 401) {
    return {
      title: "Wallet sign-in required",
      message: "Authenticate your wallet session before placing trades or using venue credentials.",
      actionLabel: "Open wallet",
      actionHref: "/wallet",
    };
  }

  if (error.code === "REGION_PROVIDER_RESTRICTED") {
    const details = toRecord(error.details);
    const provider = String(details?.provider || "this provider");
    const country = details?.country ? ` in ${String(details.country)}` : "";
    const legacyCloseOnly = Boolean(details?.legacyCloseOnly);
    const detail = typeof details?.detail === "string" ? details.detail : "";

    if (legacyCloseOnly) {
      return {
        title: "Close-only access",
        message:
          detail ||
          `New ${provider} positions are blocked${country}. Closing exposure remains allowed.`,
      };
    }

    return {
      title: "Provider restricted",
      message:
        detail ||
        `${provider} is unavailable for this action${country} under the current routing policy.`,
    };
  }

  const lowerMessage = error.message.toLowerCase();
  if (error.code === "CREDENTIAL_NOT_READY" || lowerMessage.includes("credential")) {
    return {
      title: "Credential not ready",
      message: error.message,
      actionLabel: "Fix credentials",
      actionHref: "/settings/credentials",
    };
  }

  if (
    lowerMessage.includes("fund") ||
    lowerMessage.includes("balance") ||
    lowerMessage.includes("insufficient")
  ) {
    return {
      title: "Insufficient funding",
      message: error.message,
      actionLabel: "Open wallet",
      actionHref: "/wallet",
    };
  }

  return null;
}

function normalizeMarketStatus(value: unknown): Market["status"] {
  if (
    value === "active" ||
    value === "paused" ||
    value === "closed" ||
    value === "resolved" ||
    value === "cancelled"
  ) {
    return value;
  }
  return "active";
}

function normalizeMarket(raw: Record<string, unknown>): Market {
  const yesPrice = toNumber(raw.yesPrice ?? raw.yes_price, 0.5);
  const noPrice = toNumber(raw.noPrice ?? raw.no_price, 1 - yesPrice);
  const sourceRaw = String(raw.source ?? "internal").toLowerCase();
  const source =
    sourceRaw === "limitless" ||
    sourceRaw === "polymarket" ||
    sourceRaw === "all"
      ? sourceRaw
      : "internal";

  return {
    id: String(raw.id ?? ""),
    address: String(raw.address ?? raw.id ?? ""),
    source,
    provider: String(raw.provider ?? "internal"),
    isExternal: Boolean(raw.isExternal ?? raw.is_external ?? false),
    externalUrl: String(raw.externalUrl ?? raw.external_url ?? "") || undefined,
    chainId: toNumber(raw.chainId ?? raw.chain_id, 8453),
    requiresCredentials: Boolean(
      raw.requiresCredentials ?? raw.requires_credentials ?? false,
    ),
    executionUsers: Boolean(raw.executionUsers ?? raw.execution_users ?? true),
    executionAgents: Boolean(
      raw.executionAgents ?? raw.execution_agents ?? true,
    ),
    isSyntheticTrades: Boolean(
      raw.isSyntheticTrades ?? raw.is_synthetic_trades ?? false,
    ),
    question: String(raw.question ?? ""),
    description: String(raw.description ?? ""),
    category: String(raw.category ?? "unknown"),
    status: normalizeMarketStatus(raw.status),
    yesPrice,
    noPrice,
    yesSupply: toNumber(raw.yesSupply ?? raw.yes_supply),
    noSupply: toNumber(raw.noSupply ?? raw.no_supply),
    volume24h: toNumber(raw.volume24h ?? raw.volume_24h),
    totalVolume: toNumber(raw.totalVolume ?? raw.total_volume),
    totalCollateral: toNumber(raw.totalCollateral ?? raw.total_collateral),
    feeBps: toNumber(raw.feeBps ?? raw.fee_bps),
    oracle: String(raw.oracle ?? ""),
    collateralMint: String(raw.collateralMint ?? raw.collateral_mint ?? ""),
    yesMint: String(raw.yesMint ?? raw.yes_mint ?? ""),
    noMint: String(raw.noMint ?? raw.no_mint ?? ""),
    resolutionDeadline: toIsoString(
      raw.resolutionDeadline ?? raw.resolution_deadline,
    ),
    tradingEnd: toIsoString(raw.tradingEnd ?? raw.trading_end),
    resolvedOutcome: (raw.resolvedOutcome ??
      raw.resolved_outcome) as Market["resolvedOutcome"],
    createdAt: toIsoString(raw.createdAt ?? raw.created_at),
    resolvedAt:
      raw.resolvedAt || raw.resolved_at
        ? toIsoString(raw.resolvedAt ?? raw.resolved_at)
        : undefined,
  };
}

export function mapBaseSnapshotToMarket(snapshot: BaseMarketSnapshot): Market {
  const curated = CURATED_MARKETS_BY_ID[Number(snapshot.id)];
  const resolvedOutcome =
    snapshot.outcome === "yes" || snapshot.outcome === "no"
      ? snapshot.outcome
      : undefined;
  const derivedYesPrice = snapshot.outcomes?.find(
    (outcome) => outcome.label.trim().toLowerCase() === "yes",
  )?.probability;
  const derivedNoPrice = snapshot.outcomes?.find(
    (outcome) => outcome.label.trim().toLowerCase() === "no",
  )?.probability;

  let yesPrice = toNumber(
    snapshot.yes_price,
    derivedYesPrice ??
      (resolvedOutcome === "yes" ? 1 : resolvedOutcome === "no" ? 0 : 0.5),
  );
  let noPrice = toNumber(
    snapshot.no_price,
    derivedNoPrice ??
      (resolvedOutcome === "yes"
        ? 0
        : resolvedOutcome === "no"
          ? 1
          : 1 - yesPrice),
  );

  if (resolvedOutcome === "yes") {
    yesPrice = 1;
    noPrice = 0;
  } else if (resolvedOutcome === "no") {
    yesPrice = 0;
    noPrice = 1;
  }

  const tradingEnd = fromUnixSeconds(snapshot.close_time);
  const resolutionDeadline = fromUnixSeconds(
    snapshot.resolve_time || snapshot.close_time,
  );
  const question =
    snapshot.question?.trim() ||
    curated?.question ||
    `Base market #${snapshot.id}`;
  const description =
    snapshot.description?.trim() ||
    (curated
      ? `Outcomes: ${curated.outcomes}. Context: ${curated.rationale}`
      : `Question hash: ${snapshot.question_hash}`);
  const category = snapshot.category?.trim() || curated?.category || "base";
  const sourceRaw = String(snapshot.source || "internal").toLowerCase();
  const source =
    sourceRaw === "limitless" ||
    sourceRaw === "polymarket" ||
    sourceRaw === "all"
      ? sourceRaw
      : "internal";
  const totalVolume = toNumber(snapshot.volume, 0);

  return {
    id: snapshot.id,
    address: `base-market-${snapshot.id}`,
    source,
    provider: snapshot.provider || (snapshot.is_external ? source : "internal"),
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
    volume24h: totalVolume,
    totalVolume,
    totalCollateral: 0,
    feeBps: 0,
    oracle: snapshot.resolver,
    collateralMint: "",
    yesMint: "",
    noMint: "",
    resolutionDeadline,
    tradingEnd,
    resolvedOutcome,
    createdAt: tradingEnd,
    resolvedAt: snapshot.resolved
      ? fromUnixSeconds(snapshot.resolve_time)
      : undefined,
    outcomes:
      snapshot.outcomes && snapshot.outcomes.length > 0
        ? snapshot.outcomes
        : undefined,
    liquidityMode: snapshot.liquidity_mode,
    bootstrapStatus: snapshot.bootstrap_status,
    bootstrapActive: snapshot.bootstrap_active,
    bootstrapSeedUsdc: toOptionalNumber(snapshot.bootstrap_seed_usdc),
    bootstrapManager: snapshot.bootstrap_manager,
    bootstrapPreset: snapshot.bootstrap_preset,
    bootstrapStrategy: snapshot.bootstrap_strategy,
    bootstrapLevels: toOptionalNumber(snapshot.bootstrap_levels),
    bootstrapInitialYesBps: toOptionalNumber(
      snapshot.bootstrap_initial_yes_bps,
    ),
    bootstrapBaseSpreadBps: toOptionalNumber(
      snapshot.bootstrap_base_spread_bps,
    ),
    bootstrapStepBps: toOptionalNumber(snapshot.bootstrap_step_bps),
    bootstrapCadenceSeconds: toOptionalNumber(
      snapshot.bootstrap_cadence_seconds,
    ),
    bootstrapExpirySeconds: toOptionalNumber(snapshot.bootstrap_expiry_seconds),
    bootstrapPauseReason: snapshot.bootstrap_pause_reason,
    bootstrapReservedUsdc: toOptionalNumber(snapshot.bootstrap_reserved_usdc),
    bootstrapAvailableUsdc: toOptionalNumber(snapshot.bootstrap_available_usdc),
    bootstrapActiveSlots: toOptionalNumber(snapshot.bootstrap_active_slots),
    bootstrapOrganicDepthRatio: toOptionalNumber(
      snapshot.bootstrap_organic_depth_ratio,
    ),
    bootstrapConsecutiveFailures: toOptionalNumber(
      snapshot.bootstrap_consecutive_failures,
    ),
    bootstrapGraduatedAt: snapshot.bootstrap_graduated_at
      ? toIsoString(snapshot.bootstrap_graduated_at)
      : undefined,
    bootstrapLaunchTxHash: snapshot.bootstrap_launch_tx_hash || undefined,
    bootstrapLastReconciledAt: snapshot.bootstrap_last_reconciled_at
      ? toIsoString(snapshot.bootstrap_last_reconciled_at)
      : undefined,
    bootstrapLastError: snapshot.bootstrap_last_error || undefined,
    bootstrapInventoryYesUsdc: toOptionalNumber(
      snapshot.bootstrap_inventory_yes_usdc,
    ),
    bootstrapInventoryNoUsdc: toOptionalNumber(
      snapshot.bootstrap_inventory_no_usdc,
    ),
    bootstrapInventoryTotalUsdc: toOptionalNumber(
      snapshot.bootstrap_inventory_total_usdc,
    ),
    bootstrapInventoryNetUsdc: toOptionalNumber(
      snapshot.bootstrap_inventory_net_usdc,
    ),
    mirrorLinkCount: toOptionalNumber(snapshot.mirror_link_count),
    mirrorActiveLinkCount: toOptionalNumber(snapshot.mirror_active_link_count),
    mirrorLastMirrorAt: snapshot.mirror_last_mirror_at
      ? toIsoString(snapshot.mirror_last_mirror_at)
      : undefined,
    mirrorLastHedgeAt: snapshot.mirror_last_hedge_at
      ? toIsoString(snapshot.mirror_last_hedge_at)
      : undefined,
    mirrorFreshnessSeconds: toOptionalNumber(snapshot.mirror_freshness_seconds),
    mirrorPendingHedges: toOptionalNumber(snapshot.mirror_pending_hedges),
    mirrorLinksWithErrors: toOptionalNumber(snapshot.mirror_links_with_errors),
    mirrorHedgeErrors: toOptionalNumber(snapshot.mirror_hedge_errors),
    mirrorTotalMirroredUsdc: toOptionalNumber(
      snapshot.mirror_total_mirrored_usdc,
    ),
    mirrorTotalHedgedUsdc: toOptionalNumber(snapshot.mirror_total_hedged_usdc),
    mirrorNetExposureUsdc: toOptionalNumber(snapshot.mirror_net_exposure_usdc),
    tradabilityScore: toOptionalNumber(snapshot.tradability_score),
  };
}

export function normalizeBaseMarketsResponse(
  response: BaseMarketsResponse,
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

export function normalizeBaseOrderBookResponse(
  response: BaseOrderBookResponse,
): OrderBook {
  return {
    marketId: response.market_id,
    outcome: response.outcome,
    bids: response.bids ?? [],
    asks: response.asks ?? [],
    lastUpdated: toIsoString(response.last_updated),
    includesBootstrap: Boolean(response.includes_bootstrap),
    includesMirror: Boolean(response.includes_mirror),
    bootstrapDepth: toOptionalNumber(response.bootstrap_depth),
    organicDepth: toOptionalNumber(response.organic_depth),
    mirrorDepth: toOptionalNumber(response.mirror_depth),
    bootstrapInventoryYesUsdc: toOptionalNumber(
      response.bootstrap_inventory_yes_usdc,
    ),
    bootstrapInventoryNoUsdc: toOptionalNumber(
      response.bootstrap_inventory_no_usdc,
    ),
    bootstrapInventoryTotalUsdc: toOptionalNumber(
      response.bootstrap_inventory_total_usdc,
    ),
    bootstrapInventoryNetUsdc: toOptionalNumber(
      response.bootstrap_inventory_net_usdc,
    ),
    mirrorLinkCount: toOptionalNumber(response.mirror_link_count),
    mirrorActiveLinkCount: toOptionalNumber(response.mirror_active_link_count),
    mirrorLastMirrorAt: response.mirror_last_mirror_at
      ? toIsoString(response.mirror_last_mirror_at)
      : undefined,
    mirrorLastHedgeAt: response.mirror_last_hedge_at
      ? toIsoString(response.mirror_last_hedge_at)
      : undefined,
    mirrorFreshnessSeconds: toOptionalNumber(response.mirror_freshness_seconds),
    mirrorPendingHedges: toOptionalNumber(response.mirror_pending_hedges),
    mirrorLinksWithErrors: toOptionalNumber(response.mirror_links_with_errors),
    mirrorHedgeErrors: toOptionalNumber(response.mirror_hedge_errors),
    mirrorTotalMirroredUsdc: toOptionalNumber(
      response.mirror_total_mirrored_usdc,
    ),
    mirrorTotalHedgedUsdc: toOptionalNumber(response.mirror_total_hedged_usdc),
    mirrorNetExposureUsdc: toOptionalNumber(response.mirror_net_exposure_usdc),
    tradabilityScore: toOptionalNumber(response.tradability_score),
  };
}

function normalizeOutcome(value: unknown): Outcome {
  return value === "no" ? "no" : "yes";
}

function normalizeTransactionType(value: unknown): Transaction["txType"] {
  switch (value) {
    case "deposit":
    case "withdraw":
    case "buy":
    case "sell":
    case "claim":
    case "mint":
    case "redeem":
      return value;
    default:
      return "deposit";
  }
}

function normalizeTransaction(raw: Record<string, unknown>): Transaction {
  const txSignature = String(raw.txSignature ?? raw.tx_signature ?? "");

  return {
    id: String(raw.id ?? ""),
    owner: String(raw.owner ?? ""),
    txType: normalizeTransactionType(raw.txType ?? raw.tx_type),
    marketId:
      (raw.marketId ?? raw.market_id)
        ? String(raw.marketId ?? raw.market_id)
        : undefined,
    amount: toNumber(raw.amount),
    fee: toNumber(raw.fee),
    txSignature: txSignature || undefined,
    status: String(raw.status ?? "pending"),
    createdAt: toIsoString(raw.createdAt ?? raw.created_at),
  };
}

function normalizeTrade(raw: Record<string, unknown>): Trade {
  const providerMarketRef = raw.providerMarketRef ?? raw.provider_market_ref;
  const synthetic = raw.isSynthetic ?? raw.is_synthetic;

  return {
    id: String(raw.id ?? ""),
    marketId: String(raw.marketId ?? raw.market_id ?? ""),
    outcome: normalizeOutcome(raw.outcome),
    price: toNumber(raw.price),
    quantity: toNumber(raw.quantity),
    buyer: String(raw.buyer ?? ""),
    seller: String(raw.seller ?? ""),
    txSignature: String(
      raw.txSignature ?? raw.tx_signature ?? raw.tx_hash ?? "",
    ),
    provider: raw.provider != null ? String(raw.provider) : undefined,
    providerMarketRef:
      providerMarketRef != null ? String(providerMarketRef) : undefined,
    chainId: toOptionalNumber(raw.chainId ?? raw.chain_id),
    isSynthetic: synthetic != null ? Boolean(synthetic) : undefined,
    blockNumber: toOptionalNumber(raw.blockNumber ?? raw.block_number),
    createdAt: toIsoString(raw.createdAt ?? raw.created_at),
  };
}

function normalizeCreatorEconomicsOverview(
  raw: CreatorEconomicsOverviewResponse,
): CreatorEconomicsOverview {
  return {
    creator: String(raw.creator ?? ""),
    activeSeededMarkets: toNumber(raw.activeSeededMarkets),
    totalSeedDeployedUsdc: toNumber(raw.totalSeedDeployedUsdc),
    currentCapitalValueUsdc: toNumber(raw.currentCapitalValueUsdc),
    netLiquidityPnlUsdc: toNumber(raw.netLiquidityPnlUsdc),
    subsidyBurnUsdc: toNumber(raw.subsidyBurnUsdc),
    realizedResolutionPnlUsdc: toNumber(raw.realizedResolutionPnlUsdc),
    graduationSuccessRate: toNumber(raw.graduationSuccessRate),
    staleErrorMirrorCount: toNumber(raw.staleErrorMirrorCount),
  };
}

function normalizeCreatorEconomicsMarketSummary(
  raw: CreatorEconomicsMarketSummaryResponse,
): CreatorEconomicsMarketSummary {
  return {
    marketId: String(raw.marketId ?? ""),
    marketQuestion: String(raw.marketQuestion ?? ""),
    status: String(raw.status ?? "unknown"),
    liquidityMode: String(raw.liquidityMode ?? "clob_only"),
    bootstrapStatus: String(raw.bootstrapStatus ?? "unknown"),
    seedUsdc: toNumber(raw.seedUsdc),
    reservedBudgetUsdc: toNumber(raw.reservedUsdc),
    availableBudgetUsdc: toNumber(raw.availableUsdc),
    inventoryYesUsdc: toNumber(raw.inventoryYesUsdc),
    inventoryNoUsdc: toNumber(raw.inventoryNoUsdc),
    inventoryNetUsdc: toNumber(raw.inventoryNetUsdc),
    currentCapitalValueUsdc: toNumber(raw.currentCapitalValueUsdc),
    cumulativeBootstrapFillsUsdc: toNumber(raw.cumulativeBootstrapFillsUsdc),
    subsidyBurnUsdc: toNumber(raw.subsidyBurnUsdc),
    netLiquidityPnlUsdc: toNumber(raw.netLiquidityPnlUsdc),
    roiBps: toNumber(raw.roiBps),
    organicReplacementRatio: toNumber(raw.organicReplacementRatio),
    graduationState: String(raw.graduationState ?? raw.bootstrapStatus ?? "unknown"),
    graduationReason: raw.graduationReason || undefined,
    mirrorFreshnessSeconds:
      raw.mirrorFreshnessSeconds == null
        ? undefined
        : toNumber(raw.mirrorFreshnessSeconds),
    mirrorPendingHedges: toNumber(raw.mirrorPendingHedges),
    mirrorErrorCount: toNumber(raw.mirrorErrorCount),
    mirrorLinksWithErrors: toNumber(raw.mirrorLinksWithErrors),
    realizedResolutionPnlUsdc: toNumber(raw.realizedResolutionPnlUsdc),
    graduatedAt: raw.graduatedAt ? toIsoString(raw.graduatedAt) : undefined,
    lastReconciledAt: raw.lastReconciledAt
      ? toIsoString(raw.lastReconciledAt)
      : undefined,
  };
}

function normalizeCreatorEconomicsPoint(
  raw: CreatorEconomicsPointResponse,
): CreatorEconomicsPoint {
  return {
    day: toIsoString(raw.day),
    cumulativeBootstrapFillsUsdc: toNumber(raw.cumulativeBootstrapFillsUsdc),
    subsidyBurnUsdc: toNumber(raw.subsidyBurnUsdc),
    inventoryMarkValueUsdc: toNumber(raw.inventoryMarkValueUsdc),
    organicReplacementRatio: toNumber(raw.organicReplacementRatio),
    mirrorFreshnessSeconds:
      raw.mirrorFreshnessSeconds == null
        ? undefined
        : toNumber(raw.mirrorFreshnessSeconds),
    mirrorPendingHedges: toNumber(raw.mirrorPendingHedges),
    mirrorErrorCount: toNumber(raw.mirrorErrorCount),
    graduationRetention24h:
      raw.graduationRetention24h == null
        ? undefined
        : toNumber(raw.graduationRetention24h),
    graduationRetention7d:
      raw.graduationRetention7d == null
        ? undefined
        : toNumber(raw.graduationRetention7d),
  };
}

function normalizeCreatorEconomicsMarketDetail(
  raw: CreatorEconomicsMarketSummaryResponse,
  timeseries: CreatorEconomicsTimeseriesResponse,
): CreatorEconomicsMarketDetail {
  return {
    ...normalizeCreatorEconomicsMarketSummary(raw),
    window: (timeseries.window as CreatorChartRange | undefined) ?? "30d",
    points: Array.isArray(timeseries.points)
      ? timeseries.points.map(normalizeCreatorEconomicsPoint)
      : [],
  };
}

function normalizeExternalOrderRecord(
  raw: Record<string, unknown>,
): ExternalOrderRecord {
  const responsePayload = raw.responsePayload ?? raw.response_payload;

  return {
    id: String(raw.id ?? ""),
    provider: String(
      raw.provider ?? "limitless",
    ) as ExternalOrderRecord["provider"],
    market_id: String(raw.marketId ?? raw.market_id ?? ""),
    provider_order_id: String(
      raw.providerOrderId ?? raw.provider_order_id ?? "",
    ),
    status: String(raw.status ?? "pending"),
    created_at: toIsoString(raw.createdAt ?? raw.created_at),
    updated_at: toIsoString(raw.updatedAt ?? raw.updated_at),
    response_payload:
      responsePayload &&
      typeof responsePayload === "object" &&
      !Array.isArray(responsePayload)
        ? (responsePayload as Record<string, unknown>)
        : {},
    error_message:
      raw.errorMessage === null || raw.error_message === null
        ? null
        : (raw.errorMessage ?? raw.error_message)
          ? String(raw.errorMessage ?? raw.error_message)
          : undefined,
  };
}

function normalizeExternalAgentRecord(
  raw: Record<string, unknown>,
): ExternalAgentRecord {
  const paperPerformanceRaw =
    raw.paperPerformance && typeof raw.paperPerformance === "object" && !Array.isArray(raw.paperPerformance)
      ? (raw.paperPerformance as Record<string, unknown>)
      : raw.paper_performance && typeof raw.paper_performance === "object" && !Array.isArray(raw.paper_performance)
        ? (raw.paper_performance as Record<string, unknown>)
        : null;
  return {
    id: String(raw.id ?? ""),
    owner: String(raw.owner ?? ""),
    name: String(raw.name ?? ""),
    provider: String(
      raw.provider ?? "limitless",
    ) as ExternalAgentRecord["provider"],
    market_id: String(raw.marketId ?? raw.market_id ?? ""),
    outcome: String(raw.outcome ?? "yes") as ExternalAgentRecord["outcome"],
    side: String(raw.side ?? "buy") as ExternalAgentRecord["side"],
    price: toNumber(raw.price),
    quantity: toNumber(raw.quantity),
    cadence_seconds: toNumber(raw.cadenceSeconds ?? raw.cadence_seconds),
    strategy: String(raw.strategy ?? ""),
    strategy_label: String(
      raw.strategyLabel ?? raw.strategy_label ?? raw.strategy ?? "",
    ),
    paper_performance:
      raw.paperPerformance === null || raw.paper_performance === null
        ? null
        : paperPerformanceRaw
          ? normalizeExternalAgentPaperPerformance(paperPerformanceRaw)
          : undefined,
    execution_mode: String(
      raw.executionMode ?? raw.execution_mode ?? "live",
    ) as ExternalAgentRecord["execution_mode"],
    credential_id:
      raw.credentialId === null || raw.credential_id === null
        ? null
        : (raw.credentialId ?? raw.credential_id)
          ? String(raw.credentialId ?? raw.credential_id)
          : undefined,
    source:
      raw.source === null ? null : raw.source ? String(raw.source) : undefined,
    active: Boolean(raw.active),
    last_executed_at:
      raw.lastExecutedAt === null || raw.last_executed_at === null
        ? null
        : (raw.lastExecutedAt ?? raw.last_executed_at)
          ? toIsoString(raw.lastExecutedAt ?? raw.last_executed_at)
          : undefined,
    next_execution_at: toIsoString(
      raw.nextExecutionAt ?? raw.next_execution_at,
    ),
    consecutive_failures: toNumber(
      raw.consecutiveFailures ?? raw.consecutive_failures,
    ),
    last_error_code:
      raw.lastErrorCode === null || raw.last_error_code === null
        ? null
        : (raw.lastErrorCode ?? raw.last_error_code)
          ? String(raw.lastErrorCode ?? raw.last_error_code)
          : undefined,
    created_at: toIsoString(raw.createdAt ?? raw.created_at),
    updated_at: toIsoString(raw.updatedAt ?? raw.updated_at),
  };
}

function normalizeExternalAgentPaperPerformance(
  raw: Record<string, unknown>,
): ExternalAgentPaperPerformance {
  return {
    openPositions: toNumber(raw.openPositions ?? raw.open_positions),
    closedPositions: toNumber(raw.closedPositions ?? raw.closed_positions),
    fills: toNumber(raw.fills),
    volumeUsdc: toNumber(raw.volumeUsdc ?? raw.volume_usdc),
    feesUsdc: toNumber(raw.feesUsdc ?? raw.fees_usdc),
    realizedPnlUsdc: toNumber(raw.realizedPnlUsdc ?? raw.realized_pnl_usdc),
    unrealizedPnlUsdc: toNumber(raw.unrealizedPnlUsdc ?? raw.unrealized_pnl_usdc),
    netPnlUsdc: toNumber(raw.netPnlUsdc ?? raw.net_pnl_usdc),
    maxDrawdownUsdc: toNumber(raw.maxDrawdownUsdc ?? raw.max_drawdown_usdc),
  };
}

function normalizeExternalAgentPerformanceResponse(
  raw: Record<string, unknown>,
): ExternalAgentPerformanceResponse {
  const totalsRaw =
    raw.totals && typeof raw.totals === "object" && !Array.isArray(raw.totals)
      ? (raw.totals as Record<string, unknown>)
      : {};
  const strategiesRaw = Array.isArray(raw.strategies) ? raw.strategies : [];
  const timelineRaw = Array.isArray(raw.timeline) ? raw.timeline : [];

  return {
    scope: String(raw.scope ?? ""),
    owner:
      raw.owner === null ? null : raw.owner ? String(raw.owner) : undefined,
    totals: {
      agents: toNumber(totalsRaw.agents),
      activeAgents: toNumber(totalsRaw.activeAgents ?? totalsRaw.active_agents),
      openPositions: toNumber(
        totalsRaw.openPositions ?? totalsRaw.open_positions,
      ),
      closedPositions: toNumber(
        totalsRaw.closedPositions ?? totalsRaw.closed_positions,
      ),
      fills: toNumber(totalsRaw.fills),
      volumeUsdc: toNumber(totalsRaw.volumeUsdc ?? totalsRaw.volume_usdc),
      feesUsdc: toNumber(totalsRaw.feesUsdc ?? totalsRaw.fees_usdc),
      realizedPnlUsdc: toNumber(
        totalsRaw.realizedPnlUsdc ?? totalsRaw.realized_pnl_usdc,
      ),
      unrealizedPnlUsdc: toNumber(
        totalsRaw.unrealizedPnlUsdc ?? totalsRaw.unrealized_pnl_usdc,
      ),
      netPnlUsdc: toNumber(totalsRaw.netPnlUsdc ?? totalsRaw.net_pnl_usdc),
    },
    strategies: strategiesRaw.map((entry) => {
      const strategy =
        entry && typeof entry === "object"
          ? (entry as Record<string, unknown>)
          : {};
      return {
        strategy: String(strategy.strategy ?? ""),
        agents: toNumber(strategy.agents),
        activeAgents: toNumber(strategy.activeAgents ?? strategy.active_agents),
        openPositions: toNumber(
          strategy.openPositions ?? strategy.open_positions,
        ),
        closedPositions: toNumber(
          strategy.closedPositions ?? strategy.closed_positions,
        ),
        fills: toNumber(strategy.fills),
        volumeUsdc: toNumber(strategy.volumeUsdc ?? strategy.volume_usdc),
        feesUsdc: toNumber(strategy.feesUsdc ?? strategy.fees_usdc),
        realizedPnlUsdc: toNumber(
          strategy.realizedPnlUsdc ?? strategy.realized_pnl_usdc,
        ),
        unrealizedPnlUsdc: toNumber(
          strategy.unrealizedPnlUsdc ?? strategy.unrealized_pnl_usdc,
        ),
        netPnlUsdc: toNumber(strategy.netPnlUsdc ?? strategy.net_pnl_usdc),
        winRate: toNumber(strategy.winRate ?? strategy.win_rate),
      };
    }),
    timeline: timelineRaw.map((entry) => {
      const point =
        entry && typeof entry === "object"
          ? (entry as Record<string, unknown>)
          : {};
      return {
        bucket: toIsoString(point.bucket),
        volumeUsdc: toNumber(point.volumeUsdc ?? point.volume_usdc),
        realizedPnlUsdc: toNumber(
          point.realizedPnlUsdc ?? point.realized_pnl_usdc,
        ),
        unrealizedPnlUsdc: toNumber(
          point.unrealizedPnlUsdc ?? point.unrealized_pnl_usdc,
        ),
        netPnlUsdc: toNumber(point.netPnlUsdc ?? point.net_pnl_usdc),
      };
    }),
    updatedAt: toIsoString(raw.updatedAt ?? raw.updated_at),
  };
}

function mapBaseTradeToTrade(
  snapshot: BaseTradeSnapshot,
  meta?: Pick<
    BaseTradesResponse,
    "provider" | "chain_id" | "provider_market_ref" | "is_synthetic"
  >,
): Trade {
  return {
    id: snapshot.id,
    marketId: snapshot.market_id,
    outcome: snapshot.outcome,
    price: snapshot.price,
    quantity: snapshot.quantity,
    buyer: "",
    seller: "",
    txSignature: snapshot.tx_hash,
    provider: meta?.provider,
    providerMarketRef: meta?.provider_market_ref,
    chainId: meta?.chain_id,
    isSynthetic: meta?.is_synthetic,
    blockNumber: snapshot.block_number,
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
    size: String(snapshot.size ?? "0"),
    cadence: toNumber(snapshot.cadence),
    expiryWindow: toNumber(snapshot.expiry_window),
    lastExecutedAt: fromUnixSecondsOptional(snapshot.last_executed_at),
    nextExecutionAt: fromUnixSecondsOptional(snapshot.next_execution_at),
    canExecute: Boolean(snapshot.can_execute),
    active: Boolean(snapshot.active),
    status: snapshot.status ?? "inactive",
    strategy: String(snapshot.strategy ?? ""),
    identityId: snapshot.identity_id ? String(snapshot.identity_id) : undefined,
    identityTier:
      typeof snapshot.identity_tier === "number"
        ? snapshot.identity_tier
        : undefined,
    identityActive:
      typeof snapshot.identity_active === "boolean"
        ? snapshot.identity_active
        : undefined,
    identityUpdatedAt:
      typeof snapshot.identity_updated_at === "number"
        ? fromUnixSecondsOptional(snapshot.identity_updated_at)
        : undefined,
    reputationScoreBps:
      typeof snapshot.reputation_score_bps === "number"
        ? snapshot.reputation_score_bps
        : undefined,
    reputationConfidenceBps:
      typeof snapshot.reputation_confidence_bps === "number"
        ? snapshot.reputation_confidence_bps
        : undefined,
    reputationEvents:
      typeof snapshot.reputation_events === "number"
        ? snapshot.reputation_events
        : undefined,
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
      const res = await fetch("/api/auth", { method: "GET" });
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
    skipWriteGuard = false,
  ): Promise<T> {
    const method = String(options.method || "GET").toUpperCase();

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

    if (
      options.body !== undefined &&
      !("Content-Type" in (headers as Record<string, string>))
    ) {
      (headers as Record<string, string>)["Content-Type"] = "application/json";
    }

    if (this.accessToken) {
      (headers as Record<string, string>)["Authorization"] =
        `Bearer ${this.accessToken}`;
    }
    const canUseFallback =
      method === "GET" &&
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
        throw new ApiError(401, "Session expired");
      }
    }

    if (!res.ok) {
      const text = await res.text();
      const parsed = parseApiErrorPayload(text, res.statusText);
      throw new ApiError(
        res.status,
        parsed.message,
        parsed.code,
        parsed.details,
        parsed.payload,
      );
    }

    if (res.status === 204) {
      return {} as T;
    }

    return res.json();
  }

  private async requestFromBase<T>(
    base: string,
    path: string,
    options: RequestInit = {},
  ): Promise<T> {
    const headers: HeadersInit = {
      ...options.headers,
    };

    if (
      options.body !== undefined &&
      !("Content-Type" in (headers as Record<string, string>))
    ) {
      (headers as Record<string, string>)["Content-Type"] = "application/json";
    }

    if (this.accessToken) {
      (headers as Record<string, string>)["Authorization"] =
        `Bearer ${this.accessToken}`;
    }

    const response = await fetch(`${base}${path}`, {
      ...options,
      headers,
    });

    if (!response.ok) {
      const text = await response.text();
      const parsed = parseApiErrorPayload(text, response.statusText);
      throw new ApiError(
        response.status,
        parsed.message,
        parsed.code,
        parsed.details,
        parsed.payload,
      );
    }

    if (response.status === 204) {
      return {} as T;
    }

    return response.json();
  }

  private async requestBaseRead<T>(path: string): Promise<T> {
    if (LOCAL_BASE_READ_API_BASE) {
      try {
        return await this.requestFromBase<T>(LOCAL_BASE_READ_API_BASE, path);
      } catch (localError) {
        try {
          return await this.request<T>(path);
        } catch {
          throw localError;
        }
      }
    }

    try {
      return await this.request<T>(path);
    } catch (primaryError) {
      throw primaryError;
    }
  }

  private async assertRequestWritable(method: string) {
    if (method === "GET" || method === "HEAD" || method === "OPTIONS") {
      return;
    }

    if (readOnlyPreviewEnabled || isReadOnlyMode(this.capabilities)) {
      throw new ApiError(403, "This action is unavailable in this environment");
    }

    const capabilities = await this.loadCapabilities();
    if (!capabilities) {
      throw new ApiError(
        503,
        "Runtime status is unavailable, write actions are disabled",
      );
    }
    if (isReadOnlyMode(capabilities)) {
      throw new ApiError(403, "This action is unavailable in this environment");
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
        const capabilities =
          await this.requestBaseRead<Web4Capabilities>("/web4/capabilities");
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
        const res = await fetch("/api/auth", { method: "PUT" });
        if (!res.ok) {
          this.clearAccessToken();
          throw new Error("Refresh failed");
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
      ([, v]) => v !== undefined && v !== null,
    );
    if (filtered.length === 0) return "";
    return (
      "?" +
      new URLSearchParams(filtered.map(([k, v]) => [k, String(v)])).toString()
    );
  }

  // Markets
  async getMarkets(
    filters?: MarketFilters,
  ): Promise<PaginatedResponse<Market>> {
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
    const response = await this.request<Record<string, unknown>>(
      `/markets/${id}`,
    );
    return normalizeMarket(response);
  }

  async getOrderBook(
    marketId: string,
    outcome: Outcome,
    depth = 20,
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
    }>(`/markets/${marketId}/orderbook?outcome=${outcome}&depth=${depth}`);

    return {
      marketId: String(response.marketId ?? response.market_id ?? marketId),
      outcome: response.outcome === "no" ? "no" : "yes",
      bids: response.bids ?? [],
      asks: response.asks ?? [],
      lastUpdated: toIsoString(
        response.lastUpdated ?? response.last_updated ?? response.timestamp,
      ),
    };
  }

  async getTrades(
    marketId: string,
    params?: { outcome?: Outcome; limit?: number; before?: string },
  ): Promise<PaginatedResponse<Trade>> {
    return this.getBaseTrades(marketId, params);
  }

  async getCreatorEconomicsOverview(): Promise<CreatorEconomicsOverview> {
    const response = await this.request<CreatorEconomicsOverviewResponse>(
      "/evm/creator/overview",
    );
    return normalizeCreatorEconomicsOverview(response);
  }

  async getCreatorEconomicsMarkets(): Promise<PaginatedResponse<CreatorEconomicsMarketSummary>> {
    const response = await this.request<CreatorEconomicsMarketSummaryResponse[]>(
      "/evm/creator/markets",
    );
    const data = Array.isArray(response)
      ? response.map(normalizeCreatorEconomicsMarketSummary)
      : [];
    return {
      data,
      total: data.length,
      limit: data.length,
      offset: 0,
      hasMore: false,
    };
  }

  async getCreatorEconomicsMarket(
    marketId: string,
    window: CreatorChartRange = "30d",
  ): Promise<CreatorEconomicsMarketDetail> {
    const economics = await this.request<{
      market: CreatorEconomicsMarketSummaryResponse;
    }>(`/evm/creator/markets/${encodeURIComponent(marketId)}/economics`);
    const timeseries = await this.request<CreatorEconomicsTimeseriesResponse>(
      `/evm/creator/markets/${encodeURIComponent(marketId)}/timeseries?window=${window}`,
    );
    return normalizeCreatorEconomicsMarketDetail(economics.market, timeseries);
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
    return this.request("/orders", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async cancelOrder(orderId: string): Promise<CancelOrderResponse> {
    return this.request(`/orders/${orderId}`, {
      method: "DELETE",
    });
  }

  // Positions
  async getPositions(): Promise<PaginatedResponse<Position>> {
    return this.request<PaginatedResponse<Position>>("/positions");
  }

  async getPosition(marketId: string): Promise<Position> {
    return this.request(`/positions/${marketId}`);
  }

  async claimWinnings(
    marketId: string,
    txSignature: string,
  ): Promise<ClaimWinningsResponse> {
    return this.request(`/positions/${marketId}/claim`, {
      method: "POST",
      body: JSON.stringify({ txSignature }),
    });
  }

  // User
  async getProfile(): Promise<User> {
    return this.request("/user/profile");
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
      normalizeTransaction(entry),
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
    return this.request("/wallet/balance");
  }

  async getCompliancePolicy(): Promise<CompliancePolicy> {
    const response = await this.request<{
      mode: string;
      blockedCountries?: string[];
      blocked_countries?: string[];
      writesRestricted?: boolean;
      writes_restricted?: boolean;
      country?: string;
      regionClass?: string;
      region_class?: string;
      routingMode?: string;
      routing_mode?: string;
      rails?: Record<string, unknown>;
      legacyCloseOnly?: boolean;
      legacy_close_only?: boolean;
    }>("/compliance/policy");

    const rails = Object.fromEntries(
      Object.entries(response.rails || {}).map(([provider, raw]) => {
        const value = toRecord(raw) || {};
        return [
          provider,
          {
            feed: Boolean(value.feed),
            marketData: Boolean(value.marketData ?? value.market_data),
            tradeOpen: Boolean(value.tradeOpen ?? value.trade_open),
            tradeClose: Boolean(value.tradeClose ?? value.trade_close),
            legacyCloseOnly: Boolean(
              value.legacyCloseOnly ?? value.legacy_close_only,
            ),
          } satisfies ProviderRailCapabilities,
        ];
      }),
    );

    return {
      mode: String(response.mode || "unknown"),
      blockedCountries: Array.isArray(response.blockedCountries)
        ? response.blockedCountries.map(String)
        : Array.isArray(response.blocked_countries)
          ? response.blocked_countries.map(String)
          : [],
      writesRestricted: Boolean(
        response.writesRestricted ?? response.writes_restricted,
      ),
      country: response.country || undefined,
      regionClass: String(
        response.regionClass ?? response.region_class ?? "unknown",
      ),
      routingMode: String(
        response.routingMode ?? response.routing_mode ?? "unknown",
      ),
      rails,
      legacyCloseOnly: Boolean(
        response.legacyCloseOnly ?? response.legacy_close_only,
      ),
    };
  }

  async getWeb4Capabilities(): Promise<Web4Capabilities> {
    const capabilities =
      await this.requestBaseRead<Web4Capabilities>("/web4/capabilities");
    this.setCapabilities(capabilities);
    return capabilities;
  }

  async getBaseMarkets(params?: {
    limit?: number;
    offset?: number;
    source?: "all" | "internal" | "limitless" | "polymarket" | "aerodrome";
    tradable?: "all" | "user" | "agent";
    includeLowLiquidity?: boolean;
  }): Promise<PaginatedResponse<Market>> {
    const query = this.buildQuery(params || {});
    const response = await this.requestBaseRead<BaseMarketsResponse>(
      `/evm/markets${query}`,
    );
    return normalizeBaseMarketsResponse(response);
  }

  async getBaseOrderBook(
    marketId: string,
    outcome: Outcome,
    depth = 20,
  ): Promise<OrderBook> {
    const query = this.buildQuery({ outcome, depth });
    const encodedMarketId = encodeURIComponent(marketId);
    const response = await this.requestBaseRead<BaseOrderBookResponse>(
      `/evm/markets/${encodedMarketId}/orderbook${query}`,
    );
    return normalizeBaseOrderBookResponse(response);
  }

  async getBaseTrades(
    marketId: string,
    params?: {
      outcome?: Outcome;
      limit?: number;
      before?: string;
      offset?: number;
    },
  ): Promise<PaginatedResponse<Trade>> {
    const query = this.buildQuery({
      outcome: params?.outcome,
      limit: params?.limit,
      offset: params?.offset,
    });
    const encodedMarketId = encodeURIComponent(marketId);
    const response = await this.requestBaseRead<BaseTradesResponse>(
      `/evm/markets/${encodedMarketId}/trades${query}`,
    );
    const data = (response.trades ?? []).map((trade) =>
      mapBaseTradeToTrade(trade, {
        provider: response.provider,
        chain_id: response.chain_id,
        provider_market_ref: response.provider_market_ref,
        is_synthetic: response.is_synthetic,
      }),
    );
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
    const response = await this.requestBaseRead<BaseMarketSnapshot>(
      `/evm/markets/${encodeURIComponent(id)}`,
    );
    return mapBaseSnapshotToMarket(response);
  }

  async registerBaseMarketBootstrap(
    marketId: string,
    data: {
      txHash: string;
      liquidityMode: "clob_only" | "bootstrap_hybrid";
      seedUsdc: number;
      initialYesBps: number;
      manager?: string;
      preset?: "tight" | "balanced" | "wide";
    },
  ) {
    return this.request(
      `/evm/internal/markets/${encodeURIComponent(marketId)}/bootstrap`,
      {
        method: "POST",
        body: JSON.stringify(data),
      },
    );
  }

  async getBootstrapOperatorStatus(
    owner: string,
  ): Promise<BootstrapOperatorStatus> {
    const query = this.buildQuery({ owner });
    return this.request<BootstrapOperatorStatus>(`/evm/bootstrap/operator${query}`);
  }

  async pauseBaseMarketBootstrap(marketId: string) {
    return this.request(
      `/evm/internal/markets/${encodeURIComponent(marketId)}/bootstrap/pause`,
      {
        method: "POST",
        body: JSON.stringify({}),
      },
    );
  }

  async resumeBaseMarketBootstrap(marketId: string) {
    return this.request(
      `/evm/internal/markets/${encodeURIComponent(marketId)}/bootstrap/resume`,
      {
        method: "POST",
        body: JSON.stringify({}),
      },
    );
  }

  async refreshBaseMarketBootstrap(marketId: string) {
    return this.request(
      `/evm/internal/markets/${encodeURIComponent(marketId)}/bootstrap/refresh`,
      {
        method: "POST",
        body: JSON.stringify({}),
      },
    );
  }

  async graduateBaseMarketBootstrap(marketId: string) {
    return this.request(
      `/evm/internal/markets/${encodeURIComponent(marketId)}/bootstrap/graduate`,
      {
        method: "POST",
        body: JSON.stringify({}),
      },
    );
  }

  async getBaseAgents(
    filters?: AgentFilters,
  ): Promise<PaginatedResponse<Agent>> {
    const query = this.buildQuery({
      limit: filters?.limit,
      offset: filters?.offset,
      owner: filters?.owner,
      market_id: filters?.marketId,
      active: filters?.active,
    });
    const response = await this.requestBaseRead<BaseAgentsResponse>(
      `/evm/agents${query}`,
    );
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
      throw new ApiError(404, "Agent not found");
    }
    const response = await this.requestBaseRead<BaseAgentSnapshot>(
      `/evm/agents/${parsedId}`,
    );
    return mapBaseAgentToAgent(response);
  }

  async getBaseTokenState(): Promise<BaseTokenState> {
    return this.request("/evm/token/state");
  }

  async getBaseValidationStatus(
    requestHash: string,
  ): Promise<BaseValidationStatus> {
    return this.request(`/evm/validation/${encodeURIComponent(requestHash)}`);
  }

  async getExternalCredentials(
    provider?: "limitless" | "polymarket" | "aerodrome",
  ): Promise<ExternalCredential[]> {
    const query = this.buildQuery({ provider });
    const response = await this.request<ExternalCredentialsListResponse>(
      `/external/credentials${query}`,
    );
    return response.credentials ?? [];
  }

  async getExternalCredentialStatus(
    provider: "limitless" | "polymarket" | "aerodrome",
    credentialId?: string,
  ): Promise<ExternalCredentialStatus> {
    const query = this.buildQuery({ provider, credentialId });
    return this.request(`/external/credentials/status${query}`);
  }

  async upsertExternalCredential(data: {
    provider: "limitless" | "polymarket" | "aerodrome";
    label?: string;
    credentials: Record<string, unknown>;
  }): Promise<ExternalCredential> {
    return this.request("/external/credentials", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async deleteExternalCredential(
    credentialId: string,
  ): Promise<{ ok: boolean }> {
    return this.request(`/external/credentials/${credentialId}`, {
      method: "DELETE",
    });
  }

  async bindLimitlessWallet(data: {
    credentialId: string;
    baseWallet: string;
  }): Promise<ExternalCredentialStatus> {
    return this.request("/external/credentials/limitless/wallet-bind", {
      method: "POST",
      body: JSON.stringify({
        credentialId: data.credentialId,
        baseWallet: data.baseWallet,
      }),
    });
  }

  async createExternalOrderIntent(data: {
    provider: "limitless" | "polymarket" | "aerodrome";
    marketId: string;
    outcome: "yes" | "no";
    side: "buy" | "sell";
    price: number;
    quantity: number;
    credentialId?: string;
  }): Promise<ExternalOrderIntent> {
    return this.request("/external/orders/intent", {
      method: "POST",
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
    providerResponse?: Record<string, unknown>;
    providerStatus?: number;
  }): Promise<ExternalOrderRecord> {
    const response = await this.request<Record<string, unknown>>(
      "/external/orders/submit",
      {
        method: "POST",
        body: JSON.stringify({
          intentId: data.intentId,
          signedOrder: data.signedOrder,
          credentialId: data.credentialId,
          providerResponse: data.providerResponse,
          providerStatus: data.providerStatus,
        }),
      },
    );
    return normalizeExternalOrderRecord(response);
  }

  async prepareExternalOrderSubmit(data: {
    intentId: string;
    signedOrder: Record<string, unknown>;
    credentialId?: string;
  }): Promise<PreparedExternalProviderRequest> {
    return this.request("/external/orders/prepare-submit", {
      method: "POST",
      body: JSON.stringify({
        intentId: data.intentId,
        signedOrder: data.signedOrder,
        credentialId: data.credentialId,
      }),
    });
  }

  async cancelExternalOrder(data: {
    provider: "limitless" | "polymarket" | "aerodrome";
    providerOrderId: string;
    credentialId?: string;
    payload?: Record<string, unknown>;
    providerResponse?: Record<string, unknown>;
    providerStatus?: number;
  }): Promise<{ ok: boolean }> {
    return this.request("/external/orders/cancel", {
      method: "POST",
      body: JSON.stringify({
        provider: data.provider,
        providerOrderId: data.providerOrderId,
        credentialId: data.credentialId,
        payload: data.payload,
        providerResponse: data.providerResponse,
        providerStatus: data.providerStatus,
      }),
    });
  }

  async prepareExternalOrderCancel(data: {
    provider: "limitless" | "polymarket" | "aerodrome";
    providerOrderId: string;
    credentialId?: string;
    payload?: Record<string, unknown>;
  }): Promise<PreparedExternalProviderRequest> {
    return this.request("/external/orders/prepare-cancel", {
      method: "POST",
      body: JSON.stringify({
        provider: data.provider,
        providerOrderId: data.providerOrderId,
        credentialId: data.credentialId,
        payload: data.payload,
      }),
    });
  }

  async listExternalOrders(params?: {
    provider?: "limitless" | "polymarket" | "aerodrome";
    limit?: number;
    offset?: number;
  }): Promise<ExternalOrdersListResponse> {
    const query = this.buildQuery(params || {});
    const response = await this.request<Record<string, unknown>>(
      `/external/orders${query}`,
    );
    const orders = Array.isArray(response.orders)
      ? response.orders
          .filter(
            (entry): entry is Record<string, unknown> =>
              !!entry && typeof entry === "object",
          )
          .map((entry) => normalizeExternalOrderRecord(entry))
      : [];

    return {
      orders,
      total: toNumber(response.total),
      limit: toNumber(response.limit),
      offset: toNumber(response.offset),
    };
  }

  async listExternalAgents(params?: {
    provider?: "limitless" | "polymarket" | "aerodrome";
    active?: boolean;
    limit?: number;
    offset?: number;
  }): Promise<ExternalAgentsListResponse> {
    const query = this.buildQuery(params || {});
    const response = await this.request<ExternalAgentsListResponse>(
      `/external/agents${query}`,
    );
    const agents = Array.isArray(response.agents) ? response.agents : [];
    return {
      agents: agents.map((entry) =>
        normalizeExternalAgentRecord(
          entry as unknown as Record<string, unknown>,
        ),
      ),
      total: toNumber(response.total),
      limit: toNumber(response.limit),
      offset: toNumber(response.offset),
    };
  }

  async listPublicExternalAgents(params?: {
    provider?: "limitless" | "polymarket" | "aerodrome";
    active?: boolean;
    limit?: number;
    offset?: number;
  }): Promise<ExternalAgentsListResponse> {
    const query = this.buildQuery(params || {});
    const response = await this.request<ExternalAgentsListResponse>(
      `/external/agents/public${query}`,
    );
    const agents = Array.isArray(response.agents) ? response.agents : [];
    return {
      agents: agents.map((entry) =>
        normalizeExternalAgentRecord(
          entry as unknown as Record<string, unknown>,
        ),
      ),
      total: toNumber(response.total),
      limit: toNumber(response.limit),
      offset: toNumber(response.offset),
    };
  }

  async getPublicExternalAgentsPerformance(): Promise<ExternalAgentPerformanceResponse> {
    const response = await this.request<Record<string, unknown>>(
      "/external/agents/public/performance",
    );
    return normalizeExternalAgentPerformanceResponse(response);
  }

  async createExternalAgent(data: {
    name: string;
    provider: "limitless" | "polymarket" | "aerodrome";
    marketId: string;
    outcome: "yes" | "no";
    side: "buy" | "sell";
    price: number;
    quantity: number;
    cadenceSeconds: number;
    strategy: string;
    credentialId?: string;
    executionMode?: "live" | "paper";
    active?: boolean;
  }): Promise<ExternalAgentRecord> {
    const response = await this.request<Record<string, unknown>>(
      "/external/agents",
      {
        method: "POST",
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
          executionMode: data.executionMode,
          active: data.active,
        }),
      },
    );
    return normalizeExternalAgentRecord(response);
  }

  async updateExternalAgent(
    agentId: string,
    data: Partial<{
      name: string;
      outcome: "yes" | "no";
      side: "buy" | "sell";
      price: number;
      quantity: number;
      cadenceSeconds: number;
      strategy: string;
      credentialId: string;
      executionMode: "live" | "paper";
      active: boolean;
    }>,
  ): Promise<ExternalAgentRecord> {
    const response = await this.request<Record<string, unknown>>(
      `/external/agents/${agentId}`,
      {
        method: "PATCH",
        body: JSON.stringify(data),
      },
    );
    return normalizeExternalAgentRecord(response);
  }

  async executeExternalAgent(
    agentId: string,
    data?: { force?: boolean; signedOrder?: Record<string, unknown> },
  ): Promise<Record<string, unknown>> {
    return this.request(`/external/agents/${agentId}/execute`, {
      method: "POST",
      body: JSON.stringify(data || {}),
    });
  }

  async listDecisionCells(params?: {
    limit?: number;
    offset?: number;
    status?: string;
  }): Promise<PaginatedResponse<DecisionCellListItem>> {
    const query = this.buildQuery(params || {});
    const response = await this.request<DecisionCellsListResponse>(
      `/decisions${query}`,
    );
    return {
      data: response.data,
      total: response.total,
      limit: response.limit,
      offset: response.offset,
      hasMore: response.has_more,
    };
  }

  async createDecisionCell(
    data: CreateDecisionCellRequest,
  ): Promise<DecisionCell> {
    return this.request("/decisions", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async getDecisionCell(cellId: string): Promise<DecisionCell> {
    return this.request(`/decisions/${encodeURIComponent(cellId)}`);
  }

  async updateDecisionCell(
    cellId: string,
    data: UpdateDecisionCellRequest,
  ): Promise<DecisionCell> {
    return this.request(`/decisions/${encodeURIComponent(cellId)}`, {
      method: "PATCH",
      body: JSON.stringify(data),
    });
  }

  async addDecisionAction(
    cellId: string,
    label: string,
  ): Promise<DecisionCell> {
    return this.request(`/decisions/${encodeURIComponent(cellId)}/actions`, {
      method: "POST",
      body: JSON.stringify({ label }),
    });
  }

  async addDecisionNode(
    cellId: string,
    data: CreateDecisionNodeRequest,
  ): Promise<DecisionCell> {
    return this.request(`/decisions/${encodeURIComponent(cellId)}/nodes`, {
      method: "POST",
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
        method: "PATCH",
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
        method: "POST",
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
        method: "POST",
        body: JSON.stringify(data),
      },
    );
  }

  async recalculateDecisionCell(cellId: string): Promise<DecisionCell> {
    return this.request(
      `/decisions/${encodeURIComponent(cellId)}/recalculate`,
      {
        method: "POST",
        body: JSON.stringify({}),
      },
    );
  }

  async updateDecisionAutomation(
    cellId: string,
    data: UpdateDecisionAutomationRequest,
  ): Promise<DecisionCell> {
    return this.request(`/decisions/${encodeURIComponent(cellId)}/automation`, {
      method: "POST",
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
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async getDecisionEvents(cellId: string): Promise<DecisionCell["events"]> {
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
    return this.request("/evm/write/markets/create", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareBaseSetManagerApproval(data: {
    from?: string;
    manager: string;
    approved: boolean;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/agents/manager-approval", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareBaseBootstrapCreateAgents(data: {
    from?: string;
    owner: string;
    manager: string;
    strategy: string;
    agents: Array<{
      marketId: number;
      isYes: boolean;
      priceBps: number;
      size: string;
      cadence: number;
      expiryWindow: number;
    }>;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/agents/bootstrap-create", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareBaseUpdateAgents(data: {
    from?: string;
    strategy: string;
    updates: Array<{
      agentId: number;
      isYes: boolean;
      priceBps: number;
      size: string;
      cadence: number;
      expiryWindow: number;
    }>;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/agents/update", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareBaseDeactivateAgents(data: {
    from?: string;
    agentIds: number[];
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/agents/deactivate", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareBaseSetAgentManager(data: {
    from?: string;
    agentId: number;
    manager: string;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/agents/manager", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareBaseResolveMarket(data: {
    from?: string;
    marketId: number;
    outcome: boolean;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/markets/resolve", {
      method: "POST",
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
    return this.request("/evm/write/orders/place", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareBaseCancelOrder(data: {
    from?: string;
    orderId: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/orders/cancel", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareBaseClaim(data: {
    from?: string;
    marketId: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/positions/claim", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareBaseClaimFor(data: {
    from?: string;
    user: string;
    marketId: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/positions/claim-for", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareBaseMatchOrders(data: {
    from?: string;
    firstOrderId: number;
    secondOrderId: number;
    fillSize: string;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/orders/match", {
      method: "POST",
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
    return this.request("/evm/write/agents/create", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareBaseExecuteAgent(data: {
    from?: string;
    agentId: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/agents/execute", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareBaseRegisterIdentity(data: {
    from?: string;
    wallet: string;
    tier: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/identity/register", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareBaseSetIdentityTier(data: {
    from?: string;
    wallet: string;
    tier: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/identity/tier", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareBaseSetIdentityActive(data: {
    from?: string;
    wallet: string;
    active: boolean;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/identity/active", {
      method: "POST",
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
    return this.request("/evm/write/reputation/outcome", {
      method: "POST",
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
    return this.request("/evm/write/validation/request", {
      method: "POST",
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
    return this.request("/evm/write/validation/response", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async relayBaseRawTransaction(rawTx: string): Promise<RelayRawTxResponse> {
    return this.request("/evm/write/relay", {
      method: "POST",
      body: JSON.stringify({ rawTx }),
    });
  }

  async getDepositAddress(): Promise<DepositAddress> {
    return this.request("/wallet/deposit/address");
  }

  async deposit(data: DepositRequest): Promise<DepositResponse> {
    return this.request("/wallet/deposit", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async withdraw(data: WithdrawRequest): Promise<WithdrawResponse> {
    return this.request("/wallet/withdraw", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  // Auth
  async getNonce(): Promise<string> {
    return this.getSiweNonce();
  }

  async getSiweNonce(): Promise<string> {
    const res = await this.request<{ nonce: string }>(
      "/auth/siwe/nonce",
      {},
      true,
    );
    return res.nonce;
  }

  async getSolanaNonce(): Promise<string> {
    const res = await this.request<{ nonce: string }>(
      "/auth/solana/nonce",
      {},
      true,
    );
    return res.nonce;
  }

  async getFarcasterNonce(): Promise<string> {
    const res = await this.request<{ nonce: string }>(
      "/auth/farcaster/nonce",
      {},
      true,
    );
    return res.nonce;
  }

  async login(
    wallet: string,
    signature: string,
    message: string,
  ): Promise<{ accessToken: string; expiresAt: number }> {
    return this.loginSiwe(wallet, signature, message);
  }

  async loginSiwe(
    wallet: string,
    signature: string,
    message: string,
  ): Promise<{ accessToken: string; expiresAt: number }> {
    const res = await fetch("/api/auth", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ wallet, signature, message, flow: "siwe" }),
    });

    if (!res.ok) {
      const data = await res.json();
      throw new ApiError(res.status, data.error || "SIWE login failed");
    }

    const data = await res.json();
    this.setAccessToken(data.accessToken, data.expiresAt);
    return data;
  }

  async loginSolana(
    wallet: string,
    signature: string,
    message: string,
  ): Promise<{ accessToken: string; expiresAt: number }> {
    const res = await fetch("/api/auth", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ wallet, signature, message, flow: "solana" }),
    });

    if (!res.ok) {
      const data = await res.json();
      throw new ApiError(res.status, data.error || "Solana login failed");
    }

    const data = await res.json();
    this.setAccessToken(data.accessToken, data.expiresAt);
    return data;
  }

  async loginFarcaster(
    message: string,
    signature: string,
    nonce: string,
  ): Promise<{ accessToken: string; expiresAt: number }> {
    const res = await fetch("/api/auth", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ message, signature, nonce, flow: "farcaster" }),
    });

    if (!res.ok) {
      const data = await res.json();
      throw new ApiError(res.status, data.error || "Farcaster login failed");
    }

    const data = await res.json();
    this.setAccessToken(data.accessToken, data.expiresAt);
    return data;
  }

  async post<T = unknown>(
    path: string,
    body: Record<string, unknown>,
  ): Promise<T> {
    const res = await fetch(`${PRIMARY_API_BASE}${path}`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(this.accessToken
          ? { Authorization: `Bearer ${this.accessToken}` }
          : {}),
      },
      body: JSON.stringify(body),
    });

    if (!res.ok) {
      const data = await res.json().catch(() => ({}));
      throw new ApiError(
        res.status,
        (data as Record<string, string>).error || "Request failed",
      );
    }

    return res.json();
  }

  async refresh(): Promise<{ accessToken: string; expiresAt: number }> {
    const res = await fetch("/api/auth", { method: "PUT" });

    if (!res.ok) {
      this.clearAccessToken();
      throw new ApiError(res.status, "Token refresh failed");
    }

    const data = await res.json();
    this.setAccessToken(data.accessToken, data.expiresAt);
    return data;
  }

  async logout(): Promise<void> {
    try {
      await fetch("/api/auth", { method: "DELETE" });
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
    return this.request("/notifications/unread-count");
  }

  async markAsRead(notificationId: string): Promise<void> {
    return this.request(`/notifications/${notificationId}/read`, {
      method: "PUT",
    });
  }

  async markAllAsRead(): Promise<void> {
    return this.request("/notifications/read-all", {
      method: "PUT",
    });
  }

  async getNotificationPreferences(): Promise<NotificationPreferences> {
    return this.request("/notifications/preferences");
  }

  async updateNotificationPreferences(
    prefs: Partial<NotificationPreferences>,
  ): Promise<NotificationPreferences> {
    return this.request("/notifications/preferences", {
      method: "PUT",
      body: JSON.stringify(prefs),
    });
  }

  // Leaderboards
  async getLeaderboard(
    period: LeaderboardPeriod = "weekly",
    metric: LeaderboardMetric = "pnl",
    limit = 100,
  ): Promise<Leaderboard> {
    return this.request(
      `/leaderboard?period=${period}&metric=${metric}&limit=${limit}`,
    );
  }

  async getUserRank(
    wallet: string,
    period: LeaderboardPeriod = "weekly",
    metric: LeaderboardMetric = "pnl",
  ): Promise<{ rank: number; value: number }> {
    return this.request(
      `/leaderboard/rank/${wallet}?period=${period}&metric=${metric}`,
    );
  }

  // Public profiles
  async getPublicProfile(wallet: string): Promise<PublicProfile> {
    return this.request(`/profiles/${wallet}`);
  }

  async getProfileActivity(
    wallet: string,
    params?: { limit?: number; offset?: number },
  ): Promise<PaginatedResponse<ProfileActivity>> {
    const query = this.buildQuery(params || {});
    return this.request(`/profiles/${wallet}/activity${query}`);
  }

  async getProfilePositions(
    wallet: string,
  ): Promise<PaginatedResponse<Position>> {
    return this.request(`/profiles/${wallet}/positions`);
  }

  // Hackathons
  async getHackathons(params?: {
    status?: string;
    limit?: number;
    offset?: number;
  }): Promise<{ hackathons: Hackathon[]; total: number; limit: number; offset: number }> {
    const query = this.buildQuery(params || {});
    return this.request(`/hackathons${query}`);
  }

  async getHackathon(id: string): Promise<Hackathon> {
    return this.request(`/hackathons/${id}`);
  }

  async createHackathon(data: {
    name: string;
    description: string;
    prizePoolUsdc: number;
    startTime: string;
    endTime: string;
    scoringMethod?: string;
    rulesJson?: Record<string, unknown>;
  }): Promise<Hackathon> {
    return this.request("/hackathons", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async updateHackathon(
    id: string,
    data: Partial<{
      name: string;
      description: string;
      status: string;
      prizePoolUsdc: number;
      startTime: string;
      endTime: string;
      rulesJson: Record<string, unknown>;
    }>,
  ): Promise<Hackathon> {
    return this.request(`/hackathons/${id}`, {
      method: "PATCH",
      body: JSON.stringify(data),
    });
  }

  async registerForHackathon(
    id: string,
    data?: { identityId?: string },
  ): Promise<void> {
    return this.request(`/hackathons/${id}/register`, {
      method: "POST",
      body: JSON.stringify(data ?? {}),
    });
  }

  async getHackathonRegistrations(id: string): Promise<{
    registrations: HackathonRegistration[];
    total: number;
  }> {
    return this.request(`/hackathons/${id}/registrations`);
  }

  async linkAgentToHackathon(
    hackathonId: string,
    agentId: string,
  ): Promise<void> {
    return this.request(`/hackathons/${hackathonId}/agents`, {
      method: "POST",
      body: JSON.stringify({ agentId }),
    });
  }

  async getHackathonLeaderboard(
    id: string,
    params?: { limit?: number; offset?: number },
  ): Promise<HackathonLeaderboard> {
    const query = this.buildQuery(params || {});
    return this.request(`/hackathons/${id}/leaderboard${query}`);
  }

  async getHackathonSnapshots(
    id: string,
    params?: { walletAddress?: string; limit?: number },
  ): Promise<{ snapshots: HackathonSnapshot[] }> {
    const query = this.buildQuery(params || {});
    return this.request(`/hackathons/${id}/leaderboard/snapshots${query}`);
  }

  async triggerHackathonSnapshot(
    id: string,
  ): Promise<{ snapshotCount: number }> {
    return this.request(`/hackathons/${id}/snapshot`, { method: "POST" });
  }

  // Identity (ERC-8004)
  async getIdentity(wallet: string): Promise<{
    wallet: string;
    tier: number;
    active: boolean;
    token_id?: number;
  }> {
    return this.request(`/evm/identity/${encodeURIComponent(wallet)}`);
  }

  // x402 payments
  async getX402Quote(
    resource: "orderbook" | "trades" | "mcp_tool_call",
  ): Promise<PaymentRequired> {
    return this.request(`/payments/x402/quote?resource=${encodeURIComponent(resource)}`);
  }

  // Swarm messaging (XMTP bridge)
  async getSwarmMessages(
    swarmId: string,
    params?: { limit?: number; cursor?: string },
  ): Promise<{
    data: Array<{
      id: string;
      sender: string;
      content: string;
      sentAt: string;
    }>;
    cursor?: string;
    has_more: boolean;
  }> {
    const query = this.buildQuery(params || {});
    const response = await this.request<SwarmMessagesResponse>(
      `/web4/xmtp/swarm/${encodeURIComponent(swarmId)}/messages${query}`,
    );
    const data = response.data ?? [];

    return {
      data: data.map((message) => ({
        id: message.id,
        sender: message.sender,
        content: message.message,
        sentAt: toIsoString(message.created_at),
      })),
      has_more: data.length >= (params?.limit ?? response.limit ?? data.length),
    };
  }

  // Oracle resolver

  async getOracleMarketConfig(marketId: number | string): Promise<import("@/types").OracleMarketConfig | null> {
    return this.request(`/evm/oracle/markets/${marketId}/config`);
  }

  async registerOracleMarketConfig(
    marketId: number | string,
    data: {
      feedType: string;
      feedAddress?: string;
      comparison: string;
      targetValue: string;
      targetCurrency?: string;
      category?: string;
      resolutionHint?: string;
      keeperEnabled?: boolean;
    },
  ): Promise<import("@/types").OracleMarketConfig> {
    return this.request(`/evm/oracle/markets/${marketId}/config`, {
      method: "POST",
      body: JSON.stringify({ marketId: Number(marketId), ...data }),
    });
  }

  async prepareConfigureOracle(data: {
    from?: string;
    marketId: number;
    feedType: number;
    feedAddress: string;
    comparison: number;
    targetValue: string;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/oracle/configure", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async prepareOracleResolve(data: {
    from?: string;
    marketId: number;
  }): Promise<PreparedEvmWriteTx> {
    return this.request("/evm/write/oracle/resolve", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  // KYC verification

  async verifyKyc(data: {
    merkle_root: string;
    nullifier_hash: string;
    proof: string;
    action_id: string;
    signal: string;
  }): Promise<{ tier: number; tierLabel: string }> {
    return this.request("/kyc/verify", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async getKycStatus(): Promise<import("@/types").KycStatus> {
    return this.request("/kyc/status");
  }

  // Social: follows

  async followTrader(wallet: string): Promise<{ ok: boolean }> {
    return this.request(`/social/follow/${wallet}`, { method: "POST" });
  }

  async unfollowTrader(wallet: string): Promise<{ ok: boolean }> {
    return this.request(`/social/follow/${wallet}`, { method: "DELETE" });
  }

  async getFollowing(params?: { limit?: number; offset?: number }): Promise<{
    data: Array<{ wallet: string; username?: string; followedAt: string }>;
    total: number;
  }> {
    const query = this.buildQuery(params || {});
    return this.request(`/social/following${query}`);
  }

  async getFollowers(params?: { limit?: number; offset?: number }): Promise<{
    data: Array<{ wallet: string; username?: string; followedAt: string }>;
    total: number;
  }> {
    const query = this.buildQuery(params || {});
    return this.request(`/social/followers${query}`);
  }

  async getFollowerCounts(wallet: string): Promise<import("@/types").FollowerCounts> {
    return this.request(`/profiles/${wallet}/followers-count`);
  }

  async getFollowStatus(wallet: string): Promise<{ following: boolean }> {
    return this.request(`/social/follow/${wallet}/status`);
  }

  async getSocialFeed(params?: { limit?: number; offset?: number }): Promise<{
    data: import("@/types").ProfileActivity[];
    total: number;
    hasMore: boolean;
  }> {
    const query = this.buildQuery(params || {});
    return this.request(`/social/feed${query}`);
  }

  // Social: profile edit

  async updateProfile(data: {
    username?: string;
    bio?: string;
    avatarUrl?: string;
    websiteUrl?: string;
    twitterHandle?: string;
  }): Promise<{ ok: boolean }> {
    return this.request("/profiles/me", {
      method: "PATCH",
      body: JSON.stringify(data),
    });
  }

  // Social: market comments

  async getMarketComments(
    marketId: string,
    params?: { limit?: number; offset?: number },
  ): Promise<{
    data: import("@/types").MarketComment[];
    total: number;
    hasMore: boolean;
  }> {
    const query = this.buildQuery(params || {});
    return this.request(`/social/markets/${marketId}/comments${query}`);
  }

  async postMarketComment(
    marketId: string,
    data: { text: string; parentId?: string },
  ): Promise<import("@/types").MarketComment> {
    return this.request(`/social/markets/${marketId}/comments`, {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  // Social: copy trading

  async startCopyTrading(
    targetWallet: string,
    data: { allocationUsdc?: number; maxPositionUsdc?: number },
  ): Promise<import("@/types").CopyTradingSubscription> {
    return this.request(`/social/copy/${targetWallet}`, {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async stopCopyTrading(targetWallet: string): Promise<{ ok: boolean }> {
    return this.request(`/social/copy/${targetWallet}`, { method: "DELETE" });
  }

  async getCopySubscriptions(): Promise<{
    data: import("@/types").CopyTradingSubscription[];
  }> {
    return this.request("/social/copy");
  }

  async getCopyStats(wallet: string): Promise<{
    copySubscriberCount: number;
    totalCopyAumUsdc: number;
  }> {
    return this.request(`/profiles/${wallet}/copy-stats`);
  }

  // Social: signals

  async publishSignal(data: {
    marketId: string;
    direction: import("@/types").SignalDirection;
    confidenceBps: number;
    rationale?: string;
    validUntilHours?: number;
  }): Promise<import("@/types").TradingSignal> {
    return this.request("/signals", {
      method: "POST",
      body: JSON.stringify(data),
    });
  }

  async getSignals(params?: {
    marketId?: string;
    publisher?: string;
    active?: boolean;
    limit?: number;
    offset?: number;
  }): Promise<{
    data: import("@/types").TradingSignal[];
    total: number;
    hasMore: boolean;
  }> {
    const query = this.buildQuery(params || {});
    return this.request(`/signals${query}`);
  }

  async getMarketSignals(marketId: string): Promise<{
    data: import("@/types").TradingSignal[];
  }> {
    return this.request(`/markets/${marketId}/signals`);
  }

  async subscribeToSignals(publisher: string): Promise<{ ok: boolean }> {
    return this.request(`/signals/subscribe/${publisher}`, { method: "POST" });
  }

  async unsubscribeFromSignals(publisher: string): Promise<{ ok: boolean }> {
    return this.request(`/signals/subscribe/${publisher}`, { method: "DELETE" });
  }

  async getSignalFeed(params?: { limit?: number; offset?: number }): Promise<{
    data: import("@/types").TradingSignal[];
    total: number;
    hasMore: boolean;
  }> {
    const query = this.buildQuery(params || {});
    return this.request(`/signals/feed${query}`);
  }
}

export const api = new ApiClient();
