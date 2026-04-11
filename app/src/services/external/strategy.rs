use crate::api::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct MarketState {
    pub yes_price: f64,
    pub no_price: f64,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub mid_price: f64,
    pub agent_price: f64,
    pub agent_side: String,
    pub agent_outcome: String,
    pub agent_quantity: f64,
    pub time_to_resolution_seconds: Option<i64>,
    pub fair_value_low: Option<f64>,
    pub fair_value_high: Option<f64>,
    pub midpoint_delta_bps: Option<i32>,
    pub signal_source_count: u64,
    pub signal_resolution_rules_read: bool,
    pub signal_has_live_reference: bool,
    pub signal_resolution_hazard_count: u64,
    pub reference_price: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSignal {
    pub execute: bool,
    pub price: f64,
    pub quantity: f64,
    pub reason: String,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct MarketMakerParams {
    #[serde(default = "default_one_tick")]
    quote_improvement_ticks: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct MakerRewardParams {
    #[serde(default = "default_true")]
    fee_enabled: bool,
    #[serde(default = "default_true")]
    rebate_eligible: bool,
    #[serde(default)]
    allow_fee_free: bool,
    #[serde(default = "default_min_spread_bps")]
    min_spread_bps: i32,
    #[serde(default = "default_zero_i32")]
    maker_rebate_bps: i32,
    #[serde(default = "default_one_tick")]
    quote_improvement_ticks: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct EventRepricingParams {
    #[serde(default = "default_min_hours_to_resolution")]
    min_hours_to_resolution: u64,
    #[serde(default = "default_signal_edge_bps")]
    min_edge_bps: i32,
    #[serde(default = "default_fee_buffer_bps")]
    fee_buffer_bps: i32,
    #[serde(default = "default_slippage_buffer_bps")]
    slippage_buffer_bps: i32,
    #[serde(default = "default_event_min_size_multiplier")]
    min_size_multiplier: f64,
    #[serde(default = "default_event_max_size_multiplier")]
    max_size_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct EventRepricingV2Params {
    #[serde(default)]
    signal_id: String,
    #[serde(default = "default_min_hours_to_resolution")]
    min_hours_to_resolution: u64,
    #[serde(default = "default_signal_edge_bps")]
    min_net_edge_bps: i32,
    #[serde(default = "default_fee_buffer_bps")]
    fee_buffer_bps: i32,
    #[serde(default = "default_slippage_buffer_bps")]
    slippage_buffer_bps: i32,
    #[serde(default = "default_max_slippage_bps")]
    max_slippage_bps: i32,
    #[serde(default = "default_size_frac_min")]
    size_frac_min: f64,
    #[serde(default = "default_size_frac_max")]
    size_frac_max: f64,
    #[serde(default = "default_ttl_minutes")]
    ttl_minutes: u64,
    #[serde(default = "default_min_signal_sources")]
    min_signal_sources: u64,
    #[serde(default = "default_true")]
    require_resolution_rules: bool,
    #[serde(default = "default_true")]
    require_live_reference: bool,
    #[serde(default)]
    max_resolution_hazards: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EventRepricingV2Requirements {
    pub min_hours_to_resolution: u64,
    pub min_signal_sources: u64,
    pub require_resolution_rules: bool,
    pub require_live_reference: bool,
    pub max_resolution_hazards: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct WalletFollowParams {
    target_wallet: String,
    #[serde(default = "default_wallet_latency_ms")]
    observed_detection_to_order_ms: i64,
    #[serde(default)]
    observed_slippage_ticks: f64,
    #[serde(default = "default_wallet_latency_ms")]
    max_detection_to_order_ms: i64,
    #[serde(default = "default_wallet_slippage_ticks")]
    max_slippage_ticks: f64,
    #[serde(default = "default_copy_size_multiplier")]
    copy_size_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct WalletFollowV2Params {
    target_wallet: String,
    #[serde(default = "default_follow_ratio")]
    follow_ratio: f64,
    #[serde(default = "default_wallet_score_min")]
    wallet_score_min: f64,
    #[serde(default)]
    wallet_score: f64,
    #[serde(default = "default_wallet_live_latency_ms")]
    max_detection_to_order_ms: i64,
    #[serde(default = "default_wallet_latency_ms")]
    observed_detection_to_order_ms: i64,
    #[serde(default = "default_wallet_slippage_ticks")]
    max_slippage_ticks: f64,
    #[serde(default)]
    observed_slippage_ticks: f64,
    #[serde(default = "default_max_concurrent_markets")]
    max_concurrent_markets: u64,
    #[serde(default)]
    concurrent_markets: u64,
    #[serde(default = "default_cooldown_seconds")]
    cooldown_seconds: u64,
    #[serde(default = "default_large_u64")]
    seconds_since_last_follow: u64,
    #[serde(default = "default_crowding_gate")]
    crowding_gate: f64,
    #[serde(default)]
    crowding_score: f64,
}

fn default_true() -> bool {
    true
}

fn default_zero_i32() -> i32 {
    0
}

fn default_one_tick() -> f64 {
    1.0
}

fn default_min_spread_bps() -> i32 {
    15
}

fn default_min_hours_to_resolution() -> u64 {
    24
}

fn default_signal_edge_bps() -> i32 {
    400
}

fn default_fee_buffer_bps() -> i32 {
    25
}

fn default_slippage_buffer_bps() -> i32 {
    25
}

fn default_event_min_size_multiplier() -> f64 {
    0.5
}

fn default_event_max_size_multiplier() -> f64 {
    1.0
}

fn default_max_slippage_bps() -> i32 {
    50
}

fn default_size_frac_min() -> f64 {
    0.25
}

fn default_size_frac_max() -> f64 {
    0.75
}

fn default_ttl_minutes() -> u64 {
    180
}

fn default_min_signal_sources() -> u64 {
    2
}

fn default_wallet_latency_ms() -> i64 {
    1_500
}

fn default_wallet_live_latency_ms() -> i64 {
    1_250
}

fn default_wallet_slippage_ticks() -> f64 {
    1.0
}

fn default_copy_size_multiplier() -> f64 {
    0.8
}

fn default_follow_ratio() -> f64 {
    0.8
}

fn default_wallet_score_min() -> f64 {
    0.55
}

fn default_max_concurrent_markets() -> u64 {
    3
}

fn default_cooldown_seconds() -> u64 {
    300
}

fn default_large_u64() -> u64 {
    u64::MAX
}

fn default_crowding_gate() -> f64 {
    0.75
}

// ── Polymarket Alpha strategies ──

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct LongshotHarvestParams {
    #[serde(default = "default_longshot_max_price")]
    max_implied_prob: f64,
    #[serde(default = "default_longshot_min_mispricing")]
    min_mispricing_pct: f64,
    #[serde(default)]
    historical_win_rate: f64,
    #[serde(default = "default_kelly_fraction")]
    kelly_fraction: f64,
    #[serde(default = "default_longshot_max_position")]
    max_position_pct: f64,
    #[serde(default = "default_longshot_max_category_exposure")]
    max_category_exposure_pct: f64,
    #[serde(default)]
    current_category_exposure_pct: f64,
    #[serde(default)]
    stop_loss_multiplier: f64,
    #[serde(default = "default_bankroll")]
    bankroll_usdc: f64,
    #[serde(default)]
    is_correlated: bool,
    #[serde(default)]
    drawdown_from_peak_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SpreadCaptureParams {
    #[serde(default = "default_spread_max_total_cost")]
    max_total_cost: f64,
    #[serde(default = "default_spread_min_profit_bps")]
    min_profit_bps: f64,
    #[serde(default = "default_spread_max_duration")]
    max_duration_minutes: u64,
    #[serde(default = "default_spread_refresh")]
    order_refresh_seconds: u64,
    #[serde(default)]
    inventory_limit: f64,
    #[serde(default)]
    current_yes_inventory: f64,
    #[serde(default)]
    current_no_inventory: f64,
    #[serde(default)]
    counterpart_best_bid: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct NearCertaintyParams {
    #[serde(default = "default_near_cert_min_price")]
    min_price: f64,
    #[serde(default = "default_near_cert_min_edge")]
    min_edge_bps: i32,
    #[serde(default)]
    max_hours_to_resolution: u64,
    #[serde(default = "default_kelly_fraction")]
    kelly_fraction: f64,
    #[serde(default = "default_near_cert_max_position")]
    max_position_pct: f64,
    #[serde(default)]
    signal_count: u64,
    #[serde(default = "default_near_cert_min_signals")]
    min_signal_count: u64,
    #[serde(default)]
    calibrated_probability: f64,
    #[serde(default = "default_bankroll")]
    bankroll_usdc: f64,
    #[serde(default)]
    is_correlated: bool,
    #[serde(default)]
    drawdown_from_peak_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CorrelationArbParams {
    #[serde(default)]
    related_market_id: String,
    #[serde(default)]
    related_market_price: f64,
    #[serde(default)]
    logical_constraint: String,
    #[serde(default = "default_arb_min_profit")]
    min_arb_profit_bps: f64,
    #[serde(default)]
    combined_cost: f64,
    #[serde(default = "default_arb_max_combined_cost")]
    max_combined_cost: f64,
}

fn default_longshot_max_price() -> f64 {
    0.10
}
fn default_longshot_min_mispricing() -> f64 {
    10.0
}
fn default_longshot_max_position() -> f64 {
    0.02
}
fn default_longshot_max_category_exposure() -> f64 {
    0.15
}
fn default_kelly_fraction() -> f64 {
    0.25
}
fn default_bankroll() -> f64 {
    1000.0
}
fn default_spread_max_total_cost() -> f64 {
    0.98
}
fn default_spread_min_profit_bps() -> f64 {
    200.0
}
fn default_spread_max_duration() -> u64 {
    60
}
fn default_spread_refresh() -> u64 {
    15
}
fn default_near_cert_min_price() -> f64 {
    0.90
}
fn default_near_cert_min_edge() -> i32 {
    100
}
fn default_near_cert_max_position() -> f64 {
    0.03
}
fn default_near_cert_min_signals() -> u64 {
    1
}
fn default_arb_min_profit() -> f64 {
    25.0
}
fn default_arb_max_combined_cost() -> f64 {
    0.99
}

fn normalized_strategy(strategy: &str) -> String {
    strategy.trim().to_ascii_lowercase().replace('_', "-")
}

fn parse_params<T>(raw: &Value) -> Result<T, ApiError>
where
    T: for<'de> Deserialize<'de> + Serialize + Default,
{
    let payload = if raw.is_null() {
        json!({})
    } else {
        raw.clone()
    };
    serde_json::from_value(payload)
        .map_err(|err| ApiError::bad_request("INVALID_STRATEGY_PARAMS", &err.to_string()))
}

pub fn validate_strategy_params(strategy: &str, raw: &Value) -> Result<Value, ApiError> {
    let normalized = normalized_strategy(strategy);
    let params = match normalized.as_str() {
        "market-maker" => serde_json::to_value(parse_params::<MarketMakerParams>(raw)?),
        "maker-reward" => serde_json::to_value(parse_params::<MakerRewardParams>(raw)?),
        "event-repricing" => serde_json::to_value(parse_params::<EventRepricingParams>(raw)?),
        "event-repricing-v2" => serde_json::to_value(parse_params::<EventRepricingV2Params>(raw)?),
        "wallet-follow" => {
            let parsed = parse_params::<WalletFollowParams>(raw)?;
            if parsed.target_wallet.trim().is_empty() {
                return Err(ApiError::bad_request(
                    "INVALID_STRATEGY_PARAMS",
                    "wallet_follow requires targetWallet",
                ));
            }
            serde_json::to_value(parsed)
        }
        "wallet-follow-v2" => {
            let parsed = parse_params::<WalletFollowV2Params>(raw)?;
            if parsed.target_wallet.trim().is_empty() {
                return Err(ApiError::bad_request(
                    "INVALID_STRATEGY_PARAMS",
                    "wallet_follow_v2 requires targetWallet",
                ));
            }
            serde_json::to_value(parsed)
        }
        "longshot-harvest" => serde_json::to_value(parse_params::<LongshotHarvestParams>(raw)?),
        "spread-capture" => serde_json::to_value(parse_params::<SpreadCaptureParams>(raw)?),
        "near-certainty" => serde_json::to_value(parse_params::<NearCertaintyParams>(raw)?),
        "correlation-arb" => {
            let parsed = parse_params::<CorrelationArbParams>(raw)?;
            if parsed.related_market_id.trim().is_empty() {
                return Err(ApiError::bad_request(
                    "INVALID_STRATEGY_PARAMS",
                    "correlation_arb requires relatedMarketId",
                ));
            }
            serde_json::to_value(parsed)
        }
        "momentum" | "mean-revert" | "default" | "" => Ok(json!({})),
        _ => Ok(raw.clone()),
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(params)
}

pub fn event_repricing_v2_requirements(
    raw: &Value,
) -> Result<EventRepricingV2Requirements, ApiError> {
    let params = parse_params::<EventRepricingV2Params>(raw)?;
    Ok(EventRepricingV2Requirements {
        min_hours_to_resolution: params.min_hours_to_resolution,
        min_signal_sources: params.min_signal_sources,
        require_resolution_rules: params.require_resolution_rules,
        require_live_reference: params.require_live_reference,
        max_resolution_hazards: params.max_resolution_hazards,
    })
}

pub fn evaluate_strategy(
    strategy: &str,
    state: &MarketState,
    strategy_params: &Value,
) -> TradeSignal {
    match normalized_strategy(strategy).as_str() {
        "momentum" => evaluate_momentum(state),
        "mean-revert" => evaluate_mean_revert(state),
        "market-maker" => evaluate_market_maker(state, strategy_params),
        "maker-reward" => evaluate_maker_reward(state, strategy_params),
        "event-repricing" => evaluate_event_repricing(state, strategy_params),
        "event-repricing-v2" => evaluate_event_repricing_v2(state, strategy_params),
        "wallet-follow" => evaluate_wallet_follow(state, strategy_params),
        "wallet-follow-v2" => evaluate_wallet_follow_v2(state, strategy_params),
        "longshot-harvest" => evaluate_longshot_harvest(state, strategy_params),
        "spread-capture" => evaluate_spread_capture(state, strategy_params),
        "near-certainty" => evaluate_near_certainty(state, strategy_params),
        "correlation-arb" => evaluate_correlation_arb(state, strategy_params),
        _ => evaluate_default(state),
    }
}

fn execute_signal(state: &MarketState, reason: String) -> TradeSignal {
    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: state.agent_quantity,
        reason,
        metadata: json!({}),
    }
}

fn skip_signal(state: &MarketState, reason: String) -> TradeSignal {
    TradeSignal {
        execute: false,
        price: state.agent_price,
        quantity: state.agent_quantity,
        reason,
        metadata: json!({}),
    }
}

fn evaluate_default(state: &MarketState) -> TradeSignal {
    execute_signal(state, "default: execute at configured price".to_string())
}

fn evaluate_momentum(state: &MarketState) -> TradeSignal {
    let is_buy = state.agent_side == "buy";
    let spread = if is_buy {
        state.agent_price - state.mid_price
    } else {
        state.mid_price - state.agent_price
    };

    if spread <= 0.0 {
        return skip_signal(
            state,
            format!("momentum: unfavorable spread ({spread:.4}), waiting"),
        );
    }

    let strength = (spread / 0.05).clamp(0.5, 1.5);
    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: state.agent_quantity * strength,
        reason: format!("momentum: favorable spread {spread:.4}, strength {strength:.2}x"),
        metadata: json!({ "strength": strength }),
    }
}

fn evaluate_mean_revert(state: &MarketState) -> TradeSignal {
    let deviation = state.mid_price - state.agent_price;
    let abs_deviation = deviation.abs();

    if abs_deviation < 0.02 {
        return skip_signal(
            state,
            format!("mean-revert: price within dead zone (deviation={deviation:.4})"),
        );
    }

    let is_buy = state.agent_side == "buy";
    let favorable = (is_buy && deviation < 0.0) || (!is_buy && deviation > 0.0);
    if !favorable {
        return skip_signal(
            state,
            format!(
                "mean-revert: deviation {deviation:.4} not favorable for {}",
                state.agent_side
            ),
        );
    }

    // Boost strength when oracle reference price confirms the reversion direction
    let oracle_boost = state
        .reference_price
        .map(|ref_price| {
            let ref_deviation = state.mid_price - ref_price;
            if (is_buy && ref_deviation > 0.0) || (!is_buy && ref_deviation < 0.0) {
                1.0 + (ref_deviation.abs() / 0.05).clamp(0.0, 0.5)
            } else {
                1.0
            }
        })
        .unwrap_or(1.0);

    let strength = ((abs_deviation / 0.05).clamp(0.5, 2.0) * oracle_boost).min(2.0);
    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: state.agent_quantity * strength,
        reason: format!("mean-revert: deviation {deviation:.4}, strength {strength:.2}x"),
        metadata: json!({
            "strength": strength,
            "referencePrice": state.reference_price,
            "oracleBoost": oracle_boost
        }),
    }
}

fn evaluate_market_maker(state: &MarketState, raw: &Value) -> TradeSignal {
    let params = parse_params::<MarketMakerParams>(raw).unwrap_or_default();
    let tick = 0.001 * params.quote_improvement_ticks.max(0.0);
    let (price, reason) = if state.agent_side == "buy" {
        let bid = state.best_bid.unwrap_or(state.mid_price - 0.01);
        let price = (bid + tick).min(state.agent_price);
        (
            price,
            format!("market-maker: bid at {price:.4} (book={bid:.4})"),
        )
    } else {
        let ask = state.best_ask.unwrap_or(state.mid_price + 0.01);
        let price = (ask - tick).max(state.agent_price);
        (
            price,
            format!("market-maker: ask at {price:.4} (book={ask:.4})"),
        )
    };

    TradeSignal {
        execute: true,
        price,
        quantity: state.agent_quantity,
        reason,
        metadata: json!({
            "quoteImprovementTicks": params.quote_improvement_ticks,
            "referencePrice": state.reference_price
        }),
    }
}

fn evaluate_maker_reward(state: &MarketState, raw: &Value) -> TradeSignal {
    let params = parse_params::<MakerRewardParams>(raw).unwrap_or_default();
    let spread_bps = book_spread_bps(state);

    if !params.fee_enabled && !params.allow_fee_free {
        return skip_signal(
            state,
            "maker_reward: fee-free market skipped without spread-only override".to_string(),
        );
    }
    if params.fee_enabled && !params.rebate_eligible {
        return skip_signal(
            state,
            "maker_reward: market not rebate eligible".to_string(),
        );
    }
    if spread_bps < f64::from(params.min_spread_bps) {
        return skip_signal(
            state,
            format!(
                "maker_reward: spread {:.1}bps below threshold {}bps",
                spread_bps, params.min_spread_bps
            ),
        );
    }

    let tick = 0.001 * params.quote_improvement_ticks.max(0.0);
    let (price, side_label) = if state.agent_side == "buy" {
        (
            (state.best_bid.unwrap_or(state.mid_price - 0.01) + tick).min(state.agent_price),
            "bid",
        )
    } else {
        (
            (state.best_ask.unwrap_or(state.mid_price + 0.01) - tick).max(state.agent_price),
            "ask",
        )
    };

    TradeSignal {
        execute: true,
        price,
        quantity: state.agent_quantity,
        reason: format!(
            "maker_reward: {side_label} at {price:.4}, spread {:.1}bps, rebate {}bps",
            spread_bps, params.maker_rebate_bps
        ),
        metadata: json!({
            "spreadBps": spread_bps,
            "makerRebateBps": params.maker_rebate_bps,
            "rebateEligible": params.rebate_eligible,
            "feeEnabled": params.fee_enabled
        }),
    }
}

fn evaluate_event_repricing(state: &MarketState, raw: &Value) -> TradeSignal {
    let params = parse_params::<EventRepricingParams>(raw).unwrap_or_default();
    let Some(low) = state.fair_value_low else {
        return skip_signal(
            state,
            "event_repricing: no active fair-value signal".to_string(),
        );
    };
    let Some(high) = state.fair_value_high else {
        return skip_signal(
            state,
            "event_repricing: no active fair-value signal".to_string(),
        );
    };
    let Some(time_to_resolution_seconds) = state.time_to_resolution_seconds else {
        return skip_signal(
            state,
            "event_repricing: market close window unavailable".to_string(),
        );
    };
    if time_to_resolution_seconds < (params.min_hours_to_resolution as i64 * 3600) {
        return skip_signal(
            state,
            format!(
                "event_repricing: market resolves too soon ({}h required)",
                params.min_hours_to_resolution
            ),
        );
    }

    let (fair_low, fair_high) = agent_probability_range(state, low, high);
    let fair_mid = (fair_low + fair_high) / 2.0;
    let edge_bps = ((fair_mid - state.mid_price).abs() * 10_000.0).round() as i32;
    let net_edge_bps = edge_bps - params.fee_buffer_bps - params.slippage_buffer_bps;
    if net_edge_bps < params.min_edge_bps {
        return skip_signal(
            state,
            format!(
                "event_repricing: net edge {}bps below threshold {}bps",
                net_edge_bps, params.min_edge_bps
            ),
        );
    }

    let favorable = if state.agent_side == "buy" {
        fair_mid > state.mid_price
    } else {
        fair_mid < state.mid_price
    };
    if !favorable {
        return skip_signal(
            state,
            "event_repricing: signal direction does not match configured side".to_string(),
        );
    }

    let min_size_multiplier = params.min_size_multiplier.clamp(0.1, 1.0);
    let max_size_multiplier = params.max_size_multiplier.clamp(min_size_multiplier, 1.0);
    let edge_window_bps = f64::from(params.min_edge_bps.max(1));
    let normalized_edge =
        (f64::from((net_edge_bps - params.min_edge_bps).max(0)) / edge_window_bps).clamp(0.0, 1.0);
    let size_multiplier =
        min_size_multiplier + normalized_edge * (max_size_multiplier - min_size_multiplier);

    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: state.agent_quantity * size_multiplier,
        reason: format!(
            "event_repricing: fair {:.4}-{:.4}, mid {:.4}, net edge {}bps",
            fair_low, fair_high, state.mid_price, net_edge_bps
        ),
        metadata: json!({
            "fairValueLow": fair_low,
            "fairValueHigh": fair_high,
            "midpointDeltaBps": state.midpoint_delta_bps,
            "netEdgeBps": net_edge_bps,
            "sizeMultiplier": size_multiplier
        }),
    }
}

fn evaluate_event_repricing_v2(state: &MarketState, raw: &Value) -> TradeSignal {
    let params = parse_params::<EventRepricingV2Params>(raw).unwrap_or_default();
    let Some(low) = state.fair_value_low else {
        return skip_signal(
            state,
            "event_repricing_v2: no active fair-value signal".to_string(),
        );
    };
    let Some(high) = state.fair_value_high else {
        return skip_signal(
            state,
            "event_repricing_v2: no active fair-value signal".to_string(),
        );
    };
    if params.require_resolution_rules && !state.signal_resolution_rules_read {
        return skip_signal(
            state,
            "event_repricing_v2: resolution rules not confirmed".to_string(),
        );
    }
    if state.signal_source_count < params.min_signal_sources {
        return skip_signal(
            state,
            format!(
                "event_repricing_v2: only {} sources, need {}",
                state.signal_source_count, params.min_signal_sources
            ),
        );
    }
    if params.require_live_reference && !state.signal_has_live_reference {
        return skip_signal(
            state,
            "event_repricing_v2: no canonical live reference attached".to_string(),
        );
    }
    if state.signal_resolution_hazard_count > params.max_resolution_hazards {
        return skip_signal(
            state,
            format!(
                "event_repricing_v2: {} unresolved resolution hazards",
                state.signal_resolution_hazard_count
            ),
        );
    }
    let Some(time_to_resolution_seconds) = state.time_to_resolution_seconds else {
        return skip_signal(
            state,
            "event_repricing_v2: market close window unavailable".to_string(),
        );
    };
    if time_to_resolution_seconds < (params.min_hours_to_resolution as i64 * 3600) {
        return skip_signal(
            state,
            format!(
                "event_repricing_v2: market resolves too soon ({}h required)",
                params.min_hours_to_resolution
            ),
        );
    }

    let (fair_low, fair_high) = agent_probability_range(state, low, high);
    let fair_mid = (fair_low + fair_high) / 2.0;
    let edge_bps = ((fair_mid - state.mid_price).abs() * 10_000.0).round() as i32;
    let net_edge_bps = edge_bps - params.fee_buffer_bps - params.slippage_buffer_bps;
    if net_edge_bps < params.min_net_edge_bps {
        return skip_signal(
            state,
            format!(
                "event_repricing_v2: net edge {}bps below threshold {}bps",
                net_edge_bps, params.min_net_edge_bps
            ),
        );
    }

    let favorable = if state.agent_side == "buy" {
        fair_mid > state.mid_price
    } else {
        fair_mid < state.mid_price
    };
    if !favorable {
        return skip_signal(
            state,
            "event_repricing_v2: signal direction does not match configured side".to_string(),
        );
    }

    let edge_scale = (f64::from(net_edge_bps.max(params.min_net_edge_bps))
        / f64::from(params.min_net_edge_bps))
    .clamp(1.0, 3.0);
    let max_frac = params.size_frac_max.max(params.size_frac_min);
    let size_frac = (params.size_frac_min
        + ((edge_scale - 1.0) / 2.0) * (max_frac - params.size_frac_min))
        .clamp(params.size_frac_min, max_frac);

    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: state.agent_quantity * size_frac,
        reason: format!(
            "event_repricing_v2: fair {:.4}-{:.4}, mid {:.4}, net edge {}bps",
            fair_low, fair_high, state.mid_price, net_edge_bps
        ),
        metadata: json!({
            "signalId": params.signal_id,
            "fairValueLow": fair_low,
            "fairValueHigh": fair_high,
            "midpointDeltaBps": state.midpoint_delta_bps,
            "netEdgeBps": net_edge_bps,
            "sizeFraction": size_frac,
            "maxSlippageBps": params.max_slippage_bps,
            "ttlMinutes": params.ttl_minutes,
            "sourceCount": state.signal_source_count,
            "resolutionRulesRead": state.signal_resolution_rules_read,
            "hasLiveReference": state.signal_has_live_reference,
            "resolutionHazards": state.signal_resolution_hazard_count,
            "referencePrice": state.reference_price
        }),
    }
}

fn evaluate_wallet_follow(state: &MarketState, raw: &Value) -> TradeSignal {
    let params = match parse_params::<WalletFollowParams>(raw) {
        Ok(value) => value,
        Err(_) => {
            return skip_signal(state, "wallet_follow: invalid strategy params".to_string());
        }
    };

    if params.target_wallet.trim().is_empty() {
        return skip_signal(state, "wallet_follow: target wallet missing".to_string());
    }
    if params.observed_detection_to_order_ms > params.max_detection_to_order_ms {
        return skip_signal(
            state,
            format!(
                "wallet_follow: latency {}ms above gate {}ms",
                params.observed_detection_to_order_ms, params.max_detection_to_order_ms
            ),
        );
    }
    if params.observed_slippage_ticks > params.max_slippage_ticks {
        return skip_signal(
            state,
            format!(
                "wallet_follow: slippage {:.2} ticks above gate {:.2}",
                params.observed_slippage_ticks, params.max_slippage_ticks
            ),
        );
    }

    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: state.agent_quantity * params.copy_size_multiplier.clamp(0.1, 1.0),
        reason: format!(
            "wallet_follow: {} within latency/slippage gate",
            params.target_wallet
        ),
        metadata: json!({
            "targetWallet": params.target_wallet,
            "detectionToOrderMs": params.observed_detection_to_order_ms,
            "slippageTicks": params.observed_slippage_ticks
        }),
    }
}

fn evaluate_wallet_follow_v2(state: &MarketState, raw: &Value) -> TradeSignal {
    let params = match parse_params::<WalletFollowV2Params>(raw) {
        Ok(value) => value,
        Err(_) => {
            return skip_signal(
                state,
                "wallet_follow_v2: invalid strategy params".to_string(),
            );
        }
    };

    if params.target_wallet.trim().is_empty() {
        return skip_signal(state, "wallet_follow_v2: target wallet missing".to_string());
    }
    if params.wallet_score < params.wallet_score_min {
        return skip_signal(
            state,
            format!(
                "wallet_follow_v2: wallet score {:.2} below gate {:.2}",
                params.wallet_score, params.wallet_score_min
            ),
        );
    }
    if params.concurrent_markets >= params.max_concurrent_markets {
        return skip_signal(
            state,
            format!(
                "wallet_follow_v2: concurrent markets {} at gate {}",
                params.concurrent_markets, params.max_concurrent_markets
            ),
        );
    }
    if params.seconds_since_last_follow < params.cooldown_seconds {
        return skip_signal(
            state,
            format!(
                "wallet_follow_v2: cooldown {}s remaining",
                params.cooldown_seconds - params.seconds_since_last_follow
            ),
        );
    }
    if params.observed_detection_to_order_ms > params.max_detection_to_order_ms {
        return skip_signal(
            state,
            format!(
                "wallet_follow_v2: latency {}ms above gate {}ms",
                params.observed_detection_to_order_ms, params.max_detection_to_order_ms
            ),
        );
    }
    if params.observed_slippage_ticks > params.max_slippage_ticks {
        return skip_signal(
            state,
            format!(
                "wallet_follow_v2: slippage {:.2} ticks above gate {:.2}",
                params.observed_slippage_ticks, params.max_slippage_ticks
            ),
        );
    }
    if params.crowding_score > params.crowding_gate {
        return skip_signal(
            state,
            format!(
                "wallet_follow_v2: crowding {:.2} above gate {:.2}",
                params.crowding_score, params.crowding_gate
            ),
        );
    }

    let follow_ratio = params.follow_ratio.clamp(0.05, 1.0);
    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: state.agent_quantity * follow_ratio,
        reason: format!(
            "wallet_follow_v2: {} passed wallet/latency/slippage gates",
            params.target_wallet
        ),
        metadata: json!({
            "targetWallet": params.target_wallet,
            "walletScore": params.wallet_score,
            "walletScoreMin": params.wallet_score_min,
            "detectionToOrderMs": params.observed_detection_to_order_ms,
            "slippageTicks": params.observed_slippage_ticks,
            "followRatio": follow_ratio,
            "crowdingScore": params.crowding_score
        }),
    }
}

// ── Polymarket Alpha strategy evaluators ──

fn evaluate_longshot_harvest(state: &MarketState, raw: &Value) -> TradeSignal {
    let params = parse_params::<LongshotHarvestParams>(raw).unwrap_or_default();

    // Only trade longshots (low implied probability)
    let implied_prob = if state.agent_outcome == "yes" {
        state.yes_price
    } else {
        state.no_price
    };

    if implied_prob > params.max_implied_prob {
        return skip_signal(
            state,
            format!(
                "longshot_harvest: implied prob {:.1}% above threshold {:.1}%",
                implied_prob * 100.0,
                params.max_implied_prob * 100.0
            ),
        );
    }

    // Check category exposure limit
    if params.current_category_exposure_pct >= params.max_category_exposure_pct {
        return skip_signal(
            state,
            format!(
                "longshot_harvest: category exposure {:.1}% at limit {:.1}%",
                params.current_category_exposure_pct * 100.0,
                params.max_category_exposure_pct * 100.0
            ),
        );
    }

    // Calculate mispricing using historical calibration
    let actual_win_rate = if params.historical_win_rate > 0.0 {
        params.historical_win_rate
    } else {
        // Default calibration from Becker research: 5c contracts win ~4.18%
        implied_prob * 0.836 // ~16.4% overpriced on average for longshots
    };

    let mispricing = if implied_prob > 0.0 {
        ((implied_prob - actual_win_rate) / implied_prob) * 100.0
    } else {
        0.0
    };

    if mispricing < params.min_mispricing_pct {
        return skip_signal(
            state,
            format!(
                "longshot_harvest: mispricing {:.1}% below threshold {:.1}%",
                mispricing, params.min_mispricing_pct
            ),
        );
    }

    // Direction: SELL YES (be the house against optimistic longshot buyers)
    // The agent should be configured with side="sell", outcome="yes"
    // If the agent is configured to buy NO, that also works

    // Size using Kelly Criterion with proper position sizing
    let (sized_quantity, kelly_metadata) = match crate::services::kelly::kelly_sized_quantity(
        state.agent_quantity,
        params.bankroll_usdc,
        actual_win_rate,
        implied_prob,
        params.kelly_fraction,
        params.max_position_pct,
        params.is_correlated,
        params.drawdown_from_peak_pct,
    ) {
        Some((qty, result)) => (
            qty,
            json!({
                "side": format!("{:?}", result.side),
                "fullKellyFrac": result.full_kelly_frac,
                "adjustedFrac": result.adjusted_frac,
                "positionSizeUsdc": result.position_size_usdc,
                "contracts": result.contracts,
                "edgeBps": result.edge_bps,
            }),
        ),
        None => {
            // Fallback: simple edge-scaled sizing
            let edge_factor = (mispricing / params.min_mispricing_pct).clamp(1.0, 3.0);
            let base_frac = params.kelly_fraction * params.max_position_pct;
            let size_frac = (base_frac * edge_factor).min(params.max_position_pct);
            (
                state.agent_quantity * size_frac.clamp(0.1, 1.0),
                json!({ "fallback": true, "sizeFraction": size_frac }),
            )
        }
    };

    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: sized_quantity,
        reason: format!(
            "longshot_harvest: implied {:.1}%, actual {:.1}%, mispricing {:.1}%",
            implied_prob * 100.0,
            actual_win_rate * 100.0,
            mispricing
        ),
        metadata: json!({
            "impliedProb": implied_prob,
            "historicalWinRate": actual_win_rate,
            "mispricingPct": mispricing,
            "kelly": kelly_metadata,
            "categoryExposure": params.current_category_exposure_pct
        }),
    }
}

fn evaluate_spread_capture(state: &MarketState, raw: &Value) -> TradeSignal {
    let params = parse_params::<SpreadCaptureParams>(raw).unwrap_or_default();

    // We need both sides' best bids to calculate total cost
    // The counterpart_best_bid is the best bid on the OTHER side (NO if we're looking at YES)
    let our_price = state.agent_price;
    let counterpart_bid = params.counterpart_best_bid;

    if counterpart_bid <= 0.0 {
        return skip_signal(
            state,
            "spread_capture: counterpart bid not available".to_string(),
        );
    }

    let total_cost = our_price + counterpart_bid;
    let guaranteed_profit = 1.0 - total_cost;
    let profit_bps = (guaranteed_profit / total_cost * 10_000.0).round();

    if total_cost >= params.max_total_cost {
        return skip_signal(
            state,
            format!(
                "spread_capture: total cost {:.4} exceeds max {:.4}",
                total_cost, params.max_total_cost
            ),
        );
    }

    if profit_bps < params.min_profit_bps {
        return skip_signal(
            state,
            format!(
                "spread_capture: profit {:.0}bps below threshold {:.0}bps",
                profit_bps, params.min_profit_bps
            ),
        );
    }

    // Check duration constraint
    if let Some(ttl) = state.time_to_resolution_seconds {
        let duration_minutes = ttl as u64 / 60;
        if params.max_duration_minutes > 0 && duration_minutes > params.max_duration_minutes {
            return skip_signal(
                state,
                format!(
                    "spread_capture: duration {}min exceeds max {}min",
                    duration_minutes, params.max_duration_minutes
                ),
            );
        }
    }

    // Check inventory imbalance
    if params.inventory_limit > 0.0 {
        let imbalance = (params.current_yes_inventory - params.current_no_inventory).abs();
        if imbalance > params.inventory_limit {
            return skip_signal(
                state,
                format!(
                    "spread_capture: inventory imbalance {:.2} above limit {:.2}",
                    imbalance, params.inventory_limit
                ),
            );
        }
    }

    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: state.agent_quantity,
        reason: format!(
            "spread_capture: buy {}@{:.4} + counterpart@{:.4} = {:.4}, profit {:.0}bps",
            state.agent_outcome, our_price, counterpart_bid, total_cost, profit_bps
        ),
        metadata: json!({
            "totalCost": total_cost,
            "guaranteedProfit": guaranteed_profit,
            "profitBps": profit_bps,
            "counterpartBid": counterpart_bid,
            "yesInventory": params.current_yes_inventory,
            "noInventory": params.current_no_inventory
        }),
    }
}

fn evaluate_near_certainty(state: &MarketState, raw: &Value) -> TradeSignal {
    let params = parse_params::<NearCertaintyParams>(raw).unwrap_or_default();

    let price = if state.agent_outcome == "yes" {
        state.yes_price
    } else {
        state.no_price
    };

    if price < params.min_price {
        return skip_signal(
            state,
            format!(
                "near_certainty: price {:.2} below threshold {:.2}",
                price, params.min_price
            ),
        );
    }

    // Check signal count if required
    if params.signal_count < params.min_signal_count {
        return skip_signal(
            state,
            format!(
                "near_certainty: only {} signals, need {}",
                params.signal_count, params.min_signal_count
            ),
        );
    }

    // Use calibrated probability if available, otherwise estimate from research
    let calibrated = if params.calibrated_probability > 0.0 {
        params.calibrated_probability
    } else {
        // From research: 95c contracts actually win ~96.5% of the time
        price + (1.0 - price) * 0.3 // ~30% of remaining probability is edge
    };

    let edge_bps = ((calibrated - price) * 10_000.0).round() as i32;
    if edge_bps < params.min_edge_bps {
        return skip_signal(
            state,
            format!(
                "near_certainty: edge {}bps below threshold {}bps",
                edge_bps, params.min_edge_bps
            ),
        );
    }

    // Check time constraint
    if let Some(ttl) = state.time_to_resolution_seconds {
        let hours = ttl as u64 / 3600;
        if params.max_hours_to_resolution > 0 && hours > params.max_hours_to_resolution {
            return skip_signal(
                state,
                format!(
                    "near_certainty: {}h to resolution exceeds max {}h",
                    hours, params.max_hours_to_resolution
                ),
            );
        }
    }

    // Size using Kelly Criterion
    let (sized_quantity, kelly_metadata) = match crate::services::kelly::kelly_sized_quantity(
        state.agent_quantity,
        params.bankroll_usdc,
        calibrated,
        price,
        params.kelly_fraction,
        params.max_position_pct.min(0.03), // cap for near-certainty tail risk
        params.is_correlated,
        params.drawdown_from_peak_pct,
    ) {
        Some((qty, result)) => (
            qty,
            json!({
                "side": format!("{:?}", result.side),
                "fullKellyFrac": result.full_kelly_frac,
                "adjustedFrac": result.adjusted_frac,
                "positionSizeUsdc": result.position_size_usdc,
                "contracts": result.contracts,
                "edgeBps": result.edge_bps,
            }),
        ),
        None => {
            // Fallback: fixed small fraction
            let size_frac = params.max_position_pct.min(0.03);
            (
                state.agent_quantity * (size_frac / params.max_position_pct).clamp(0.1, 1.0),
                json!({ "fallback": true, "sizeFraction": size_frac }),
            )
        }
    };

    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: sized_quantity,
        reason: format!(
            "near_certainty: price {:.2}, calibrated {:.4}, edge {}bps",
            price, calibrated, edge_bps
        ),
        metadata: json!({
            "price": price,
            "calibratedProb": calibrated,
            "edgeBps": edge_bps,
            "signalCount": params.signal_count,
            "kelly": kelly_metadata
        }),
    }
}

fn evaluate_correlation_arb(state: &MarketState, raw: &Value) -> TradeSignal {
    let params = match parse_params::<CorrelationArbParams>(raw) {
        Ok(value) => value,
        Err(_) => {
            return skip_signal(
                state,
                "correlation_arb: invalid strategy params".to_string(),
            );
        }
    };

    if params.related_market_id.trim().is_empty() {
        return skip_signal(
            state,
            "correlation_arb: related market ID missing".to_string(),
        );
    }

    if params.related_market_price <= 0.0 || params.related_market_price >= 1.0 {
        return skip_signal(
            state,
            "correlation_arb: related market price unavailable".to_string(),
        );
    }

    // Calculate combined cost for the arb trade
    // For implication arbs: if A implies B, then P(A) <= P(B)
    // If P(A) > P(B), buy NO on A + YES on B (or vice versa)
    let combined = params.combined_cost;
    if combined <= 0.0 {
        return skip_signal(
            state,
            "correlation_arb: combined cost not computed".to_string(),
        );
    }

    if combined >= params.max_combined_cost {
        return skip_signal(
            state,
            format!(
                "correlation_arb: combined cost {:.4} too high (max {:.4})",
                combined, params.max_combined_cost
            ),
        );
    }

    let profit = 1.0 - combined;
    let profit_bps = (profit / combined * 10_000.0).round();

    if profit_bps < params.min_arb_profit_bps {
        return skip_signal(
            state,
            format!(
                "correlation_arb: profit {:.0}bps below threshold {:.0}bps",
                profit_bps, params.min_arb_profit_bps
            ),
        );
    }

    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: state.agent_quantity,
        reason: format!(
            "correlation_arb: {} vs {}, combined {:.4}, profit {:.0}bps",
            state.agent_outcome, params.logical_constraint, combined, profit_bps
        ),
        metadata: json!({
            "relatedMarketId": params.related_market_id,
            "relatedMarketPrice": params.related_market_price,
            "logicalConstraint": params.logical_constraint,
            "combinedCost": combined,
            "profitBps": profit_bps
        }),
    }
}

fn book_spread_bps(state: &MarketState) -> f64 {
    match (state.best_bid, state.best_ask) {
        (Some(bid), Some(ask)) if bid > 0.0 && ask > 0.0 && ask >= bid => {
            let mid = (bid + ask) / 2.0;
            if mid <= 0.0 {
                0.0
            } else {
                ((ask - bid) / mid) * 10_000.0
            }
        }
        _ => 0.0,
    }
}

fn agent_probability_range(
    state: &MarketState,
    fair_value_low: f64,
    fair_value_high: f64,
) -> (f64, f64) {
    if state.agent_outcome == "no" {
        (1.0 - fair_value_high, 1.0 - fair_value_low)
    } else {
        (fair_value_low, fair_value_high)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_state() -> MarketState {
        MarketState {
            yes_price: 0.6,
            no_price: 0.4,
            best_bid: Some(0.58),
            best_ask: Some(0.62),
            mid_price: 0.6,
            agent_price: 0.55,
            agent_side: "buy".to_string(),
            agent_outcome: "yes".to_string(),
            agent_quantity: 100.0,
            time_to_resolution_seconds: Some(48 * 3600),
            fair_value_low: None,
            fair_value_high: None,
            midpoint_delta_bps: None,
            signal_source_count: 3,
            signal_resolution_rules_read: true,
            signal_has_live_reference: true,
            signal_resolution_hazard_count: 0,
            reference_price: None,
        }
    }

    #[test]
    fn default_always_executes() {
        let signal = evaluate_strategy("unknown", &base_state(), &json!({}));
        assert!(signal.execute);
        assert!((signal.price - 0.55).abs() < f64::EPSILON);
    }

    #[test]
    fn maker_reward_requires_rebate_or_override() {
        let signal = evaluate_strategy(
            "maker_reward",
            &base_state(),
            &json!({ "feeEnabled": false, "rebateEligible": false }),
        );
        assert!(!signal.execute);

        let signal = evaluate_strategy(
            "maker_reward",
            &base_state(),
            &json!({ "feeEnabled": true, "rebateEligible": true, "makerRebateBps": 4 }),
        );
        assert!(signal.execute);
    }

    #[test]
    fn event_repricing_uses_signal_range() {
        let mut state = base_state();
        state.fair_value_low = Some(0.66);
        state.fair_value_high = Some(0.70);
        state.mid_price = 0.60;
        let signal = evaluate_strategy("event_repricing", &state, &json!({}));
        assert!(signal.execute);
        assert!(signal.quantity >= 50.0 && signal.quantity <= 100.0);
    }

    #[test]
    fn event_repricing_v2_scales_with_size_fraction() {
        let mut state = base_state();
        state.fair_value_low = Some(0.66);
        state.fair_value_high = Some(0.70);
        state.mid_price = 0.60;
        let signal = evaluate_strategy(
            "event_repricing_v2",
            &state,
            &json!({
                "sizeFracMin": 0.3,
                "sizeFracMax": 0.6,
                "minNetEdgeBps": 400
            }),
        );
        assert!(signal.execute);
        assert!(signal.quantity >= 30.0);
        assert!(signal.quantity <= 60.0);
    }

    #[test]
    fn event_repricing_v2_rejects_unsourced_signal() {
        let mut state = base_state();
        state.fair_value_low = Some(0.66);
        state.fair_value_high = Some(0.70);
        state.mid_price = 0.60;
        state.signal_source_count = 1;
        let signal = evaluate_strategy(
            "event_repricing_v2",
            &state,
            &json!({
                "signalId": "sig-1",
                "minSignalSources": 2
            }),
        );
        assert!(!signal.execute);
        assert!(signal.reason.contains("only 1 sources"));
    }

    #[test]
    fn event_repricing_v2_rejects_resolution_hazards() {
        let mut state = base_state();
        state.fair_value_low = Some(0.66);
        state.fair_value_high = Some(0.70);
        state.mid_price = 0.60;
        state.signal_resolution_hazard_count = 2;
        let signal = evaluate_strategy(
            "event_repricing_v2",
            &state,
            &json!({
                "signalId": "sig-1",
                "maxResolutionHazards": 0
            }),
        );
        assert!(!signal.execute);
        assert!(signal.reason.contains("resolution hazards"));
    }

    #[test]
    fn wallet_follow_v2_respects_wallet_score_gate() {
        let signal = evaluate_strategy(
            "wallet_follow_v2",
            &base_state(),
            &json!({
                "targetWallet": "0xabc",
                "walletScore": 0.2,
                "walletScoreMin": 0.6
            }),
        );
        assert!(!signal.execute);
    }

    #[test]
    fn wallet_follow_v2_executes_when_all_gates_pass() {
        let signal = evaluate_strategy(
            "wallet_follow_v2",
            &base_state(),
            &json!({
                "targetWallet": "0xabc",
                "walletScore": 0.8,
                "walletScoreMin": 0.6,
                "followRatio": 0.5,
                "observedDetectionToOrderMs": 900,
                "maxDetectionToOrderMs": 1250,
                "observedSlippageTicks": 0.5,
                "maxSlippageTicks": 1.0,
                "concurrentMarkets": 1,
                "maxConcurrentMarkets": 3,
                "secondsSinceLastFollow": 900,
                "cooldownSeconds": 300,
                "crowdingScore": 0.1,
                "crowdingGate": 0.5
            }),
        );
        assert!(signal.execute);
        assert!((signal.quantity - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn validate_wallet_follow_requires_target_wallet() {
        let err = validate_strategy_params("wallet_follow", &json!({})).unwrap_err();
        assert_eq!(err.code, "INVALID_STRATEGY_PARAMS");
    }

    #[test]
    fn validate_wallet_follow_v2_requires_target_wallet() {
        let err = validate_strategy_params("wallet_follow_v2", &json!({})).unwrap_err();
        assert_eq!(err.code, "INVALID_STRATEGY_PARAMS");
    }

    // ── Polymarket Alpha strategy tests ──

    #[test]
    fn longshot_harvest_sells_overpriced_longshots() {
        let mut state = base_state();
        state.yes_price = 0.05;
        state.no_price = 0.95;
        state.agent_outcome = "yes".to_string();
        state.agent_side = "sell".to_string();
        let signal = evaluate_strategy(
            "longshot_harvest",
            &state,
            &json!({
                "maxImpliedProb": 0.10,
                "minMispricingPct": 10.0,
                "historicalWinRate": 0.0418
            }),
        );
        assert!(signal.execute);
        assert!(signal.reason.contains("longshot_harvest"));
    }

    #[test]
    fn longshot_harvest_skips_expensive_contracts() {
        let state = base_state(); // yes_price = 0.6 = too high for longshot
        let signal = evaluate_strategy(
            "longshot_harvest",
            &state,
            &json!({ "maxImpliedProb": 0.10 }),
        );
        assert!(!signal.execute);
    }

    #[test]
    fn longshot_harvest_skips_low_mispricing() {
        let mut state = base_state();
        state.yes_price = 0.05;
        state.agent_outcome = "yes".to_string();
        let signal = evaluate_strategy(
            "longshot_harvest",
            &state,
            &json!({
                "maxImpliedProb": 0.10,
                "minMispricingPct": 50.0,
                "historicalWinRate": 0.048
            }),
        );
        assert!(!signal.execute);
    }

    #[test]
    fn spread_capture_executes_on_profitable_spread() {
        let mut state = base_state();
        state.agent_price = 0.47;
        state.agent_outcome = "yes".to_string();
        state.time_to_resolution_seconds = Some(10 * 60); // 10 minutes
        let signal = evaluate_strategy(
            "spread_capture",
            &state,
            &json!({
                "maxTotalCost": 0.98,
                "minProfitBps": 200.0,
                "counterpartBestBid": 0.48,
                "maxDurationMinutes": 60
            }),
        );
        assert!(signal.execute);
        assert!(signal.reason.contains("spread_capture"));
    }

    #[test]
    fn spread_capture_skips_when_too_expensive() {
        let mut state = base_state();
        state.agent_price = 0.52;
        state.time_to_resolution_seconds = Some(10 * 60);
        let signal = evaluate_strategy(
            "spread_capture",
            &state,
            &json!({
                "maxTotalCost": 0.98,
                "minProfitBps": 200.0,
                "counterpartBestBid": 0.50,
                "maxDurationMinutes": 60
            }),
        );
        assert!(!signal.execute); // 0.52 + 0.50 = 1.02 > 0.98
    }

    #[test]
    fn near_certainty_buys_high_probability() {
        let mut state = base_state();
        state.yes_price = 0.93;
        state.no_price = 0.07;
        state.agent_outcome = "yes".to_string();
        state.agent_side = "buy".to_string();
        let signal = evaluate_strategy(
            "near_certainty",
            &state,
            &json!({
                "minPrice": 0.90,
                "minEdgeBps": 100,
                "calibratedProbability": 0.955,
                "signalCount": 2,
                "minSignalCount": 1
            }),
        );
        assert!(signal.execute);
    }

    #[test]
    fn near_certainty_skips_low_probability() {
        let state = base_state(); // yes_price = 0.6 = not near certainty
        let signal = evaluate_strategy("near_certainty", &state, &json!({ "minPrice": 0.90 }));
        assert!(!signal.execute);
    }

    #[test]
    fn correlation_arb_executes_on_mispricing() {
        let state = base_state();
        let signal = evaluate_strategy(
            "correlation_arb",
            &state,
            &json!({
                "relatedMarketId": "polymarket:0xabc",
                "relatedMarketPrice": 0.40,
                "logicalConstraint": "A_implies_B",
                "combinedCost": 0.92,
                "maxCombinedCost": 0.99,
                "minArbProfitBps": 25.0
            }),
        );
        assert!(signal.execute);
    }

    #[test]
    fn correlation_arb_requires_related_market() {
        let err = validate_strategy_params("correlation_arb", &json!({})).unwrap_err();
        assert_eq!(err.code, "INVALID_STRATEGY_PARAMS");
    }
}
