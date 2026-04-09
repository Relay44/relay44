//! LMSR-based cost function engine for continuous distribution prediction markets.
//!
//! Traders express beliefs as Gaussian distributions (mu, sigma). The market maintains
//! an aggregate distribution. Trade cost = LMSR cost of shifting the aggregate.

use std::f64::consts::{PI, SQRT_2};

/// Maximum payout multiplier to prevent unbounded payouts
pub const MAX_PAYOUT_RATIO: f64 = 10.0;

/// Minimum sigma to prevent division by zero and degenerate distributions
pub const MIN_SIGMA: f64 = 1e-6;

/// Market state for LMSR calculations
#[derive(Debug, Clone)]
pub struct DistributionMarketState {
    pub mu: f64,          // current aggregate mean
    pub sigma: f64,       // current aggregate std dev
    pub liquidity_b: f64, // LMSR liquidity parameter (higher = more liquid)
    pub outcome_min: f64,
    pub outcome_max: f64,
}

/// Result of a trade cost calculation
#[derive(Debug, Clone)]
pub struct TradeCostResult {
    pub cost: f64,        // collateral required (positive = pay, negative = receive)
    pub new_mu: f64,      // aggregate mu after trade
    pub new_sigma: f64,   // aggregate sigma after trade
    pub delta_mu: f64,    // change in mu
    pub delta_sigma: f64, // change in sigma
    pub stiffness: f64,   // market resistance to further shifts
    pub peak_density: f64,
    pub headroom_pct: f64,
    pub lambda: f64,
}

/// Payout calculation result
#[derive(Debug, Clone)]
pub struct PayoutResult {
    pub payout_ratio: f64, // density ratio (may be > 1 for winners)
    pub gross_payout: f64, // before fees
    pub net_payout: f64,   // after fees
}

/// Curve point for visualization
#[derive(Debug, Clone, serde::Serialize)]
pub struct CurvePoint {
    pub x: f64,
    pub market_pdf: f64,
    pub proposal_pdf: Option<f64>,
    pub cdf: f64,
}

/// Error function approximation (Abramowitz & Stegun 7.1.26, max error 1.5e-7)
fn erf(x: f64) -> f64 {
    let sign = if x >= 0.0 { 1.0 } else { -1.0 };
    let x = x.abs();

    let p = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;

    let t = 1.0 / (1.0 + p * x);
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let t5 = t4 * t;

    let y = 1.0 - (a1 * t + a2 * t2 + a3 * t3 + a4 * t4 + a5 * t5) * (-x * x).exp();

    sign * y
}

/// KL divergence between two Gaussian distributions
/// D_KL(P || Q) where P = N(mu1, sigma1), Q = N(mu2, sigma2)
/// Returns 0.0 if either sigma is degenerate.
fn kl_divergence(mu1: f64, sigma1: f64, mu2: f64, sigma2: f64) -> f64 {
    if sigma1 < MIN_SIGMA || sigma2 < MIN_SIGMA {
        return 0.0;
    }
    let var1 = sigma1 * sigma1;
    let var2 = sigma2 * sigma2;
    let mu_diff = mu2 - mu1;
    (sigma2 / sigma1).ln() + (var1 + mu_diff * mu_diff) / (2.0 * var2) - 0.5
}

impl DistributionMarketState {
    /// Gaussian probability density function
    pub fn pdf(x: f64, mu: f64, sigma: f64) -> f64 {
        if sigma < MIN_SIGMA {
            return 0.0;
        }
        let z = (x - mu) / sigma;
        (1.0 / (sigma * (2.0 * PI).sqrt())) * (-0.5 * z * z).exp()
    }

    /// Gaussian cumulative distribution function using error function approximation
    pub fn cdf(x: f64, mu: f64, sigma: f64) -> f64 {
        if sigma < MIN_SIGMA {
            return if x >= mu { 1.0 } else { 0.0 };
        }
        0.5 * (1.0 + erf((x - mu) / (sigma * SQRT_2)))
    }

    /// LMSR cost of shifting the market distribution
    ///
    /// For Gaussian markets, the cost function is based on the KL divergence
    /// between the old and new distributions, scaled by the liquidity parameter:
    ///
    /// cost = b * D_KL(new || old)
    ///
    /// Where D_KL for two Gaussians is:
    /// D_KL = ln(sigma_old/sigma_new) + (sigma_new^2 + (mu_new - mu_old)^2) / (2*sigma_old^2) - 0.5
    pub fn trade_cost(&self, new_mu: f64, new_sigma: f64) -> TradeCostResult {
        // Guard against degenerate inputs
        let new_sigma = new_sigma.max(MIN_SIGMA);
        let current_sigma = self.sigma.max(MIN_SIGMA);

        // Compute KL divergence from old to new distribution
        let kl = kl_divergence(self.mu, current_sigma, new_mu, new_sigma);
        let cost = self.liquidity_b * kl;

        // Compute derived stats
        let stiffness = self.liquidity_b / (self.sigma * self.sigma);
        let peak_density = Self::pdf(new_mu, new_mu, new_sigma);
        let range = self.outcome_max - self.outcome_min;
        let headroom_pct = 1.0 - (new_sigma / (range / 2.0));
        let lambda = self.liquidity_b * peak_density;

        TradeCostResult {
            cost,
            new_mu,
            new_sigma,
            delta_mu: new_mu - self.mu,
            delta_sigma: new_sigma - self.sigma,
            stiffness,
            peak_density,
            headroom_pct,
            lambda,
        }
    }

    /// Calculate payout for a position given the resolved outcome
    pub fn calculate_payout(
        &self,
        position_mu: f64,
        position_sigma: f64,
        position_size: f64,
        resolved_value: f64,
        fee_bps: u32,
        discount_bps: u32,
    ) -> PayoutResult {
        let position_density = Self::pdf(resolved_value, position_mu, position_sigma);
        let market_density = Self::pdf(resolved_value, self.mu, self.sigma);

        // Avoid division by zero -- if market density is ~0 at resolved value,
        // that means it was an extreme surprise. Cap the ratio.
        let payout_ratio = if market_density < 1e-15 {
            if position_density < 1e-15 {
                1.0
            } else {
                MAX_PAYOUT_RATIO
            }
        } else {
            (position_density / market_density).min(MAX_PAYOUT_RATIO)
        };

        let gross_payout = position_size * payout_ratio;

        // Apply fee with discount
        let base_fee = gross_payout * (fee_bps as f64) / 10_000.0;
        let fee = base_fee - (base_fee * (discount_bps as f64) / 10_000.0);
        let net_payout = gross_payout - fee;

        PayoutResult {
            payout_ratio,
            gross_payout,
            net_payout: net_payout.max(0.0),
        }
    }

    /// Compute the stiffness of the current market state.
    /// Higher stiffness = more liquid, harder to move the distribution.
    pub fn stiffness(&self) -> f64 {
        let s = self.sigma.max(MIN_SIGMA);
        self.liquidity_b / (s * s)
    }

    /// Sensitivity: how much mu shifts per unit of cost
    pub fn mu_per_unit(&self) -> f64 {
        let s = self.sigma.max(MIN_SIGMA);
        if self.liquidity_b < MIN_SIGMA {
            return f64::MAX;
        }
        s * s / self.liquidity_b
    }

    /// Sensitivity: how much sigma shifts per unit of cost
    pub fn sigma_per_unit(&self) -> f64 {
        if self.liquidity_b < MIN_SIGMA {
            return f64::MAX;
        }
        self.sigma.max(MIN_SIGMA) / (2.0 * self.liquidity_b)
    }

    /// Generate curve data for visualization (N points between outcome_min and outcome_max)
    pub fn generate_curve(
        &self,
        n_points: usize,
        proposal_mu: Option<f64>,
        proposal_sigma: Option<f64>,
    ) -> Vec<CurvePoint> {
        let step = (self.outcome_max - self.outcome_min) / (n_points as f64 - 1.0);
        (0..n_points)
            .map(|i| {
                let x = self.outcome_min + step * i as f64;
                let market_pdf = Self::pdf(x, self.mu, self.sigma);
                let cdf_val = Self::cdf(x, self.mu, self.sigma);
                let proposal_pdf = match (proposal_mu, proposal_sigma) {
                    (Some(pm), Some(ps)) => Some(Self::pdf(x, pm, ps)),
                    _ => None,
                };
                CurvePoint {
                    x,
                    market_pdf,
                    proposal_pdf,
                    cdf: cdf_val,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-4;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    fn default_market() -> DistributionMarketState {
        DistributionMarketState {
            mu: 50.0,
            sigma: 10.0,
            liquidity_b: 100.0,
            outcome_min: 0.0,
            outcome_max: 100.0,
        }
    }

    // ---- PDF tests ----

    #[test]
    fn test_pdf_standard_normal_at_zero() {
        // For N(0,1), pdf(0) = 1/sqrt(2*pi) ~ 0.39894
        let val = DistributionMarketState::pdf(0.0, 0.0, 1.0);
        assert!(
            approx_eq(val, 0.3989, EPSILON),
            "pdf(0, 0, 1) = {val}, expected ~0.3989"
        );
    }

    #[test]
    fn test_pdf_symmetry() {
        let left = DistributionMarketState::pdf(-2.0, 0.0, 1.0);
        let right = DistributionMarketState::pdf(2.0, 0.0, 1.0);
        assert!(
            approx_eq(left, right, 1e-10),
            "PDF should be symmetric: left={left}, right={right}"
        );
    }

    #[test]
    fn test_pdf_peak_at_mean() {
        let at_mean = DistributionMarketState::pdf(5.0, 5.0, 2.0);
        let off_mean = DistributionMarketState::pdf(7.0, 5.0, 2.0);
        assert!(
            at_mean > off_mean,
            "PDF at mean ({at_mean}) should exceed PDF off-mean ({off_mean})"
        );
    }

    // ---- CDF tests ----

    #[test]
    fn test_cdf_at_mean() {
        let val = DistributionMarketState::cdf(0.0, 0.0, 1.0);
        assert!(
            approx_eq(val, 0.5, EPSILON),
            "cdf(0, 0, 1) = {val}, expected 0.5"
        );
    }

    #[test]
    fn test_cdf_at_one_sigma() {
        // cdf(1, 0, 1) ~ 0.8413
        let val = DistributionMarketState::cdf(1.0, 0.0, 1.0);
        assert!(
            approx_eq(val, 0.8413, EPSILON),
            "cdf(1, 0, 1) = {val}, expected ~0.8413"
        );
    }

    #[test]
    fn test_cdf_at_negative_one_sigma() {
        // cdf(-1, 0, 1) ~ 0.1587
        let val = DistributionMarketState::cdf(-1.0, 0.0, 1.0);
        assert!(
            approx_eq(val, 0.1587, EPSILON),
            "cdf(-1, 0, 1) = {val}, expected ~0.1587"
        );
    }

    #[test]
    fn test_cdf_monotonic() {
        let a = DistributionMarketState::cdf(-1.0, 0.0, 1.0);
        let b = DistributionMarketState::cdf(0.0, 0.0, 1.0);
        let c = DistributionMarketState::cdf(1.0, 0.0, 1.0);
        assert!(a < b && b < c, "CDF must be monotonically increasing");
    }

    // ---- erf tests ----

    #[test]
    fn test_erf_at_zero() {
        assert!(
            approx_eq(erf(0.0), 0.0, 1e-7),
            "erf(0) should be ~0, got {}",
            erf(0.0)
        );
    }

    #[test]
    fn test_erf_symmetry() {
        let pos = erf(1.0);
        let neg = erf(-1.0);
        assert!(
            approx_eq(pos, -neg, 1e-10),
            "erf should be odd: erf(1)={pos}, erf(-1)={neg}"
        );
    }

    #[test]
    fn test_erf_known_values() {
        // erf(1) ~ 0.8427
        assert!(
            approx_eq(erf(1.0), 0.8427, EPSILON),
            "erf(1) = {}, expected ~0.8427",
            erf(1.0)
        );
        // erf(2) ~ 0.9953
        assert!(
            approx_eq(erf(2.0), 0.9953, EPSILON),
            "erf(2) = {}, expected ~0.9953",
            erf(2.0)
        );
    }

    #[test]
    fn test_erf_large_input() {
        // erf(5) should be very close to 1
        assert!(
            approx_eq(erf(5.0), 1.0, 1e-6),
            "erf(5) = {}, expected ~1.0",
            erf(5.0)
        );
    }

    // ---- KL divergence tests ----

    #[test]
    fn test_kl_same_distribution_is_zero() {
        let kl = kl_divergence(5.0, 2.0, 5.0, 2.0);
        assert!(
            approx_eq(kl, 0.0, 1e-10),
            "KL divergence of identical distributions should be 0, got {kl}"
        );
    }

    #[test]
    fn test_kl_is_non_negative() {
        let kl = kl_divergence(0.0, 1.0, 1.0, 2.0);
        assert!(kl >= 0.0, "KL divergence should be non-negative, got {kl}");
    }

    #[test]
    fn test_kl_increases_with_mu_difference() {
        let kl_small = kl_divergence(0.0, 1.0, 0.5, 1.0);
        let kl_large = kl_divergence(0.0, 1.0, 2.0, 1.0);
        assert!(
            kl_large > kl_small,
            "Larger mu shift should give larger KL: small={kl_small}, large={kl_large}"
        );
    }

    // ---- Trade cost tests ----

    #[test]
    fn test_trade_cost_no_change_is_zero() {
        let market = default_market();
        let result = market.trade_cost(50.0, 10.0);
        assert!(
            approx_eq(result.cost, 0.0, 1e-10),
            "No-change trade should have zero cost, got {}",
            result.cost
        );
    }

    #[test]
    fn test_trade_cost_large_shift_costs_more() {
        let market = default_market();
        let small = market.trade_cost(51.0, 10.0);
        let large = market.trade_cost(55.0, 10.0);
        assert!(
            large.cost > small.cost,
            "Larger mu shift should cost more: small={}, large={}",
            small.cost,
            large.cost
        );
    }

    #[test]
    fn test_trade_cost_positive_for_any_change() {
        let market = default_market();
        let result = market.trade_cost(52.0, 9.0);
        assert!(
            result.cost > 0.0,
            "Any distribution change should have positive cost, got {}",
            result.cost
        );
    }

    #[test]
    fn test_trade_cost_deltas() {
        let market = default_market();
        let result = market.trade_cost(55.0, 8.0);
        assert!(approx_eq(result.delta_mu, 5.0, 1e-10));
        assert!(approx_eq(result.delta_sigma, -2.0, 1e-10));
        assert!(approx_eq(result.new_mu, 55.0, 1e-10));
        assert!(approx_eq(result.new_sigma, 8.0, 1e-10));
    }

    // ---- Payout tests ----

    #[test]
    fn test_payout_centered_position_wins() {
        let market = default_market();
        // Position centered exactly on resolved value with tighter sigma
        let result = market.calculate_payout(50.0, 5.0, 1000.0, 50.0, 100, 0);
        assert!(
            result.payout_ratio > 1.0,
            "Position centered on resolved value with tighter sigma should win: ratio={}",
            result.payout_ratio
        );
    }

    #[test]
    fn test_payout_off_center_position_loses() {
        let market = default_market();
        // Position far from resolved value
        let result = market.calculate_payout(80.0, 5.0, 1000.0, 50.0, 100, 0);
        assert!(
            result.payout_ratio < 1.0,
            "Position far from resolved value should lose: ratio={}",
            result.payout_ratio
        );
    }

    #[test]
    fn test_payout_fee_applied() {
        let market = default_market();
        let no_fee = market.calculate_payout(50.0, 5.0, 1000.0, 50.0, 0, 0);
        let with_fee = market.calculate_payout(50.0, 5.0, 1000.0, 50.0, 100, 0);
        assert!(
            with_fee.net_payout < no_fee.net_payout,
            "Fee should reduce payout: no_fee={}, with_fee={}",
            no_fee.net_payout,
            with_fee.net_payout
        );
    }

    #[test]
    fn test_payout_discount_reduces_fee() {
        let market = default_market();
        let full_fee = market.calculate_payout(50.0, 5.0, 1000.0, 50.0, 100, 0);
        let discounted = market.calculate_payout(50.0, 5.0, 1000.0, 50.0, 100, 5000);
        assert!(
            discounted.net_payout > full_fee.net_payout,
            "Discount should increase net payout: full={}, discounted={}",
            full_fee.net_payout,
            discounted.net_payout
        );
    }

    #[test]
    fn test_payout_ratio_capped_at_10x() {
        let market = DistributionMarketState {
            mu: 50.0,
            sigma: 20.0, // very wide market
            liquidity_b: 100.0,
            outcome_min: 0.0,
            outcome_max: 100.0,
        };
        // Very tight position exactly at resolved value vs very wide market
        let result = market.calculate_payout(50.0, 0.1, 1000.0, 50.0, 0, 0);
        assert!(
            result.payout_ratio <= 10.0,
            "Payout ratio should be capped at 10x, got {}",
            result.payout_ratio
        );
    }

    // ---- Stiffness tests ----

    #[test]
    fn test_stiffness_increases_with_liquidity() {
        let low_liq = DistributionMarketState {
            liquidity_b: 50.0,
            ..default_market()
        };
        let high_liq = DistributionMarketState {
            liquidity_b: 200.0,
            ..default_market()
        };
        assert!(
            high_liq.stiffness() > low_liq.stiffness(),
            "Higher liquidity_b should give higher stiffness: low={}, high={}",
            low_liq.stiffness(),
            high_liq.stiffness()
        );
    }

    #[test]
    fn test_stiffness_decreases_with_sigma() {
        let narrow = DistributionMarketState {
            sigma: 5.0,
            ..default_market()
        };
        let wide = DistributionMarketState {
            sigma: 20.0,
            ..default_market()
        };
        assert!(
            narrow.stiffness() > wide.stiffness(),
            "Narrower sigma should give higher stiffness: narrow={}, wide={}",
            narrow.stiffness(),
            wide.stiffness()
        );
    }

    // ---- Sensitivity tests ----

    #[test]
    fn test_mu_per_unit_inversely_proportional_to_liquidity() {
        let low = DistributionMarketState {
            liquidity_b: 50.0,
            ..default_market()
        };
        let high = DistributionMarketState {
            liquidity_b: 200.0,
            ..default_market()
        };
        assert!(
            low.mu_per_unit() > high.mu_per_unit(),
            "Lower liquidity should give more mu shift per cost unit"
        );
    }

    // ---- Curve generation tests ----

    #[test]
    fn test_generate_curve_correct_point_count() {
        let market = default_market();
        let curve = market.generate_curve(101, None, None);
        assert_eq!(curve.len(), 101, "Should generate exactly 101 points");
    }

    #[test]
    fn test_generate_curve_range() {
        let market = default_market();
        let curve = market.generate_curve(11, None, None);
        assert!(
            approx_eq(curve.first().unwrap().x, 0.0, 1e-10),
            "First point should be at outcome_min"
        );
        assert!(
            approx_eq(curve.last().unwrap().x, 100.0, 1e-10),
            "Last point should be at outcome_max"
        );
    }

    #[test]
    fn test_generate_curve_no_proposal() {
        let market = default_market();
        let curve = market.generate_curve(11, None, None);
        for point in &curve {
            assert!(
                point.proposal_pdf.is_none(),
                "Without proposal, proposal_pdf should be None"
            );
        }
    }

    #[test]
    fn test_generate_curve_with_proposal() {
        let market = default_market();
        let curve = market.generate_curve(11, Some(55.0), Some(8.0));
        for point in &curve {
            assert!(
                point.proposal_pdf.is_some(),
                "With proposal, proposal_pdf should be Some"
            );
        }
    }

    #[test]
    fn test_generate_curve_cdf_monotonic() {
        let market = default_market();
        let curve = market.generate_curve(101, None, None);
        for window in curve.windows(2) {
            assert!(
                window[1].cdf >= window[0].cdf,
                "CDF should be monotonically increasing: {} at x={} then {} at x={}",
                window[0].cdf,
                window[0].x,
                window[1].cdf,
                window[1].x
            );
        }
    }
}
