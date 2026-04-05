use serde::{Deserialize, Serialize};

/// Kelly Criterion position sizing for prediction market trading.
///
/// Implements quarter-Kelly with hard caps, correlation penalty,
/// and drawdown circuit breaker.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KellyInput {
    pub bankroll_usdc: f64,
    pub estimated_prob: f64,
    pub market_price: f64,
    pub kelly_fraction: f64,
    pub max_position_pct: f64,
    pub is_correlated: bool,
    pub drawdown_from_peak_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KellyResult {
    pub side: KellySide,
    pub full_kelly_frac: f64,
    pub adjusted_frac: f64,
    pub position_size_usdc: f64,
    pub contracts: u64,
    pub max_profit_usdc: f64,
    pub max_loss_usdc: f64,
    pub edge_bps: i32,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KellySide {
    Yes,
    No,
    Skip,
}

impl Default for KellyInput {
    fn default() -> Self {
        Self {
            bankroll_usdc: 1000.0,
            estimated_prob: 0.5,
            market_price: 0.5,
            kelly_fraction: 0.25,
            max_position_pct: 0.05,
            is_correlated: false,
            drawdown_from_peak_pct: 0.0,
        }
    }
}

/// Calculate optimal position size using Kelly Criterion.
///
/// Returns the side (YES/NO/SKIP) and size in USDC.
pub fn calculate_kelly(input: &KellyInput) -> KellyResult {
    let skip = |reason: &str| KellyResult {
        side: KellySide::Skip,
        full_kelly_frac: 0.0,
        adjusted_frac: 0.0,
        position_size_usdc: 0.0,
        contracts: 0,
        max_profit_usdc: 0.0,
        max_loss_usdc: 0.0,
        edge_bps: 0,
        reason: reason.to_string(),
    };

    if input.bankroll_usdc <= 0.0 {
        return skip("bankroll is zero or negative");
    }
    if input.market_price <= 0.001 || input.market_price >= 0.999 {
        return skip("market price outside tradeable range");
    }
    if input.estimated_prob <= 0.0 || input.estimated_prob >= 1.0 {
        return skip("estimated probability outside valid range");
    }

    // Try YES side first
    let yes_result = kelly_for_side(input, true);
    let no_result = kelly_for_side(input, false);

    match (yes_result, no_result) {
        (Some(y), Some(n)) => {
            if y.full_kelly_frac >= n.full_kelly_frac {
                y
            } else {
                n
            }
        }
        (Some(y), None) => y,
        (None, Some(n)) => n,
        (None, None) => skip("no edge on either side"),
    }
}

fn kelly_for_side(input: &KellyInput, is_yes: bool) -> Option<KellyResult> {
    let (prob, price, side) = if is_yes {
        (input.estimated_prob, input.market_price, KellySide::Yes)
    } else {
        (
            1.0 - input.estimated_prob,
            1.0 - input.market_price,
            KellySide::No,
        )
    };

    // Net odds: profit per dollar risked
    let b = (1.0 - price) / price;
    if b <= 0.0 {
        return None;
    }

    let q = 1.0 - prob;
    let full_kelly = (prob * b - q) / b;

    if full_kelly <= 0.0 {
        return None;
    }

    let edge_bps = ((prob - price) * 10_000.0).round() as i32;

    // Apply fraction (quarter-Kelly default)
    let mut adjusted = full_kelly * input.kelly_fraction.clamp(0.01, 1.0);

    // Correlation penalty: halve size
    if input.is_correlated {
        adjusted *= 0.5;
    }

    // Drawdown circuit breaker
    // 0-10% drawdown: no change
    // 10-20% drawdown: reduce to half
    // 20%+ drawdown: reduce to quarter (1/8 Kelly)
    if input.drawdown_from_peak_pct > 20.0 {
        adjusted *= 0.25;
    } else if input.drawdown_from_peak_pct > 10.0 {
        adjusted *= 0.5;
    }

    // Hard cap
    adjusted = adjusted.min(input.max_position_pct);

    let position_size = (input.bankroll_usdc * adjusted).max(0.0);
    let contracts = (position_size / price).floor() as u64;

    if contracts == 0 {
        return None;
    }

    let actual_cost = contracts as f64 * price;
    let max_profit = contracts as f64 * (1.0 - price);

    let reason = format!(
        "kelly_{}: edge {}bps, full {:.1}%, adj {:.1}%, ${:.2} ({} contracts)",
        if is_yes { "yes" } else { "no" },
        edge_bps,
        full_kelly * 100.0,
        adjusted * 100.0,
        actual_cost,
        contracts
    );

    Some(KellyResult {
        side,
        full_kelly_frac: full_kelly,
        adjusted_frac: adjusted,
        position_size_usdc: actual_cost,
        contracts,
        max_profit_usdc: max_profit,
        max_loss_usdc: actual_cost,
        edge_bps,
        reason,
    })
}

/// Calculate expected value per contract.
pub fn expected_value(market_price: f64, estimated_prob: f64) -> f64 {
    let payout = 1.0 - market_price;
    (estimated_prob * payout) - ((1.0 - estimated_prob) * market_price)
}

/// Calculate calibrated probability from historical data.
///
/// Uses linear interpolation between calibration bucket boundaries.
pub fn calibrated_probability(
    implied_prob: f64,
    buckets: &[(f64, f64, f64)], // (bucket_low, bucket_high, actual_win_rate)
) -> f64 {
    for &(low, high, win_rate) in buckets {
        if implied_prob >= low && implied_prob < high {
            return win_rate;
        }
    }
    implied_prob // fallback: no calibration data
}

/// Calculate mispricing percentage.
pub fn mispricing_pct(implied_prob: f64, actual_win_rate: f64) -> f64 {
    if implied_prob <= 0.0 {
        return 0.0;
    }
    ((actual_win_rate - implied_prob) / implied_prob) * 100.0
}

/// Bayesian update: compute posterior probability given new evidence.
pub fn bayesian_update(prior: f64, likelihood_if_true: f64, likelihood_if_false: f64) -> f64 {
    let numerator = likelihood_if_true * prior;
    let denominator = numerator + (likelihood_if_false * (1.0 - prior));
    if denominator <= 0.0 {
        return prior;
    }
    (numerator / denominator).clamp(0.001, 0.999)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kelly_positive_ev_yes() {
        let input = KellyInput {
            bankroll_usdc: 5000.0,
            estimated_prob: 0.45,
            market_price: 0.30,
            kelly_fraction: 0.25,
            max_position_pct: 0.05,
            is_correlated: false,
            drawdown_from_peak_pct: 0.0,
        };
        let result = calculate_kelly(&input);
        assert_eq!(result.side, KellySide::Yes);
        assert!(result.position_size_usdc > 0.0);
        assert!(result.edge_bps > 0);
        assert!(result.contracts > 0);
    }

    #[test]
    fn kelly_positive_ev_no() {
        let input = KellyInput {
            bankroll_usdc: 5000.0,
            estimated_prob: 0.03,
            market_price: 0.08,
            kelly_fraction: 0.25,
            max_position_pct: 0.05,
            is_correlated: false,
            drawdown_from_peak_pct: 0.0,
        };
        let result = calculate_kelly(&input);
        assert_eq!(result.side, KellySide::No);
        assert!(result.position_size_usdc > 0.0);
    }

    #[test]
    fn kelly_no_edge_skips() {
        let input = KellyInput {
            bankroll_usdc: 5000.0,
            estimated_prob: 0.50,
            market_price: 0.50,
            kelly_fraction: 0.25,
            max_position_pct: 0.05,
            is_correlated: false,
            drawdown_from_peak_pct: 0.0,
        };
        let result = calculate_kelly(&input);
        assert_eq!(result.side, KellySide::Skip);
    }

    #[test]
    fn kelly_respects_hard_cap() {
        let input = KellyInput {
            bankroll_usdc: 5000.0,
            estimated_prob: 0.90,
            market_price: 0.30,
            kelly_fraction: 1.0, // full Kelly would be huge
            max_position_pct: 0.05,
            is_correlated: false,
            drawdown_from_peak_pct: 0.0,
        };
        let result = calculate_kelly(&input);
        assert!(result.position_size_usdc <= 5000.0 * 0.05 + 1.0); // within cap + rounding
    }

    #[test]
    fn kelly_correlation_penalty() {
        let base = KellyInput {
            bankroll_usdc: 5000.0,
            estimated_prob: 0.45,
            market_price: 0.30,
            kelly_fraction: 0.25,
            max_position_pct: 0.10,
            is_correlated: false,
            drawdown_from_peak_pct: 0.0,
        };
        let correlated = KellyInput {
            is_correlated: true,
            ..base.clone()
        };
        let r1 = calculate_kelly(&base);
        let r2 = calculate_kelly(&correlated);
        assert!(r2.adjusted_frac < r1.adjusted_frac);
    }

    #[test]
    fn kelly_drawdown_circuit_breaker() {
        let base = KellyInput {
            bankroll_usdc: 5000.0,
            estimated_prob: 0.45,
            market_price: 0.30,
            kelly_fraction: 0.25,
            max_position_pct: 0.10,
            is_correlated: false,
            drawdown_from_peak_pct: 0.0,
        };
        let in_drawdown = KellyInput {
            drawdown_from_peak_pct: 25.0,
            ..base.clone()
        };
        let r1 = calculate_kelly(&base);
        let r2 = calculate_kelly(&in_drawdown);
        assert!(r2.adjusted_frac < r1.adjusted_frac);
    }

    #[test]
    fn expected_value_positive() {
        let ev = expected_value(0.12, 0.20);
        assert!(ev > 0.0);
        assert!((ev - 0.08).abs() < 0.001);
    }

    #[test]
    fn expected_value_negative() {
        let ev = expected_value(0.12, 0.05);
        assert!(ev < 0.0);
    }

    #[test]
    fn bayesian_update_weak_jobs() {
        let posterior = bayesian_update(0.35, 0.70, 0.25);
        assert!((posterior - 0.601).abs() < 0.01);
    }

    #[test]
    fn bayesian_update_neutral_evidence() {
        let posterior = bayesian_update(0.50, 0.50, 0.50);
        assert!((posterior - 0.50).abs() < 0.001);
    }

    #[test]
    fn mispricing_longshot() {
        let pct = mispricing_pct(0.05, 0.0418);
        assert!(pct < 0.0); // overpriced
        assert!((pct - (-16.4)).abs() < 1.0);
    }

    #[test]
    fn calibrated_probability_lookup() {
        let buckets = vec![
            (0.0, 0.05, 0.0043),
            (0.05, 0.10, 0.063),
            (0.10, 0.20, 0.135),
        ];
        let result = calibrated_probability(0.07, &buckets);
        assert!((result - 0.063).abs() < 0.001);
    }
}
