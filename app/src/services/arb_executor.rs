use std::sync::Arc;

use log::{info, warn};
use tokio::sync::mpsc;

use super::hedge_engine;
use super::market_data::cache;
use super::market_data::Venue;
use super::risk_governor;
use super::telegram_format::TelegramClient;
use crate::AppState;

pub struct ArbSignal {
    pub market_slug: String,
    pub outcome: String,
    pub buy_venue: Venue,
    pub buy_price: f64,
    pub buy_market_key: String,
    pub sell_venue: Venue,
    pub sell_price: f64,
    pub sell_market_key: String,
    pub buy_depth_usdc: f64,
    pub sell_depth_usdc: f64,
}

struct ArbConfig {
    paper_mode: bool,
    owner: String,
    pm_credential_id: Option<String>,
    lim_credential_id: Option<String>,
    max_trade_usdc: f64,
    min_trade_usdc: f64,
}

struct LegResult {
    provider: String,
    provider_market_id: String,
    side: u8,
    price: f64,
    quantity_usdc: f64,
    order_id: Option<String>,
    status: String,
    error: Option<String>,
}

pub fn spawn_from_channel(state: Arc<AppState>, mut rx: mpsc::Receiver<ArbSignal>) {
    let config = ArbConfig {
        paper_mode: env_bool("CROSS_VENUE_ARB_PAPER_MODE", true),
        owner: std::env::var("CROSS_VENUE_ARB_OWNER").unwrap_or_else(|_| "arb".to_string()),
        pm_credential_id: std::env::var("CROSS_VENUE_ARB_PM_CREDENTIAL_ID").ok(),
        lim_credential_id: std::env::var("CROSS_VENUE_ARB_LIM_CREDENTIAL_ID").ok(),
        max_trade_usdc: env_f64("CROSS_VENUE_ARB_MAX_TRADE_USDC", 25.0),
        min_trade_usdc: env_f64("CROSS_VENUE_ARB_MIN_TRADE_USDC", 1.0),
    };

    let tg = std::env::var("TELEGRAM_BOT_TOKEN")
        .ok()
        .zip(std::env::var("TELEGRAM_CHAT_ID").ok())
        .map(|(t, c)| TelegramClient::new(t, c));

    info!(
        "Arb executor started (paper={}, owner={}, max={})",
        config.paper_mode, config.owner, config.max_trade_usdc
    );

    tokio::spawn(async move {
        while let Some(signal) = rx.recv().await {
            if let Err(e) = try_execute(&state, &signal, &config, tg.as_ref()).await {
                warn!("arb_executor: {}", e);
            }
        }
        info!("arb_executor: channel closed, exiting");
    });
}

async fn try_execute(
    state: &AppState,
    signal: &ArbSignal,
    config: &ArbConfig,
    tg: Option<&TelegramClient>,
) -> Result<(), String> {
    let size = compute_size(signal, config);
    if size < config.min_trade_usdc {
        return Ok(());
    }

    let risk = risk_governor::check_order(state.db.pool(), &config.owner, size)
        .await
        .map_err(|e| format!("risk check: {}", e))?;
    if !risk.allowed {
        info!(
            "arb_executor: blocked by risk governor: {}",
            risk.reason.as_deref().unwrap_or("unknown")
        );
        return Ok(());
    }

    let (buy_provider, buy_market_id) =
        resolve_provider_market(state, &signal.market_slug, signal.buy_venue).await?;
    let (sell_provider, sell_market_id) =
        resolve_provider_market(state, &signal.market_slug, signal.sell_venue).await?;

    let arb_id = insert_opportunity(state, signal, size, config).await?;

    if config.paper_mode {
        update_paper(state, arb_id, signal, size).await;
        if let Some(tg) = tg {
            let msg = format_paper_msg(signal, size);
            let _ = tg.send(&msg).await;
        }
        return Ok(());
    }

    let buy_cred = credential_for_venue(config, signal.buy_venue);
    let sell_cred = credential_for_venue(config, signal.sell_venue);

    let (buy_res, sell_res) = tokio::join!(
        execute_leg(
            state,
            &buy_provider,
            &buy_market_id,
            buy_cred,
            &signal.outcome,
            0,
            signal.buy_price,
            size,
        ),
        execute_leg(
            state,
            &sell_provider,
            &sell_market_id,
            sell_cred,
            &signal.outcome,
            1,
            signal.sell_price,
            size,
        ),
    );

    let buy_leg = leg_result(&buy_provider, &buy_market_id, 0, signal.buy_price, size, &buy_res);
    let sell_leg =
        leg_result(&sell_provider, &sell_market_id, 1, signal.sell_price, size, &sell_res);

    persist_legs(state, arb_id, &buy_leg, &sell_leg).await;

    let status = match (&buy_res, &sell_res) {
        (Ok(_), Ok(_)) => "executed",
        (Ok(_), Err(_)) | (Err(_), Ok(_)) => "partial",
        (Err(_), Err(_)) => "failed",
    };
    update_status(state, arb_id, status).await;

    if buy_res.is_ok() || sell_res.is_ok() {
        let _ =
            risk_governor::record_fill(state.db.pool(), &config.owner, size, 0.0).await;
    }

    if let Some(tg) = tg {
        let msg = format_exec_msg(signal, size, status, &buy_leg, &sell_leg);
        let _ = tg.send(&msg).await;
    }

    Ok(())
}

fn compute_size(signal: &ArbSignal, config: &ArbConfig) -> f64 {
    signal
        .buy_depth_usdc
        .min(signal.sell_depth_usdc)
        .min(config.max_trade_usdc)
}

fn credential_for_venue<'a>(config: &'a ArbConfig, venue: Venue) -> Option<&'a str> {
    match venue {
        Venue::Polymarket => config.pm_credential_id.as_deref(),
        Venue::Limitless => config.lim_credential_id.as_deref(),
        _ => None,
    }
}

async fn resolve_provider_market(
    state: &AppState,
    market_slug: &str,
    venue: Venue,
) -> Result<(String, String), String> {
    let provider = venue.as_str();
    let row: (String,) = sqlx::query_as(
        "SELECT provider_market_id FROM market_venue_links \
         WHERE market_slug = $1 AND provider = $2 AND active = true",
    )
    .bind(market_slug)
    .bind(provider)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| format!("resolve {}/{}: {}", market_slug, provider, e))?;

    Ok((provider.to_string(), row.0))
}

async fn execute_leg(
    state: &AppState,
    provider: &str,
    market_id: &str,
    credential_id: Option<&str>,
    outcome: &str,
    side: u8,
    price: f64,
    quantity: f64,
) -> Result<hedge_engine::HedgeResult, String> {
    match provider {
        "limitless" => {
            hedge_engine::execute_limitless_order(
                state,
                market_id,
                credential_id,
                outcome,
                side,
                price,
                quantity,
            )
            .await
        }
        "polymarket" => {
            hedge_engine::execute_polymarket_order(
                state,
                market_id,
                credential_id,
                outcome,
                side,
                price,
                quantity,
            )
            .await
        }
        _ => Err(format!("arb_executor: unsupported provider {}", provider)),
    }
}

fn leg_result(
    provider: &str,
    market_id: &str,
    side: u8,
    price: f64,
    qty: f64,
    res: &Result<hedge_engine::HedgeResult, String>,
) -> LegResult {
    match res {
        Ok(hr) => LegResult {
            provider: provider.to_string(),
            provider_market_id: market_id.to_string(),
            side,
            price,
            quantity_usdc: qty,
            order_id: hr.provider_order_id.clone(),
            status: "submitted".to_string(),
            error: None,
        },
        Err(e) => LegResult {
            provider: provider.to_string(),
            provider_market_id: market_id.to_string(),
            side,
            price,
            quantity_usdc: qty,
            order_id: None,
            status: "failed".to_string(),
            error: Some(e.clone()),
        },
    }
}

async fn insert_opportunity(
    state: &AppState,
    signal: &ArbSignal,
    size: f64,
    config: &ArbConfig,
) -> Result<i32, String> {
    let (buy_prov, sell_prov) = (signal.buy_venue.as_str(), signal.sell_venue.as_str());
    let spread_bps = ((signal.sell_price - signal.buy_price) * 10_000.0).round() as i32;
    let mode = if config.paper_mode { "paper" } else { "live" };

    let row: (i32,) = sqlx::query_as(
        "INSERT INTO arbitrage_opportunities \
         (market_slug, outcome, buy_provider, buy_price, sell_provider, sell_price, \
          spread_bps, max_size_usdc, status, execution_mode) \
         VALUES ($1, $2, $3, $4::NUMERIC, $5, $6::NUMERIC, $7, $8::NUMERIC, 'executing', $9) \
         RETURNING id",
    )
    .bind(&signal.market_slug)
    .bind(&signal.outcome)
    .bind(buy_prov)
    .bind(format!("{}", signal.buy_price))
    .bind(sell_prov)
    .bind(format!("{}", signal.sell_price))
    .bind(spread_bps)
    .bind(format!("{}", size))
    .bind(mode)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| format!("insert arb opportunity: {}", e))?;

    Ok(row.0)
}

async fn update_paper(state: &AppState, arb_id: i32, signal: &ArbSignal, size: f64) {
    let _ = sqlx::query(
        "UPDATE arbitrage_opportunities \
         SET status = 'paper', \
             buy_leg_status = 'paper', sell_leg_status = 'paper', \
             pnl_usdc = $2::NUMERIC \
         WHERE id = $1",
    )
    .bind(arb_id)
    .bind(format!(
        "{}",
        (signal.sell_price - signal.buy_price) * size
    ))
    .execute(state.db.pool())
    .await;
}

async fn persist_legs(state: &AppState, arb_id: i32, buy: &LegResult, sell: &LegResult) {
    for leg in [("buy", buy), ("sell", sell)] {
        let _ = sqlx::query(
            "INSERT INTO arb_execution_legs \
             (arb_id, leg, provider, provider_market_id, side, price, quantity_usdc, \
              provider_order_id, status, error_message, submitted_at) \
             VALUES ($1, $2, $3, $4, $5, $6::NUMERIC, $7::NUMERIC, $8, $9, $10, NOW())",
        )
        .bind(arb_id)
        .bind(leg.0)
        .bind(&leg.1.provider)
        .bind(&leg.1.provider_market_id)
        .bind(leg.1.side as i16)
        .bind(format!("{}", leg.1.price))
        .bind(format!("{}", leg.1.quantity_usdc))
        .bind(&leg.1.order_id)
        .bind(&leg.1.status)
        .bind(&leg.1.error)
        .execute(state.db.pool())
        .await;
    }

    let _ = sqlx::query(
        "UPDATE arbitrage_opportunities SET \
         buy_order_id = $2, sell_order_id = $3, \
         buy_leg_status = $4, sell_leg_status = $5 \
         WHERE id = $1",
    )
    .bind(arb_id)
    .bind(&buy.order_id)
    .bind(&sell.order_id)
    .bind(&buy.status)
    .bind(&sell.status)
    .execute(state.db.pool())
    .await;
}

async fn update_status(state: &AppState, arb_id: i32, status: &str) {
    let _ = sqlx::query(
        "UPDATE arbitrage_opportunities SET status = $2, executed_at = NOW() WHERE id = $1",
    )
    .bind(arb_id)
    .bind(status)
    .execute(state.db.pool())
    .await;
}

fn format_paper_msg(signal: &ArbSignal, size: f64) -> String {
    let spread = signal.sell_price - signal.buy_price;
    format!(
        "<b>\u{1F4DD} Paper arb — {} {}</b>\n\
         BUY on {} @ {:.1}¢ | SELL on {} @ {:.1}¢\n\
         Size: ${:.2} | Est. edge: ${:.2} ({:.0} bps)",
        signal.outcome.to_uppercase(),
        signal.market_slug,
        signal.buy_venue.as_str(),
        signal.buy_price * 100.0,
        signal.sell_venue.as_str(),
        signal.sell_price * 100.0,
        size,
        spread * size,
        spread * 10_000.0,
    )
}

fn format_exec_msg(
    signal: &ArbSignal,
    size: f64,
    status: &str,
    buy: &LegResult,
    sell: &LegResult,
) -> String {
    let emoji = match status {
        "executed" => "\u{2705}",
        "partial" => "\u{26A0}\u{FE0F}",
        _ => "\u{274C}",
    };
    let spread = signal.sell_price - signal.buy_price;
    let mut msg = format!(
        "<b>{} Arb {} — {} {}</b>\n\
         BUY on {} @ {:.1}¢ [{}] | SELL on {} @ {:.1}¢ [{}]\n\
         Size: ${:.2} | Spread: {:.0} bps",
        emoji,
        status,
        signal.outcome.to_uppercase(),
        signal.market_slug,
        buy.provider,
        signal.buy_price * 100.0,
        buy.status,
        sell.provider,
        signal.sell_price * 100.0,
        sell.status,
        size,
        spread * 10_000.0,
    );
    if status == "partial" {
        msg.push_str("\n<b>MANUAL INTERVENTION REQUIRED</b>");
    }
    if let Some(e) = &buy.error {
        msg.push_str(&format!("\nBuy error: {}", e));
    }
    if let Some(e) = &sell.error {
        msg.push_str(&format!("\nSell error: {}", e));
    }
    msg
}

fn env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
}
