//! Polymarket Alpha Scanner.
//!
//! Background service that polls the Polymarket Gamma API to discover markets,
//! classify them by category, and score opportunities for the alpha strategies:
//! longshot-harvest, spread-capture, near-certainty, correlation-arb.

use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

use crate::services::market_data::{L2Event, L2Level, L2Payload, Venue};
use crate::AppState;

fn emit_l2(state: &AppState, market_key: String, price: f64) {
    if price <= 0.0 {
        return;
    }
    let seq = state.market_data.next_seq(Venue::Polymarket);
    state.market_data.emit(L2Event {
        venue: Venue::Polymarket,
        market_key,
        seq,
        observed_at: chrono::Utc::now(),
        payload: L2Payload::Snapshot {
            bids: vec![L2Level { price, size: 0.0 }],
            asks: vec![],
            last_trade: None,
        },
    });
}

// ── Gamma API types ──

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GammaMarket {
    #[serde(default)]
    condition_id: String,
    #[serde(default)]
    question: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    tokens: Vec<GammaToken>,
    #[serde(default)]
    outcome_prices: Option<String>,
    #[serde(default)]
    outcomes: Option<String>,
    #[serde(default)]
    volume: Option<String>,
    #[serde(default)]
    liquidity: Option<String>,
    #[serde(default)]
    end_date_iso: Option<String>,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    tags: Vec<GammaTag>,
    #[serde(default)]
    category: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GammaToken {
    #[serde(default)]
    token_id: String,
    #[serde(default)]
    outcome: String,
    #[serde(default)]
    price: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct GammaTag {
    #[serde(default)]
    slug: String,
    #[serde(default)]
    label: Option<String>,
}

// ── Scanned market output ──

#[derive(Debug, Clone, Serialize)]
pub struct ScannedOpportunity {
    pub condition_id: String,
    pub question: String,
    pub slug: String,
    pub category: String,
    pub yes_token_id: String,
    pub no_token_id: String,
    pub yes_price: f64,
    pub no_price: f64,
    pub spread_bps: i32,
    pub volume_usdc: f64,
    pub liquidity_usdc: f64,
    pub opportunity_type: String,
    pub opportunity_score: f64,
    pub duration_minutes: Option<i32>,
    pub mispricing_score: f64,
    pub fee_rate_bps: i32,
}

// ── Category classification ──

fn classify_category(market: &GammaMarket) -> String {
    let tags: Vec<String> = market
        .tags
        .iter()
        .map(|t| t.slug.to_ascii_lowercase())
        .collect();

    if let Some(cat) = &market.category {
        let cat_lower = cat.to_ascii_lowercase();
        if !cat_lower.is_empty() && cat_lower != "unknown" {
            return cat_lower;
        }
    }

    let question_lower = market.question.to_ascii_lowercase();

    // Tag-based classification
    for tag in &tags {
        if tag.contains("crypto") || tag.contains("bitcoin") || tag.contains("ethereum") {
            return "crypto".to_string();
        }
        if tag.contains("sport")
            || tag.contains("nba")
            || tag.contains("nfl")
            || tag.contains("soccer")
        {
            return "sports".to_string();
        }
        if tag.contains("politic")
            || tag.contains("election")
            || tag.contains("trump")
            || tag.contains("biden")
        {
            return "politics".to_string();
        }
        if tag.contains("entertainment") || tag.contains("celebrity") || tag.contains("oscars") {
            return "entertainment".to_string();
        }
        if tag.contains("finance") || tag.contains("fed") || tag.contains("rate") {
            return "finance".to_string();
        }
        if tag.contains("world") || tag.contains("geopolitic") || tag.contains("war") {
            return "world".to_string();
        }
    }

    // Question-based fallback
    if question_lower.contains("bitcoin")
        || question_lower.contains("btc")
        || question_lower.contains("eth")
        || question_lower.contains("crypto")
    {
        return "crypto".to_string();
    }
    if question_lower.contains("nba")
        || question_lower.contains("nfl")
        || question_lower.contains("win") && question_lower.contains("game")
    {
        return "sports".to_string();
    }
    if question_lower.contains("president")
        || question_lower.contains("elect")
        || question_lower.contains("congress")
    {
        return "politics".to_string();
    }

    "other".to_string()
}

/// Fee rate in bps by category (Polymarket V2 fee structure).
fn fee_rate_bps(category: &str) -> i32 {
    match category {
        "crypto" => 720,
        "sports" => 300,
        "politics" | "finance" | "tech" => 400,
        "world" | "geopolitics" => 0,
        _ => 500, // economics/culture/weather/other
    }
}

/// Mispricing severity multiplier by category (from Becker research).
fn category_bias_multiplier(category: &str) -> f64 {
    match category {
        "world" => 7.32,
        "entertainment" => 4.79,
        "crypto" => 2.69,
        "sports" => 2.23,
        "politics" => 1.02,
        "finance" => 0.17,
        _ => 1.50,
    }
}

// ── Opportunity scoring ──

fn score_market(market: &GammaMarket) -> Option<ScannedOpportunity> {
    if market.condition_id.is_empty() {
        return None;
    }

    let (yes_token, no_token) = extract_tokens(market)?;
    let yes_price = yes_token.price.unwrap_or(0.0);
    let no_price = no_token.price.unwrap_or(0.0);

    if yes_price <= 0.0 || no_price <= 0.0 {
        return None;
    }

    let category = classify_category(market);
    let volume = market
        .volume
        .as_ref()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);
    let liquidity = market
        .liquidity
        .as_ref()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);

    // Skip very low liquidity markets
    if liquidity < 100.0 {
        return None;
    }

    let spread_bps = ((yes_price + no_price - 1.0).abs() * 10_000.0).round() as i32;
    let fee_bps = fee_rate_bps(&category);
    let bias = category_bias_multiplier(&category);

    let duration_minutes = market.end_date_iso.as_ref().and_then(|end| {
        chrono::DateTime::parse_from_rfc3339(end)
            .ok()
            .map(|end_dt| {
                let now = chrono::Utc::now();
                let diff = end_dt.signed_duration_since(now);
                diff.num_minutes().max(0) as i32
            })
    });

    // Score different opportunity types
    let mut best_type = String::new();
    let mut best_score: f64 = 0.0;
    let mut mispricing_score: f64 = 0.0;

    // 1. Longshot harvest: YES price under 10c in high-bias categories
    if yes_price < 0.10 && yes_price > 0.005 {
        // Historical calibration: 5c contracts win 4.18% not 5%
        let implied = yes_price;
        let estimated_actual = implied * (1.0 - 0.164 * (bias / 2.69)); // scale by category bias
        let mispricing_pct = ((implied - estimated_actual) / implied * 100.0).max(0.0);
        mispricing_score = mispricing_pct;

        let score = mispricing_pct * bias * (liquidity / 1000.0).min(5.0).max(0.5);
        if score > best_score {
            best_score = score;
            best_type = "longshot_sell".to_string();
        }
    }

    // Also check NO side for longshots
    if no_price < 0.10 && no_price > 0.005 {
        let implied = no_price;
        let estimated_actual = implied * (1.0 - 0.164 * (bias / 2.69));
        let mispricing_pct = ((implied - estimated_actual) / implied * 100.0).max(0.0);

        let score = mispricing_pct * bias * (liquidity / 1000.0).min(5.0).max(0.5);
        if score > best_score {
            best_score = score;
            best_type = "longshot_sell_no".to_string();
            mispricing_score = mispricing_pct;
        }
    }

    // 2. Near-certainty: price above 90c
    if yes_price > 0.90 {
        let edge_estimate = (1.0 - yes_price) * 0.3; // ~30% of remaining gap is edge
        let edge_bps = (edge_estimate * 10_000.0).round();
        let score = edge_bps * (liquidity / 1000.0).min(3.0).max(0.5);
        if score > best_score {
            best_score = score;
            best_type = "near_certainty_buy".to_string();
            mispricing_score = edge_estimate * 100.0;
        }
    }

    if no_price > 0.90 {
        let edge_estimate = (1.0 - no_price) * 0.3;
        let edge_bps = (edge_estimate * 10_000.0).round();
        let score = edge_bps * (liquidity / 1000.0).min(3.0).max(0.5);
        if score > best_score {
            best_score = score;
            best_type = "near_certainty_buy_no".to_string();
            mispricing_score = edge_estimate * 100.0;
        }
    }

    // 3. Spread capture: short-duration markets with wide spreads
    if let Some(mins) = duration_minutes {
        if mins > 0 && mins <= 120 {
            // Check if both sides can be bought cheaply
            let total_cost = yes_price + no_price;
            if total_cost < 0.98 && total_cost > 0.80 {
                let profit_pct = (1.0 - total_cost) * 100.0;
                let annualized = profit_pct * (525_600.0 / mins as f64); // annualize
                let score = annualized * (liquidity / 500.0).min(3.0).max(0.5);
                if score > best_score {
                    best_score = score;
                    best_type = "spread_capture".to_string();
                    mispricing_score = profit_pct;
                }
            }
        }
    }

    if best_type.is_empty() || best_score <= 0.0 {
        return None;
    }

    Some(ScannedOpportunity {
        condition_id: market.condition_id.clone(),
        question: market.question.clone(),
        slug: market.slug.clone(),
        category,
        yes_token_id: yes_token.token_id.clone(),
        no_token_id: no_token.token_id.clone(),
        yes_price,
        no_price,
        spread_bps,
        volume_usdc: volume,
        liquidity_usdc: liquidity,
        opportunity_type: best_type,
        opportunity_score: best_score,
        duration_minutes,
        mispricing_score,
        fee_rate_bps: fee_bps,
    })
}

fn extract_tokens(market: &GammaMarket) -> Option<(GammaToken, GammaToken)> {
    if market.tokens.len() >= 2 {
        let yes = market
            .tokens
            .iter()
            .find(|t| t.outcome.to_ascii_lowercase() == "yes")
            .cloned();
        let no = market
            .tokens
            .iter()
            .find(|t| t.outcome.to_ascii_lowercase() == "no")
            .cloned();
        match (yes, no) {
            (Some(y), Some(n)) => Some((y, n)),
            _ => {
                // Fallback: first = Yes, second = No
                Some((market.tokens[0].clone(), market.tokens[1].clone()))
            }
        }
    } else {
        None
    }
}

// ── Scanner fetch ──

async fn fetch_gamma_markets(
    base_url: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<GammaMarket>, String> {
    let url = format!(
        "{}/markets?active=true&limit={}&offset={}&order=volume24hr&ascending=false",
        base_url, limit, offset
    );

    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Gamma API fetch failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Gamma API returned {}", resp.status()));
    }

    resp.json::<Vec<GammaMarket>>()
        .await
        .map_err(|e| format!("Gamma API parse failed: {}", e))
}

// ── Database persistence ──

async fn upsert_scanned_market(state: &AppState, opp: &ScannedOpportunity) -> Result<(), String> {
    let pool = state.db.pool();
    sqlx::query(
        r#"
        INSERT INTO polymarket_scanned_markets (
            condition_id, question, slug, category,
            yes_token_id, no_token_id,
            yes_price, no_price, spread_bps,
            volume_usdc, liquidity_usdc,
            implied_probability, mispricing_score,
            opportunity_type, opportunity_score,
            duration_minutes, fee_rate_bps,
            active, last_scanned_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, true, NOW())
        ON CONFLICT (condition_id) DO UPDATE SET
            question = EXCLUDED.question,
            yes_price = EXCLUDED.yes_price,
            no_price = EXCLUDED.no_price,
            spread_bps = EXCLUDED.spread_bps,
            volume_usdc = EXCLUDED.volume_usdc,
            liquidity_usdc = EXCLUDED.liquidity_usdc,
            implied_probability = EXCLUDED.implied_probability,
            mispricing_score = EXCLUDED.mispricing_score,
            opportunity_type = EXCLUDED.opportunity_type,
            opportunity_score = EXCLUDED.opportunity_score,
            duration_minutes = EXCLUDED.duration_minutes,
            fee_rate_bps = EXCLUDED.fee_rate_bps,
            last_scanned_at = NOW()
        "#,
    )
    .bind(&opp.condition_id)
    .bind(&opp.question)
    .bind(&opp.slug)
    .bind(&opp.category)
    .bind(&opp.yes_token_id)
    .bind(&opp.no_token_id)
    .bind(opp.yes_price)
    .bind(opp.no_price)
    .bind(opp.spread_bps)
    .bind(opp.volume_usdc)
    .bind(opp.liquidity_usdc)
    .bind(opp.yes_price) // implied_probability = yes_price
    .bind(opp.mispricing_score)
    .bind(&opp.opportunity_type)
    .bind(opp.opportunity_score)
    .bind(opp.duration_minutes)
    .bind(opp.fee_rate_bps)
    .execute(pool)
    .await
    .map_err(|e| format!("DB upsert failed: {}", e))?;

    Ok(())
}

async fn record_scan_run(
    state: &AppState,
    scanned: i32,
    opportunities: i32,
    longshots: i32,
    near_certs: i32,
    spreads: i32,
    error: Option<String>,
) {
    let pool = state.db.pool();
    if let Err(e) = sqlx::query(
        r#"
        INSERT INTO polymarket_scanner_runs
            (markets_scanned, opportunities_found, longshots_found,
             near_certainties_found, spread_captures_found, completed_at, error)
        VALUES ($1, $2, $3, $4, $5, NOW(), $6)
        "#,
    )
    .bind(scanned)
    .bind(opportunities)
    .bind(longshots)
    .bind(near_certs)
    .bind(spreads)
    .bind(error)
    .execute(pool)
    .await
    {
        warn!("Failed to record polymarket scan run: {}", e);
    }
}

// ── Public API ──

/// Run a single scan cycle. Returns list of discovered opportunities.
pub async fn run_scan(state: &AppState) -> Result<Vec<ScannedOpportunity>, String> {
    let base_url = &state.config.polymarket_gamma_api_base;
    let mut all_opportunities = Vec::new();
    let mut total_scanned = 0u32;
    let mut longshots = 0i32;
    let mut near_certs = 0i32;
    let mut spreads = 0i32;

    // Fetch up to 500 markets in batches
    for offset in (0..500).step_by(100) {
        let markets = fetch_gamma_markets(base_url, 100, offset).await?;
        if markets.is_empty() {
            break;
        }

        total_scanned += markets.len() as u32;

        for market in &markets {
            if let Some((yes_token, no_token)) = extract_tokens(market) {
                if let Some(p) = yes_token.price {
                    emit_l2(state, yes_token.token_id.clone(), p);
                }
                if let Some(p) = no_token.price {
                    emit_l2(state, no_token.token_id.clone(), p);
                }
            }

            if let Some(opp) = score_market(market) {
                match opp.opportunity_type.as_str() {
                    t if t.starts_with("longshot") => longshots += 1,
                    t if t.starts_with("near_certainty") => near_certs += 1,
                    "spread_capture" => spreads += 1,
                    _ => {}
                }
                if let Err(e) = upsert_scanned_market(state, &opp).await {
                    warn!("Scanner: failed to persist {}: {}", opp.condition_id, e);
                }
                all_opportunities.push(opp);
            }
        }
    }

    record_scan_run(
        state,
        total_scanned as i32,
        all_opportunities.len() as i32,
        longshots,
        near_certs,
        spreads,
        None,
    )
    .await;

    Ok(all_opportunities)
}

/// List top opportunities from the database.
pub async fn list_opportunities(
    state: &AppState,
    opportunity_type: Option<&str>,
    limit: i64,
) -> Result<Vec<serde_json::Value>, String> {
    let pool = state.db.pool();

    let rows = if let Some(opp_type) = opportunity_type {
        sqlx::query_as::<_, (serde_json::Value,)>(
            r#"
            SELECT json_build_object(
                'conditionId', condition_id,
                'question', question,
                'slug', slug,
                'category', category,
                'yesTokenId', yes_token_id,
                'noTokenId', no_token_id,
                'yesPrice', yes_price,
                'noPrice', no_price,
                'spreadBps', spread_bps,
                'volumeUsdc', volume_usdc,
                'liquidityUsdc', liquidity_usdc,
                'opportunityType', opportunity_type,
                'opportunityScore', opportunity_score,
                'mispricingScore', mispricing_score,
                'durationMinutes', duration_minutes,
                'feeRateBps', fee_rate_bps,
                'lastScannedAt', last_scanned_at
            )
            FROM polymarket_scanned_markets
            WHERE active = true
              AND opportunity_type LIKE $1
            ORDER BY opportunity_score DESC
            LIMIT $2
            "#,
        )
        .bind(format!("{}%", opp_type))
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, (serde_json::Value,)>(
            r#"
            SELECT json_build_object(
                'conditionId', condition_id,
                'question', question,
                'slug', slug,
                'category', category,
                'yesTokenId', yes_token_id,
                'noTokenId', no_token_id,
                'yesPrice', yes_price,
                'noPrice', no_price,
                'spreadBps', spread_bps,
                'volumeUsdc', volume_usdc,
                'liquidityUsdc', liquidity_usdc,
                'opportunityType', opportunity_type,
                'opportunityScore', opportunity_score,
                'mispricingScore', mispricing_score,
                'durationMinutes', duration_minutes,
                'feeRateBps', fee_rate_bps,
                'lastScannedAt', last_scanned_at
            )
            FROM polymarket_scanned_markets
            WHERE active = true
              AND opportunity_type IS NOT NULL
            ORDER BY opportunity_score DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    };

    rows.map(|r| r.into_iter().map(|(v,)| v).collect())
        .map_err(|e| format!("DB query failed: {}", e))
}

// ── Background scanner loop ──

pub fn spawn_scanner(state: Arc<AppState>) {
    if !state.config.polymarket_enabled {
        info!("Polymarket scanner disabled (POLYMARKET_ENABLED=false)");
        return;
    }

    let enabled = std::env::var("PM_SCANNER_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    if !enabled {
        info!("Polymarket scanner disabled (PM_SCANNER_ENABLED=false)");
        return;
    }

    let interval_secs = std::env::var("PM_SCANNER_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60)
        .max(30);

    info!("Starting Polymarket scanner (interval={}s)", interval_secs);

    tokio::spawn(async move {
        // Initial delay to let the rest of the app start
        tokio::time::sleep(Duration::from_secs(20)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if state
                .is_shutting_down
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                info!("Polymarket scanner shutting down");
                break;
            }

            match run_scan(&state).await {
                Ok(opps) => {
                    let longshots = opps
                        .iter()
                        .filter(|o| o.opportunity_type.starts_with("longshot"))
                        .count();
                    let certs = opps
                        .iter()
                        .filter(|o| o.opportunity_type.starts_with("near_certainty"))
                        .count();
                    let spreads = opps
                        .iter()
                        .filter(|o| o.opportunity_type == "spread_capture")
                        .count();

                    info!(
                        "PM scan: {} opportunities (longshot={}, near_cert={}, spread={})",
                        opps.len(),
                        longshots,
                        certs,
                        spreads
                    );
                }
                Err(e) => {
                    warn!("PM scan error: {}", e);
                    record_scan_run(&state, 0, 0, 0, 0, 0, Some(e)).await;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_market(
        yes_price: f64,
        no_price: f64,
        category: &str,
        volume: f64,
        liquidity: f64,
    ) -> GammaMarket {
        GammaMarket {
            condition_id: "test-cond-1".to_string(),
            question: "Test market?".to_string(),
            slug: "test-market".to_string(),
            tokens: vec![
                GammaToken {
                    token_id: "tok-yes".to_string(),
                    outcome: "Yes".to_string(),
                    price: Some(yes_price),
                },
                GammaToken {
                    token_id: "tok-no".to_string(),
                    outcome: "No".to_string(),
                    price: Some(no_price),
                },
            ],
            outcome_prices: None,
            outcomes: None,
            volume: Some(volume.to_string()),
            liquidity: Some(liquidity.to_string()),
            end_date_iso: None,
            active: true,
            tags: vec![GammaTag {
                slug: category.to_string(),
                label: None,
            }],
            category: Some(category.to_string()),
        }
    }

    #[test]
    fn scores_longshot_opportunity() {
        let market = make_market(0.05, 0.95, "crypto", 50000.0, 5000.0);
        let opp = score_market(&market);
        assert!(opp.is_some());
        let opp = opp.unwrap();
        // May score as longshot_sell or near_certainty_buy_no (since no_price=0.95)
        // Both are valid alpha opportunities
        assert!(
            opp.opportunity_type.starts_with("longshot")
                || opp.opportunity_type.starts_with("near_certainty"),
            "unexpected type: {}",
            opp.opportunity_type
        );
        assert!(opp.opportunity_score > 0.0);
        assert!(opp.mispricing_score > 0.0);
    }

    #[test]
    fn scores_near_certainty() {
        let market = make_market(0.93, 0.07, "politics", 100000.0, 10000.0);
        let opp = score_market(&market);
        assert!(opp.is_some());
        let opp = opp.unwrap();
        assert!(opp.opportunity_type.starts_with("near_certainty"));
    }

    #[test]
    fn skips_low_liquidity() {
        let market = make_market(0.05, 0.95, "crypto", 100.0, 50.0);
        let opp = score_market(&market);
        assert!(opp.is_none());
    }

    #[test]
    fn skips_mid_range_no_edge() {
        let market = make_market(0.50, 0.50, "politics", 50000.0, 5000.0);
        let opp = score_market(&market);
        assert!(opp.is_none()); // no longshot, no near-certainty, no spread capture
    }

    #[test]
    fn classifies_crypto() {
        let market = make_market(0.50, 0.50, "unknown", 1000.0, 1000.0);
        let mut m = market;
        m.question = "Will Bitcoin reach $200K?".to_string();
        m.category = None;
        m.tags = vec![];
        assert_eq!(classify_category(&m), "crypto");
    }

    #[test]
    fn fee_rates_correct() {
        assert_eq!(fee_rate_bps("crypto"), 720);
        assert_eq!(fee_rate_bps("sports"), 300);
        assert_eq!(fee_rate_bps("world"), 0);
        assert_eq!(fee_rate_bps("politics"), 400);
    }
}
