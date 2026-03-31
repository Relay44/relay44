//! Pluggable trading strategy logic.
//!
//! Strategies compute a trade signal from market state and agent config.
//! The `strategy` field on `ExternalAgentRecord` selects which logic runs.

use serde::{Deserialize, Serialize};

/// Market state snapshot passed to strategy evaluation.
#[derive(Debug, Clone)]
pub struct MarketState {
    /// Current yes price (0–1 probability).
    pub yes_price: f64,
    /// Current no price (0–1 probability).
    pub no_price: f64,
    /// Best bid price for the agent's outcome.
    pub best_bid: Option<f64>,
    /// Best ask price for the agent's outcome.
    pub best_ask: Option<f64>,
    /// Mid price for the agent's outcome.
    pub mid_price: f64,
    /// Agent's configured price target.
    pub agent_price: f64,
    /// Agent's configured side ("buy" or "sell").
    pub agent_side: String,
    /// Agent's configured outcome ("yes" or "no").
    pub agent_outcome: String,
    /// Agent's configured quantity.
    pub agent_quantity: f64,
}

/// Signal returned by a strategy evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSignal {
    /// Whether to execute this tick.
    pub execute: bool,
    /// Adjusted price (may differ from agent_price).
    pub price: f64,
    /// Adjusted quantity (may differ from agent_quantity).
    pub quantity: f64,
    /// Reason for the decision.
    pub reason: String,
}

/// Evaluate the strategy and return a trade signal.
///
/// Falls back to the default (always-execute) behavior for unknown strategies.
pub fn evaluate_strategy(strategy: &str, state: &MarketState) -> TradeSignal {
    match strategy {
        "momentum" => evaluate_momentum(state),
        "mean-revert" | "mean_revert" => evaluate_mean_revert(state),
        "market-maker" | "market_maker" => evaluate_market_maker(state),
        _ => evaluate_default(state),
    }
}

/// Default strategy: always execute at the agent's configured price/quantity.
fn evaluate_default(state: &MarketState) -> TradeSignal {
    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: state.agent_quantity,
        reason: "default: execute at configured price".to_string(),
    }
}

/// Momentum strategy: only buy when price is trending in favorable direction.
///
/// For buys: execute when mid price < agent_price (price hasn't run away).
/// For sells: execute when mid price > agent_price.
/// Quantity scales up when signal is strong (bigger gap = more conviction).
fn evaluate_momentum(state: &MarketState) -> TradeSignal {
    let is_buy = state.agent_side == "buy";
    let spread = if is_buy {
        state.agent_price - state.mid_price
    } else {
        state.mid_price - state.agent_price
    };

    if spread <= 0.0 {
        return TradeSignal {
            execute: false,
            price: state.agent_price,
            quantity: state.agent_quantity,
            reason: format!(
                "momentum: unfavorable spread ({:.4}), waiting",
                spread
            ),
        };
    }

    // Scale quantity: 50% at edge, 100% at 5%+ spread, 150% at 10%+ spread.
    let strength = (spread / 0.05).clamp(0.5, 1.5);
    let adjusted_qty = state.agent_quantity * strength;

    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: adjusted_qty,
        reason: format!(
            "momentum: favorable spread {:.4}, strength {:.2}x",
            spread, strength
        ),
    }
}

/// Mean-revert strategy: execute when price deviates from a neutral zone.
///
/// Buys when price drops below agent_price (expecting reversion up).
/// Sells when price rises above agent_price (expecting reversion down).
/// Skips when price is within a tight band around agent_price.
fn evaluate_mean_revert(state: &MarketState) -> TradeSignal {
    let deviation = state.mid_price - state.agent_price;
    let abs_deviation = deviation.abs();

    // Dead zone: skip when within 2% of target.
    if abs_deviation < 0.02 {
        return TradeSignal {
            execute: false,
            price: state.agent_price,
            quantity: state.agent_quantity,
            reason: format!(
                "mean-revert: price within dead zone (deviation={:.4})",
                deviation
            ),
        };
    }

    let is_buy = state.agent_side == "buy";
    let favorable = (is_buy && deviation < 0.0) || (!is_buy && deviation > 0.0);

    if !favorable {
        return TradeSignal {
            execute: false,
            price: state.agent_price,
            quantity: state.agent_quantity,
            reason: format!(
                "mean-revert: deviation {:.4} not favorable for {}",
                deviation, state.agent_side
            ),
        };
    }

    // More aggressive sizing when deviation is larger.
    let strength = (abs_deviation / 0.05).clamp(0.5, 2.0);
    let adjusted_qty = state.agent_quantity * strength;

    TradeSignal {
        execute: true,
        price: state.agent_price,
        quantity: adjusted_qty,
        reason: format!(
            "mean-revert: deviation {:.4}, strength {:.2}x",
            deviation, strength
        ),
    }
}

/// Market-maker strategy: place orders on both sides of the spread.
///
/// Always executes, but adjusts price to sit at best_bid/best_ask edges.
/// Flips the side based on inventory considerations (simplified: alternates).
fn evaluate_market_maker(state: &MarketState) -> TradeSignal {
    let (price, reason) = if state.agent_side == "buy" {
        let bid = state.best_bid.unwrap_or(state.mid_price - 0.01);
        // Improve by 0.001 to sit at top of book.
        let price = (bid + 0.001).min(state.agent_price);
        (price, format!("market-maker: bid at {:.4} (book={:.4})", price, bid))
    } else {
        let ask = state.best_ask.unwrap_or(state.mid_price + 0.01);
        let price = (ask - 0.001).max(state.agent_price);
        (price, format!("market-maker: ask at {:.4} (book={:.4})", price, ask))
    };

    TradeSignal {
        execute: true,
        price,
        quantity: state.agent_quantity,
        reason,
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
        }
    }

    #[test]
    fn default_always_executes() {
        let signal = evaluate_strategy("unknown", &base_state());
        assert!(signal.execute);
        assert!((signal.price - 0.55).abs() < f64::EPSILON);
    }

    #[test]
    fn momentum_executes_when_favorable() {
        // mid=0.6, agent_price=0.55 → spread is negative for buy → skip
        let signal = evaluate_strategy("momentum", &base_state());
        assert!(!signal.execute);

        // Now set mid below agent_price → favorable
        let mut state = base_state();
        state.mid_price = 0.50;
        let signal = evaluate_strategy("momentum", &state);
        assert!(signal.execute);
        assert!(signal.quantity >= 100.0); // scaled up
    }

    #[test]
    fn mean_revert_skips_in_dead_zone() {
        let mut state = base_state();
        state.mid_price = 0.56; // within 2% of 0.55
        let signal = evaluate_strategy("mean-revert", &state);
        assert!(!signal.execute);
    }

    #[test]
    fn mean_revert_executes_on_deviation() {
        let mut state = base_state();
        state.mid_price = 0.50; // 5% below target, buy is favorable
        let signal = evaluate_strategy("mean-revert", &state);
        assert!(signal.execute);
    }

    #[test]
    fn market_maker_always_executes() {
        let signal = evaluate_strategy("market-maker", &base_state());
        assert!(signal.execute);
        // Price should be near best_bid + 0.001 but capped at agent_price
        assert!(signal.price <= 0.55);
    }
}
