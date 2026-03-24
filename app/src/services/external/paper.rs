use serde::Serialize;

use super::types::{
    clamp_probability, ExternalMarketSnapshot, ExternalOrderBookLevel, ExternalOrderBookSnapshot,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperFillComputation {
    pub requested_quantity: f64,
    pub filled_quantity: f64,
    pub average_price: f64,
    pub mark_price: f64,
    pub notional_usdc: f64,
    pub fee_usdc: f64,
    pub slippage_bps: i64,
    pub partial_fill: bool,
    pub used_orderbook_depth: bool,
}

fn market_quote_price(market: &ExternalMarketSnapshot, outcome: &str) -> f64 {
    match outcome.trim().to_ascii_lowercase().as_str() {
        "no" => clamp_probability(market.no_price),
        _ => clamp_probability(market.yes_price),
    }
}

fn sorted_levels(side: &str, orderbook: &ExternalOrderBookSnapshot) -> Vec<ExternalOrderBookLevel> {
    let mut levels = match side.trim().to_ascii_lowercase().as_str() {
        "sell" => orderbook.bids.clone(),
        _ => orderbook.asks.clone(),
    };

    if side.trim().eq_ignore_ascii_case("sell") {
        levels.sort_by(|a, b| b.price.total_cmp(&a.price));
    } else {
        levels.sort_by(|a, b| a.price.total_cmp(&b.price));
    }

    levels
}

pub fn resolve_mark_price(
    market: &ExternalMarketSnapshot,
    orderbook: &ExternalOrderBookSnapshot,
    outcome: &str,
) -> f64 {
    let quote = market_quote_price(market, outcome);
    let best_bid = orderbook
        .bids
        .iter()
        .map(|level| clamp_probability(level.price))
        .max_by(|a, b| a.total_cmp(b));
    let best_ask = orderbook
        .asks
        .iter()
        .map(|level| clamp_probability(level.price))
        .min_by(|a, b| a.total_cmp(b));

    match (best_bid, best_ask) {
        (Some(bid), Some(ask)) if bid > 0.0 && ask > 0.0 => clamp_probability((bid + ask) / 2.0),
        (Some(bid), _) if bid > 0.0 => clamp_probability(bid),
        (_, Some(ask)) if ask > 0.0 => clamp_probability(ask),
        _ => quote,
    }
}

pub fn simulate_fill(
    market: &ExternalMarketSnapshot,
    orderbook: &ExternalOrderBookSnapshot,
    outcome: &str,
    side: &str,
    requested_quantity: f64,
    fee_bps: u64,
) -> PaperFillComputation {
    let sanitized_quantity = requested_quantity.max(0.0);
    let quote = market_quote_price(market, outcome);
    let mark_price = resolve_mark_price(market, orderbook, outcome);
    let levels = sorted_levels(side, orderbook);

    if sanitized_quantity <= 0.0 {
        return PaperFillComputation {
            requested_quantity: 0.0,
            filled_quantity: 0.0,
            average_price: quote,
            mark_price,
            notional_usdc: 0.0,
            fee_usdc: 0.0,
            slippage_bps: 0,
            partial_fill: false,
            used_orderbook_depth: false,
        };
    }

    let mut remaining = sanitized_quantity;
    let mut notional = 0.0;
    let mut used_depth = false;

    for level in levels {
        if remaining <= 0.0 {
            break;
        }

        let quantity = level.quantity.max(0.0);
        if quantity <= 0.0 {
            continue;
        }

        let filled = remaining.min(quantity);
        notional += filled * clamp_probability(level.price);
        remaining -= filled;
        used_depth = true;
    }

    let filled_quantity = sanitized_quantity - remaining;

    let (average_price, filled_quantity, partial_fill, used_orderbook_depth, notional) =
        if filled_quantity > 0.0 {
            (
                clamp_probability(notional / filled_quantity),
                filled_quantity,
                remaining > 0.0,
                used_depth,
                notional,
            )
        } else {
            let average_price = quote;
            let notional = sanitized_quantity * average_price;
            (average_price, sanitized_quantity, false, false, notional)
        };

    let fee_usdc = notional * (fee_bps as f64 / 10_000.0);
    let slippage_bps = ((average_price - quote) * 10_000.0).round() as i64;

    PaperFillComputation {
        requested_quantity: sanitized_quantity,
        filled_quantity,
        average_price,
        mark_price,
        notional_usdc: notional,
        fee_usdc,
        slippage_bps,
        partial_fill,
        used_orderbook_depth,
    }
}

pub fn unrealized_pnl(side: &str, entry_price: f64, mark_price: f64, quantity: f64) -> f64 {
    let gross = if side.trim().eq_ignore_ascii_case("sell") {
        (entry_price - mark_price) * quantity
    } else {
        (mark_price - entry_price) * quantity
    };
    if gross.is_finite() {
        gross
    } else {
        0.0
    }
}

pub fn realized_pnl(
    side: &str,
    entry_price: f64,
    exit_price: f64,
    quantity: f64,
    total_fees_usdc: f64,
) -> f64 {
    unrealized_pnl(side, entry_price, exit_price, quantity) - total_fees_usdc.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::external::types::{now_rfc3339, ExternalOutcome};

    fn sample_market() -> ExternalMarketSnapshot {
        ExternalMarketSnapshot {
            id: "limitless:test".to_string(),
            question: "q".to_string(),
            description: "d".to_string(),
            category: "c".to_string(),
            status: "active".to_string(),
            close_time: 0,
            resolved: false,
            outcome: None,
            yes_price: 0.62,
            no_price: 0.38,
            volume: 1000.0,
            source: "external_limitless".to_string(),
            provider: "limitless".to_string(),
            is_external: true,
            external_url: "https://example.com".to_string(),
            chain_id: 8453,
            requires_credentials: false,
            execution_users: true,
            execution_agents: true,
            outcomes: vec![
                ExternalOutcome {
                    label: "Yes".to_string(),
                    probability: 0.62,
                },
                ExternalOutcome {
                    label: "No".to_string(),
                    probability: 0.38,
                },
            ],
            provider_market_ref: "ref".to_string(),
        }
    }

    fn sample_orderbook() -> ExternalOrderBookSnapshot {
        ExternalOrderBookSnapshot {
            market_id: "limitless:test".to_string(),
            outcome: "yes".to_string(),
            bids: vec![
                ExternalOrderBookLevel {
                    price: 0.60,
                    quantity: 4.0,
                    orders: 1,
                },
                ExternalOrderBookLevel {
                    price: 0.58,
                    quantity: 10.0,
                    orders: 1,
                },
            ],
            asks: vec![
                ExternalOrderBookLevel {
                    price: 0.63,
                    quantity: 3.0,
                    orders: 1,
                },
                ExternalOrderBookLevel {
                    price: 0.66,
                    quantity: 5.0,
                    orders: 1,
                },
            ],
            last_updated: now_rfc3339(),
            source: "external_limitless".to_string(),
            provider: "limitless".to_string(),
            chain_id: 8453,
            provider_market_ref: "ref".to_string(),
            is_synthetic: false,
        }
    }

    #[test]
    fn simulate_fill_uses_asks_for_buy_orders() {
        let fill = simulate_fill(&sample_market(), &sample_orderbook(), "yes", "buy", 4.0, 30);

        assert_eq!(fill.filled_quantity, 4.0);
        assert!((fill.average_price - 0.6375).abs() < 0.0001);
        assert!(fill.used_orderbook_depth);
    }

    #[test]
    fn simulate_fill_uses_bids_for_sell_orders() {
        let fill = simulate_fill(
            &sample_market(),
            &sample_orderbook(),
            "yes",
            "sell",
            5.0,
            30,
        );

        assert_eq!(fill.filled_quantity, 5.0);
        assert!((fill.average_price - 0.596).abs() < 0.0001);
        assert!(fill.used_orderbook_depth);
    }

    #[test]
    fn simulate_fill_falls_back_to_quote_without_depth() {
        let mut orderbook = sample_orderbook();
        orderbook.asks.clear();
        orderbook.bids.clear();

        let fill = simulate_fill(&sample_market(), &orderbook, "yes", "buy", 2.0, 30);

        assert_eq!(fill.filled_quantity, 2.0);
        assert!((fill.average_price - 0.62).abs() < 0.0001);
        assert!(!fill.used_orderbook_depth);
    }

    #[test]
    fn simulate_fill_returns_partial_when_depth_runs_out() {
        let fill = simulate_fill(
            &sample_market(),
            &sample_orderbook(),
            "yes",
            "buy",
            20.0,
            30,
        );

        assert_eq!(fill.filled_quantity, 8.0);
        assert!(fill.partial_fill);
    }

    #[test]
    fn realized_pnl_respects_side_direction() {
        assert!((realized_pnl("buy", 0.50, 0.60, 10.0, 0.2) - 0.8).abs() < 0.0001);
        assert!((realized_pnl("sell", 0.60, 0.50, 10.0, 0.2) - 0.8).abs() < 0.0001);
    }
}
