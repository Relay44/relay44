use log::{info, warn};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::AppState;

fn is_base_rpc_rate_limited(err: &anyhow::Error) -> bool {
    err.to_string().contains("429 Too Many Requests")
}

fn backoff_seconds(rate_limit_streak: u32) -> u64 {
    match rate_limit_streak {
        0 => 20,
        1 => 60,
        2 => 120,
        3 => 240,
        _ => 300,
    }
}

fn spawn_evm_indexer(app_state: &Arc<AppState>) {
    let config = &app_state.config;

    if !config.evm_enabled || !config.evm_reads_enabled {
        info!("EVM indexer disabled by config toggles");
        return;
    }

    let market_core = config.market_core_address.clone();
    let order_book = config.order_book_address.clone();

    if market_core.is_empty() || order_book.is_empty() {
        warn!("Skipping EVM indexer start: MARKET_CORE_ADDRESS or ORDER_BOOK_ADDRESS is missing");
        return;
    }

    let indexer = app_state.evm_indexer.clone();
    let db = app_state.db.clone();
    let rpc = app_state.evm_rpc.clone();
    let lookback_blocks = config.indexer_lookback_blocks;
    let confirmations = config.indexer_confirmations;

    tokio::spawn(async move {
        match db.get_chain_sync_cursor("evm_indexer_main").await {
            Ok(Some(cursor)) => {
                indexer.set_last_synced_block(cursor.last_block).await;
                info!(
                    "Restored EVM indexer cursor from DB at block {}",
                    cursor.last_block
                );
            }
            Ok(None) => {}
            Err(err) => warn!("Failed to restore EVM indexer cursor: {}", err),
        }

        info!("Starting EVM log indexer background loop");
        const TOPICS: [&str; 6] = [
            "0x550857481380e1875f94e5eac6470eff69ecd368405067d9d5dfdf645d3d1f8e",
            "0xbc7c1013df472d2b00db2b9da4c476dbf8f0bc22116913d78750cf21d2c80fc2",
            "0xac1c16fb14f9a45ec49f65d268ff0d0f1945c504b82df54a9c6ad9f01b059be5",
            "0x9384174c8517f5537b08e79211fc039e8a098571a3a2b4cb21dfa6f3237e8de1",
            "0x5aac01386940f75e601757cfe5dc1d4ab2bac84f98d30664486114a8abb38a45",
            "0x93c1c30a0fa404e7a08a9f6a9d68323786a7e120f3adc0c16eb8855922e35dfa",
        ];
        let mut rate_limit_streak = 0_u32;

        loop {
            let latest_block = match rpc.eth_block_number().await {
                Ok(block) => block,
                Err(err) => {
                    let delay_seconds = if is_base_rpc_rate_limited(&err) {
                        rate_limit_streak = rate_limit_streak.saturating_add(1);
                        let delay = backoff_seconds(rate_limit_streak);
                        warn!(
                            "Base RPC rate limited for indexer latest block fetch; backing off {}s (streak={})",
                            delay, rate_limit_streak
                        );
                        delay
                    } else {
                        rate_limit_streak = 0;
                        warn!("Failed to fetch latest Base block for indexer: {}", err);
                        backoff_seconds(0)
                    };
                    tokio::time::sleep(tokio::time::Duration::from_secs(delay_seconds)).await;
                    continue;
                }
            };
            let target_block = latest_block.saturating_sub(confirmations);

            match indexer
                .sync(
                    &market_core,
                    &order_book,
                    lookback_blocks,
                    &TOPICS,
                    Some(target_block),
                )
                .await
            {
                Ok(log_count) => {
                    rate_limit_streak = 0;
                    let last_synced = indexer.last_synced_block().await;
                    let meta = json!({
                        "latestBlock": latest_block,
                        "targetBlock": target_block,
                        "logsIndexed": log_count,
                        "updatedAt": chrono::Utc::now().to_rfc3339(),
                    });
                    if let Err(err) = db
                        .upsert_chain_sync_cursor("evm_indexer_main", last_synced, meta)
                        .await
                    {
                        warn!("Failed to persist EVM indexer cursor: {}", err);
                    }
                }
                Err(err) => {
                    if is_base_rpc_rate_limited(&err) {
                        rate_limit_streak = rate_limit_streak.saturating_add(1);
                        let delay = backoff_seconds(rate_limit_streak);
                        warn!(
                            "Base RPC rate limited during indexer sync; backing off {}s (streak={})",
                            delay, rate_limit_streak
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                        continue;
                    }
                    rate_limit_streak = 0;
                    warn!("EVM indexer sync failed: {}", err);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(backoff_seconds(0))).await;
        }
    });
}

fn spawn_event_bridge(app_state: &Arc<AppState>) {
    let ws_state = app_state.clone();
    let mut event_rx = app_state.event_bus.subscribe();
    tokio::spawn(async move {
        use crate::services::websocket::{MarketUpdate, TradeUpdate};
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    let now = chrono::Utc::now().timestamp();
                    match &event {
                        crate::services::event_bus::PlatformEvent::AgentExecuted(e) => {
                            ws_state
                                .ws_hub
                                .broadcast_trade(TradeUpdate {
                                    market_id: e.market_id.clone(),
                                    outcome: e.outcome.clone(),
                                    price: e.price,
                                    quantity: 0,
                                    buyer: e.owner.clone(),
                                    seller: String::new(),
                                    timestamp: now,
                                })
                                .await;
                        }
                        crate::services::event_bus::PlatformEvent::PositionOpened(e)
                        | crate::services::event_bus::PlatformEvent::PositionClosed(e) => {
                            ws_state
                                .ws_hub
                                .broadcast_market(MarketUpdate {
                                    market_id: e.market_id.clone(),
                                    yes_price: e.entry_price,
                                    no_price: 1.0 - e.entry_price,
                                    status: "active".to_string(),
                                    timestamp: now,
                                })
                                .await;
                        }
                        _ => {}
                    }

                    if let Ok(json_str) = serde_json::to_string(&event) {
                        let _ = ws_state.ws_hub.broadcast_raw_global(json_str).await;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("EventBus→WS bridge lagged by {} events", n);
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

pub fn spawn_background_services(app_state: Arc<AppState>) {
    spawn_evm_indexer(&app_state);
    spawn_event_bridge(&app_state);

    super::agent_scheduler::spawn_agent_scheduler(app_state.clone());
    super::distribution_scheduler::spawn_distribution_scheduler(app_state.clone());
    super::liquidity_mirror::spawn_liquidity_mirror(app_state.clone());
    super::hedge_engine::spawn_hedge_engine(app_state.clone());
    super::smart_router::spawn_arb_scanner(app_state.clone());
    super::portfolio_snapshot::spawn_portfolio_snapshotter(app_state.clone());
    super::polymarket_scanner::spawn_scanner(app_state.clone());
    super::limitless_scanner::spawn_limitless_scanner(app_state.clone());
    super::aerodrome_scanner::spawn_aerodrome_scanner(app_state.clone());
    super::market_creator::spawn_market_creator(app_state);
}
