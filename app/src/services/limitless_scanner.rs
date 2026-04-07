//! Limitless Market Scanner.
//!
//! Background service that polls the Limitless API to discover and index
//! prediction markets. Also performs cross-venue matching against
//! Polymarket to enable arbitrage detection via the smart router.

use log::{info, warn};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

use crate::services::external::providers::limitless;
use crate::services::external::types::ExternalMarketSnapshot;
use crate::AppState;

// ── Scanner entry point ──

/// Spawn the Limitless market scanner background loop.
pub fn spawn_limitless_scanner(state: Arc<AppState>) {
    if !state.config.limitless_enabled {
        info!("Limitless scanner disabled (limitless_enabled=false)");
        return;
    }

    let enabled = std::env::var("LIMITLESS_SCANNER_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    if !enabled {
        info!("Limitless scanner disabled (LIMITLESS_SCANNER_ENABLED=false)");
        return;
    }

    let interval_secs = std::env::var("LIMITLESS_SCANNER_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60)
        .max(30);

    info!(
        "Starting Limitless scanner (interval={}s)",
        interval_secs
    );

    tokio::spawn(async move {
        // Wait for app startup.
        tokio::time::sleep(Duration::from_secs(25)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if state
                .is_shutting_down
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                info!("Limitless scanner shutting down");
                break;
            }

            match run_scan(&state).await {
                Ok((indexed, opportunities, venue_matches)) => {
                    if indexed > 0 || opportunities > 0 {
                        info!(
                            "Limitless scan: indexed={} opportunities={} venue_matches={}",
                            indexed, opportunities, venue_matches
                        );
                    }
                }
                Err(e) => {
                    warn!("Limitless scan error: {}", e);
                    record_scan_run(&state, 0, 0, 0, 0, Some(&e)).await;
                }
            }
        }
    });
}

// ── Scan execution ──

async fn run_scan(
    state: &AppState,
) -> Result<(i32, i32, i32), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;

    // Fetch all active markets (paginate up to 200).
    let mut all_markets: Vec<ExternalMarketSnapshot> = Vec::new();
    for page_offset in (0..200).step_by(25) {
        let batch = limitless::fetch_active_markets(
            &client,
            state.config.limitless_api_base.as_str(),
            25,
            page_offset,
        )
        .await
        .map_err(|e| format!("Fetch limitless markets: {}", e.message))?;

        if batch.is_empty() {
            break;
        }
        all_markets.extend(batch);
    }

    let total_scanned = all_markets.len() as i32;
    let mut indexed = 0i32;
    let mut opportunities = 0i32;

    for market in &all_markets {
        let slug = match market.id.split_once(':') {
            Some((_, s)) => s,
            None => continue,
        };

        let spread_bps = ((market.yes_price + market.no_price - 1.0).abs() * 10_000.0)
            .round() as i32;

        let (opp_type, opp_score) = score_opportunity(market);
        if opp_score > 0.0 {
            opportunities += 1;
        }

        if let Err(e) = upsert_scanned_market(
            state,
            slug,
            &market.question,
            &market.category,
            market.yes_price,
            market.no_price,
            spread_bps,
            market.volume,
            0.0, // liquidity computed separately
            market.close_time as i64,
            &opp_type,
            opp_score,
            &market.provider_market_ref,
        )
        .await
        {
            warn!("Failed to persist limitless market {}: {}", slug, e);
            continue;
        }
        indexed += 1;
    }

    // Cross-venue matching against Polymarket.
    let venue_matches = match_cross_venue(state, &all_markets).await;

    record_scan_run(
        state,
        total_scanned,
        indexed,
        opportunities,
        venue_matches,
        None,
    )
    .await;

    Ok((indexed, opportunities, venue_matches))
}

// ── Opportunity scoring ──

fn score_opportunity(market: &ExternalMarketSnapshot) -> (String, f64) {
    let yes = market.yes_price;
    let no = market.no_price;

    // Longshot: YES under 10c
    if yes > 0.0 && yes <= 0.10 {
        // Becker research: longshots are overpriced by ~16%
        let mispricing = (yes - yes * 0.836) / yes * 100.0;
        if mispricing > 10.0 {
            return ("longshot_sell".to_string(), mispricing.min(100.0));
        }
    }

    // Longshot on NO side
    if no > 0.0 && no <= 0.10 {
        let mispricing = (no - no * 0.836) / no * 100.0;
        if mispricing > 10.0 {
            return ("longshot_sell_no".to_string(), mispricing.min(100.0));
        }
    }

    // Near-certainty: price above 90c
    if yes >= 0.90 {
        // Research: 95c wins ~96.5%
        let calibrated = yes + (1.0 - yes) * 0.3;
        let edge_bps = ((calibrated - yes) * 10_000.0).round();
        if edge_bps >= 50.0 {
            return ("near_certainty_buy_yes".to_string(), edge_bps.min(500.0));
        }
    }
    if no >= 0.90 {
        let calibrated = no + (1.0 - no) * 0.3;
        let edge_bps = ((calibrated - no) * 10_000.0).round();
        if edge_bps >= 50.0 {
            return ("near_certainty_buy_no".to_string(), edge_bps.min(500.0));
        }
    }

    // Spread capture: total cost under 98c
    let total_cost = yes + no;
    if total_cost < 0.98 && total_cost > 0.0 {
        let profit_bps = ((1.0 - total_cost) / total_cost * 10_000.0).round();
        if profit_bps >= 50.0 {
            return ("spread_capture".to_string(), profit_bps.min(1000.0));
        }
    }

    ("none".to_string(), 0.0)
}

// ── Cross-venue matching ──

/// Compare Limitless markets against Polymarket scanned markets.
/// When a match is found, upsert into market_venue_links so the
/// smart router arb scanner can detect price discrepancies.
async fn match_cross_venue(
    state: &AppState,
    limitless_markets: &[ExternalMarketSnapshot],
) -> i32 {
    // Load polymarket scanned questions with end_date epoch for close_time validation.
    let poly_rows = match sqlx::query_as::<_, (String, String, Option<String>, Option<f64>)>(
        "SELECT condition_id, question, slug, EXTRACT(EPOCH FROM end_date)::FLOAT8 \
         FROM polymarket_scanned_markets WHERE active = TRUE",
    )
    .fetch_all(state.db.pool())
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            warn!("Cross-venue: failed to load polymarket markets: {}", e);
            return 0;
        }
    };

    if poly_rows.is_empty() {
        return 0;
    }

    const CLOSE_TIME_MAX_DELTA_SECS: u64 = 7 * 24 * 3600; // 7 days

    let mut matches = 0i32;

    for lm in limitless_markets {
        let lm_slug = match lm.id.split_once(':') {
            Some((_, s)) => s,
            None => continue,
        };
        let lm_normalized = normalize_question(&lm.question);

        for (poly_condition_id, poly_question, poly_slug, poly_end_epoch) in &poly_rows {
            let poly_normalized = normalize_question(poly_question);

            // Match if normalized questions are similar enough.
            let similarity = question_similarity(&lm_normalized, &poly_normalized);
            if similarity < 0.7 {
                continue;
            }

            // Check close_time proximity (within 7 days).
            // Skip check if either is 0/null (perpetual or unknown expiry).
            let lm_close = lm.close_time;
            if lm_close > 0 {
                if let Some(&poly_ts) = poly_end_epoch.as_ref() {
                    let poly_secs = poly_ts as u64;
                    if poly_secs > 0 {
                        let delta = if lm_close > poly_secs {
                            lm_close - poly_secs
                        } else {
                            poly_secs - lm_close
                        };
                        if delta > CLOSE_TIME_MAX_DELTA_SECS {
                            continue;
                        }
                    }
                }
            }

            let market_slug = build_venue_slug(lm_slug, poly_slug.as_deref());

            // Upsert Limitless venue link.
            if let Err(e) = upsert_venue_link(
                state,
                &market_slug,
                "limitless",
                lm_slug,
                0, // Limitless maker fee = 0
            )
            .await
            {
                warn!("Cross-venue: limitless link failed: {}", e);
                continue;
            }

            // Upsert Polymarket venue link.
            if let Err(e) = upsert_venue_link(
                state,
                &market_slug,
                "polymarket",
                poly_condition_id,
                0, // Polymarket maker fee = 0
            )
            .await
            {
                warn!("Cross-venue: polymarket link failed: {}", e);
                continue;
            }

            matches += 1;
            info!(
                "Cross-venue match: '{}' ↔ '{}' (sim={:.2}, slug={})",
                lm.question.chars().take(50).collect::<String>(),
                poly_question.chars().take(50).collect::<String>(),
                similarity,
                market_slug
            );
        }
    }

    matches
}

/// Normalize a question for comparison: lowercase, strip punctuation, collapse whitespace.
fn normalize_question(q: &str) -> String {
    q.chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == ' ' {
                Some(c.to_ascii_lowercase())
            } else {
                None
            }
        })
        .collect::<String>()
        .split_whitespace()
        .filter(|w| !matches!(*w, "will" | "the" | "a" | "an" | "be" | "by" | "in" | "on" | "of" | "to" | "is" | "it"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Jaccard similarity between two normalized question strings.
fn question_similarity(a: &str, b: &str) -> f64 {
    let set_a: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let set_b: std::collections::HashSet<&str> = b.split_whitespace().collect();

    if set_a.is_empty() || set_b.is_empty() {
        return 0.0;
    }

    let intersection = set_a.intersection(&set_b).count() as f64;
    let union = set_a.union(&set_b).count() as f64;

    if union == 0.0 {
        return 0.0;
    }

    intersection / union
}

/// Build a canonical slug for cross-venue matching.
fn build_venue_slug(limitless_slug: &str, poly_slug: Option<&str>) -> String {
    // Prefer the shorter slug as canonical.
    match poly_slug {
        Some(ps) if !ps.is_empty() && ps.len() <= limitless_slug.len() => ps.to_string(),
        _ => limitless_slug.to_string(),
    }
}

// ── Database operations ──

async fn upsert_scanned_market(
    state: &AppState,
    slug: &str,
    question: &str,
    category: &str,
    yes_price: f64,
    no_price: f64,
    spread_bps: i32,
    volume_usdc: f64,
    liquidity_usdc: f64,
    close_time: i64,
    opportunity_type: &str,
    opportunity_score: f64,
    provider_market_ref: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO limitless_scanned_markets (
            slug, question, category,
            yes_price, no_price, spread_bps,
            volume_usdc, liquidity_usdc, close_time,
            opportunity_type, opportunity_score, provider_market_ref,
            active, last_scanned_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, TRUE, NOW())
        ON CONFLICT (slug) DO UPDATE SET
            question = EXCLUDED.question,
            category = EXCLUDED.category,
            yes_price = EXCLUDED.yes_price,
            no_price = EXCLUDED.no_price,
            spread_bps = EXCLUDED.spread_bps,
            volume_usdc = EXCLUDED.volume_usdc,
            liquidity_usdc = EXCLUDED.liquidity_usdc,
            close_time = EXCLUDED.close_time,
            opportunity_type = EXCLUDED.opportunity_type,
            opportunity_score = EXCLUDED.opportunity_score,
            provider_market_ref = EXCLUDED.provider_market_ref,
            active = TRUE,
            last_scanned_at = NOW()
        "#,
    )
    .bind(slug)
    .bind(question)
    .bind(category)
    .bind(yes_price)
    .bind(no_price)
    .bind(spread_bps)
    .bind(volume_usdc)
    .bind(liquidity_usdc)
    .bind(close_time)
    .bind(opportunity_type)
    .bind(opportunity_score)
    .bind(provider_market_ref)
    .execute(state.db.pool())
    .await
    .map_err(|e| format!("DB upsert: {}", e))?;

    Ok(())
}

async fn upsert_venue_link(
    state: &AppState,
    market_slug: &str,
    provider: &str,
    provider_market_id: &str,
    fee_bps: i32,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO market_venue_links (market_slug, provider, provider_market_id, fee_bps)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (market_slug, provider) DO UPDATE SET
            provider_market_id = EXCLUDED.provider_market_id,
            fee_bps = EXCLUDED.fee_bps,
            active = TRUE,
            updated_at = NOW()
        "#,
    )
    .bind(market_slug)
    .bind(provider)
    .bind(provider_market_id)
    .bind(fee_bps)
    .execute(state.db.pool())
    .await
    .map_err(|e| format!("Venue link upsert: {}", e))?;

    Ok(())
}

async fn record_scan_run(
    state: &AppState,
    scanned: i32,
    indexed: i32,
    opportunities: i32,
    venue_matches: i32,
    error: Option<&str>,
) {
    if let Err(e) = sqlx::query(
        r#"
        INSERT INTO limitless_scanner_runs
            (markets_scanned, markets_indexed, opportunities_found, venue_matches_found, completed_at, error)
        VALUES ($1, $2, $3, $4, NOW(), $5)
        "#,
    )
    .bind(scanned)
    .bind(indexed)
    .bind(opportunities)
    .bind(venue_matches)
    .bind(error)
    .execute(state.db.pool())
    .await
    {
        warn!("Failed to record limitless scan run: {}", e);
    }
}

// ── Public API for querying scanned markets ──

/// List scanned Limitless markets, optionally filtered by opportunity type.
pub async fn list_scanned_markets(
    state: &AppState,
    opportunity_type: Option<&str>,
    limit: i64,
) -> Result<Vec<serde_json::Value>, String> {
    let rows = if let Some(opp_type) = opportunity_type {
        sqlx::query_as::<_, (String, String, Option<String>, f64, f64, i32, f64, f64, i64, String, f64, Option<String>)>(
            r#"
            SELECT slug, question, category,
                   yes_price, no_price, spread_bps,
                   volume_usdc, liquidity_usdc, close_time,
                   opportunity_type, opportunity_score, provider_market_ref
            FROM limitless_scanned_markets
            WHERE active = TRUE AND opportunity_type = $1
            ORDER BY opportunity_score DESC
            LIMIT $2
            "#,
        )
        .bind(opp_type)
        .bind(limit)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as::<_, (String, String, Option<String>, f64, f64, i32, f64, f64, i64, String, f64, Option<String>)>(
            r#"
            SELECT slug, question, category,
                   yes_price, no_price, spread_bps,
                   volume_usdc, liquidity_usdc, close_time,
                   opportunity_type, opportunity_score, provider_market_ref
            FROM limitless_scanned_markets
            WHERE active = TRUE AND opportunity_score > 0
            ORDER BY opportunity_score DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| e.to_string())?
    };

    Ok(rows
        .into_iter()
        .map(|(slug, question, category, yes_price, no_price, spread_bps, volume, liquidity, close_time, opp_type, opp_score, pmr)| {
            json!({
                "slug": slug,
                "question": question,
                "category": category,
                "yesPrice": yes_price,
                "noPrice": no_price,
                "spreadBps": spread_bps,
                "volumeUsdc": volume,
                "liquidityUsdc": liquidity,
                "closeTime": close_time,
                "opportunityType": opp_type,
                "opportunityScore": opp_score,
                "providerMarketRef": pmr,
                "provider": "limitless",
                "marketId": format!("limitless:{}", slug)
            })
        })
        .collect())
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_stopwords() {
        let q = "Will the price of Bitcoin reach $100K by 2026?";
        let normalized = normalize_question(q);
        assert!(!normalized.contains("will"));
        assert!(!normalized.contains("the"));
        assert!(normalized.contains("bitcoin"));
        assert!(normalized.contains("100k"));
    }

    #[test]
    fn similarity_identical() {
        let a = normalize_question("Bitcoin price above 100K");
        let b = normalize_question("Bitcoin price above 100K");
        assert!((question_similarity(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn similarity_related() {
        let a = normalize_question("Will Bitcoin hit $100,000 by December 2026?");
        let b = normalize_question("Bitcoin to reach $100,000 before end of 2026?");
        let sim = question_similarity(&a, &b);
        assert!(sim > 0.3, "Expected similarity > 0.3, got {}", sim);
    }

    #[test]
    fn similarity_unrelated() {
        let a = normalize_question("Who will win the 2026 US presidential election?");
        let b = normalize_question("Will Ethereum flip Bitcoin by market cap?");
        let sim = question_similarity(&a, &b);
        assert!(sim < 0.3, "Expected similarity < 0.3, got {}", sim);
    }

    #[test]
    fn score_longshot() {
        let market = ExternalMarketSnapshot {
            id: "limitless:test".to_string(),
            question: "Test".to_string(),
            description: String::new(),
            category: "crypto".to_string(),
            status: "active".to_string(),
            close_time: 0,
            resolved: false,
            outcome: None,
            yes_price: 0.05,
            no_price: 0.95,
            volume: 1000.0,
            source: "external_limitless".to_string(),
            provider: "limitless".to_string(),
            is_external: true,
            external_url: String::new(),
            chain_id: 8453,
            requires_credentials: true,
            execution_users: true,
            execution_agents: true,
            outcomes: vec![],
            provider_market_ref: String::new(),
        };
        let (opp_type, score) = score_opportunity(&market);
        assert_eq!(opp_type, "longshot_sell");
        assert!(score > 0.0);
    }

    #[test]
    fn score_near_certainty() {
        let market = ExternalMarketSnapshot {
            id: "limitless:test".to_string(),
            question: "Test".to_string(),
            description: String::new(),
            category: "crypto".to_string(),
            status: "active".to_string(),
            close_time: 0,
            resolved: false,
            outcome: None,
            yes_price: 0.95,
            no_price: 0.05,
            volume: 1000.0,
            source: "external_limitless".to_string(),
            provider: "limitless".to_string(),
            is_external: true,
            external_url: String::new(),
            chain_id: 8453,
            requires_credentials: true,
            execution_users: true,
            execution_agents: true,
            outcomes: vec![],
            provider_market_ref: String::new(),
        };
        let (opp_type, score) = score_opportunity(&market);
        assert!(
            opp_type == "near_certainty_buy_yes" || opp_type == "longshot_sell_no",
            "Expected near_certainty or longshot, got {}",
            opp_type
        );
        assert!(score > 0.0);
    }

    #[test]
    fn score_spread_capture() {
        let market = ExternalMarketSnapshot {
            id: "limitless:test".to_string(),
            question: "Test".to_string(),
            description: String::new(),
            category: "crypto".to_string(),
            status: "active".to_string(),
            close_time: 0,
            resolved: false,
            outcome: None,
            yes_price: 0.48,
            no_price: 0.48,
            volume: 1000.0,
            source: "external_limitless".to_string(),
            provider: "limitless".to_string(),
            is_external: true,
            external_url: String::new(),
            chain_id: 8453,
            requires_credentials: true,
            execution_users: true,
            execution_agents: true,
            outcomes: vec![],
            provider_market_ref: String::new(),
        };
        let (opp_type, score) = score_opportunity(&market);
        assert_eq!(opp_type, "spread_capture");
        assert!(score > 0.0);
    }

    #[test]
    fn venue_slug_prefers_shorter() {
        assert_eq!(
            build_venue_slug("btc-above-100k-2026", Some("btc-100k")),
            "btc-100k"
        );
        assert_eq!(
            build_venue_slug("short", Some("longer-slug-here")),
            "short"
        );
        assert_eq!(
            build_venue_slug("only-limitless", None),
            "only-limitless"
        );
    }
}
