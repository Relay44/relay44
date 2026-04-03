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

fn default_wallet_latency_ms() -> i64 {
    1_500
}

fn default_wallet_slippage_ticks() -> f64 {
    1.0
}

fn default_copy_size_multiplier() -> f64 {
    0.8
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
        "momentum" | "mean-revert" | "default" | "" => Ok(json!({})),
        _ => Ok(raw.clone()),
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(params)
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
        "wallet-follow" => evaluate_wallet_follow(state, strategy_params),
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

    let strength = (abs_deviation / 0.05).clamp(0.5, 2.0);
    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: state.agent_quantity * strength,
        reason: format!("mean-revert: deviation {deviation:.4}, strength {strength:.2}x"),
        metadata: json!({ "strength": strength }),
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
        metadata: json!({ "quoteImprovementTicks": params.quote_improvement_ticks }),
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
        }
    }

    #[test]
    fn default_always_executes() {
        let signal = evaluate_strategy("unknown", &base_state(), &json!({}));
        assert!(signal.execute);
        assert!((signal.price - 0.55).abs() < f64::EPSILON);
    }

    #[test]
    fn momentum_executes_when_favorable() {
        let signal = evaluate_strategy("momentum", &base_state(), &json!({}));
        assert!(!signal.execute);

        let mut state = base_state();
        state.mid_price = 0.50;
        let signal = evaluate_strategy("momentum", &state, &json!({}));
        assert!(signal.execute);
        assert!(signal.quantity >= 100.0);
    }

    #[test]
    fn mean_revert_skips_in_dead_zone() {
        let mut state = base_state();
        state.mid_price = 0.56;
        let signal = evaluate_strategy("mean-revert", &state, &json!({}));
        assert!(!signal.execute);
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
        assert!((signal.quantity - 93.75).abs() < 1e-9);
    }

    #[test]
    fn event_repricing_scales_down_at_threshold_edge() {
        let mut state = base_state();
        state.fair_value_low = Some(0.64);
        state.fair_value_high = Some(0.65);
        state.mid_price = 0.60;
        let signal = evaluate_strategy("event_repricing", &state, &json!({}));
        assert!(signal.execute);
        assert!((signal.quantity - 50.0).abs() < 1e-9);
    }

    #[test]
    fn wallet_follow_respects_latency_gate() {
        let signal = evaluate_strategy(
            "wallet_follow",
            &base_state(),
            &json!({
                "targetWallet": "0xabc",
                "observedDetectionToOrderMs": 1800,
                "observedSlippageTicks": 0.5
            }),
        );
        assert!(!signal.execute);

        let signal = evaluate_strategy(
            "wallet_follow",
            &base_state(),
            &json!({
                "targetWallet": "0xabc",
                "observedDetectionToOrderMs": 900,
                "observedSlippageTicks": 0.5
            }),
        );
        assert!(signal.execute);
        assert!((signal.quantity - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn validate_wallet_follow_requires_target_wallet() {
        let err = validate_strategy_params("wallet_follow", &json!({})).unwrap_err();
        assert_eq!(err.code, "INVALID_STRATEGY_PARAMS");
    }
}
