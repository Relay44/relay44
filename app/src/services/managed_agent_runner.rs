//! Managed agent execution engine.
//!
//! Paper-trade runner for agents deployed from templates in `agent_templates`.
//! Ticks on a fixed interval, evaluates each active managed agent's strategy
//! against the external market universe (Limitless + Polymarket), opens paper
//! positions via `services::external::paper::simulate_fill`, and closes them
//! on hold-expiry, take-profit/stop-loss, or market resolution.
//!
//! Gated by `MANAGED_AGENT_RUNNER_ENABLED`. The runner is independent from
//! the `external_agents` scheduler — managed agents have their own schema,
//! their own position table (`managed_agent_positions`), and their own PnL
//! accounting surface on `managed_agents`.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use log::{debug, info, warn};
use serde_json::Value as Json;
use sqlx::Row;
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use crate::services::external::paper::{
    realized_pnl, resolve_mark_price, simulate_fill, unrealized_pnl, PaperFillComputation,
};
use crate::services::external::types::{ExternalMarketId, ExternalMarketSnapshot};
use crate::services::external::{
    fetch_market_by_id, fetch_markets, fetch_orderbook, ExternalMarketSource,
    ExternalMarketsRequest, TradableFilter,
};
use crate::AppState;

/// Default tick interval for the managed agent runner (60 seconds).
const DEFAULT_TICK_INTERVAL_SECS: u64 = 60;
/// Max number of agents to evaluate per tick.
const DEFAULT_SCAN_LIMIT: i64 = 100;
/// Orderbook depth to request when sizing paper fills.
const ORDERBOOK_DEPTH: u64 = 20;
/// Deactivate agents after this many consecutive failures.
const MAX_CONSECUTIVE_FAILURES: i32 = 20;
/// Cap the number of markets a single agent evaluates per tick.
const MAX_MARKETS_PER_TICK: usize = 25;
/// Cap the size of the candidate universe fetched per strategy.
const MARKET_UNIVERSE_LIMIT: u64 = 60;

/// Spawn the managed agent runner background loop.
pub fn spawn_managed_agent_runner(state: Arc<AppState>) {
    if !state.config.agent_service_enabled {
        info!("Managed agent runner disabled (agent_service_enabled=false)");
        return;
    }
    if !state.config.managed_agent_runner_enabled {
        info!("Managed agent runner disabled (managed_agent_runner_enabled=false)");
        return;
    }

    let interval_secs = state
        .config
        .managed_agent_runner_interval_secs
        .max(15);

    info!(
        "Starting managed agent runner (interval={}s, fee_bps={}, hold_secs={})",
        interval_secs, state.config.paper_fee_bps, state.config.paper_hold_duration_seconds
    );

    tokio::spawn(async move {
        // Stagger against the external agents scheduler.
        tokio::time::sleep(Duration::from_secs(12)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if state.is_shutting_down.load(Ordering::Relaxed) {
                info!("Managed agent runner shutting down");
                break;
            }

            match run_tick(&state).await {
                Ok(stats) => {
                    if stats.has_activity() {
                        info!(
                            "Managed agent runner tick: scanned={} closed={} opened={} errors={}",
                            stats.scanned, stats.closed, stats.opened, stats.errors
                        );
                    }
                }
                Err(e) => {
                    warn!("Managed agent runner tick error: {}", e);
                }
            }
        }
    });
}

#[derive(Default, Debug)]
struct TickStats {
    scanned: u64,
    closed: u64,
    opened: u64,
    errors: u64,
}

impl TickStats {
    fn has_activity(&self) -> bool {
        self.closed > 0 || self.opened > 0 || self.errors > 0
    }
}

/// Single row loaded from `managed_agents` for execution.
#[derive(Debug, Clone)]
struct AgentRecord {
    id: String,
    owner: String,
    template_id: String,
    seed_usdc: f64,
    pnl_usdc: f64,
    high_watermark_usdc: f64,
    max_drawdown_pct: f64,
    total_trades: i64,
    params: Json,
    // From agent_templates
    strategy: String,
    category: String,
    min_seed_usdc: f64,
}

impl AgentRecord {
    fn equity(&self) -> f64 {
        (self.seed_usdc + self.pnl_usdc).max(0.0)
    }

    fn param_f64(&self, key: &str, default: f64) -> f64 {
        self.params
            .get(key)
            .and_then(|v| v.as_f64())
            .unwrap_or(default)
    }

    fn param_u64(&self, key: &str, default: u64) -> u64 {
        self.params
            .get(key)
            .and_then(|v| v.as_u64())
            .unwrap_or(default)
    }
}

async fn run_tick(state: &AppState) -> Result<TickStats, String> {
    let now = Utc::now();
    let mut stats = TickStats::default();

    // First: sweep positions that need closing (hold expired, or agent
    // stopped/paused). This must run even for paused agents so outstanding
    // positions don't stay open forever.
    stats.closed += sweep_due_positions(state, now).await.map_err(|e| e.to_string())?;

    // Then: load active agents whose next_execution_at is due.
    let rows = sqlx::query(
        "SELECT a.id, a.owner, a.template_id, a.seed_usdc, a.pnl_usdc, \
                a.high_watermark_usdc, a.max_drawdown_pct, a.total_trades, \
                a.params::text as params_text, a.consecutive_failures, \
                t.strategy, t.category, t.min_seed_usdc \
         FROM managed_agents a \
         JOIN agent_templates t ON t.id = a.template_id \
         WHERE a.status = 'active' AND a.next_execution_at <= $1 \
         ORDER BY a.next_execution_at ASC, a.id ASC \
         LIMIT $2",
    )
    .bind(now)
    .bind(DEFAULT_SCAN_LIMIT)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    stats.scanned = rows.len() as u64;

    for row in &rows {
        let id: String = row.try_get("id").unwrap_or_default();
        let consecutive_failures: i32 = row.try_get("consecutive_failures").unwrap_or(0);

        if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
            let _ = sqlx::query(
                "UPDATE managed_agents SET status = 'stopped', last_error = $2, updated_at = NOW() WHERE id = $1",
            )
            .bind(id.as_str())
            .bind(format!("auto-stopped after {} failures", consecutive_failures))
            .execute(state.db.pool())
            .await;
            continue;
        }

        let params_text: String = row.try_get("params_text").unwrap_or_else(|_| "{}".into());
        let params: Json = serde_json::from_str(&params_text).unwrap_or(Json::Object(Default::default()));

        let agent = AgentRecord {
            id: id.clone(),
            owner: row.try_get("owner").unwrap_or_default(),
            template_id: row.try_get("template_id").unwrap_or_default(),
            seed_usdc: row.try_get::<f64, _>("seed_usdc").unwrap_or(0.0),
            pnl_usdc: row.try_get::<f64, _>("pnl_usdc").unwrap_or(0.0),
            high_watermark_usdc: row.try_get::<f64, _>("high_watermark_usdc").unwrap_or(0.0),
            max_drawdown_pct: row.try_get::<f64, _>("max_drawdown_pct").unwrap_or(0.0),
            total_trades: row.try_get::<i64, _>("total_trades").unwrap_or(0),
            params,
            strategy: row.try_get("strategy").unwrap_or_default(),
            category: row.try_get("category").unwrap_or_default(),
            min_seed_usdc: row.try_get::<f64, _>("min_seed_usdc").unwrap_or(0.0),
        };

        match execute_agent(state, &agent, now).await {
            Ok(opened) => {
                stats.opened += opened;
                let _ = reschedule_agent(state, &agent.id, now, true, None).await;
            }
            Err(e) => {
                stats.errors += 1;
                warn!("managed agent {} tick error: {}", agent.id, e);
                let _ = reschedule_agent(state, &agent.id, now, false, Some(&e)).await;
            }
        }
    }

    Ok(stats)
}

/// Close any open positions whose hold period has expired, or whose owning
/// agent is no longer active. Returns the number of positions closed.
async fn sweep_due_positions(state: &AppState, now: DateTime<Utc>) -> anyhow::Result<u64> {
    let rows = sqlx::query(
        "SELECT p.id, p.agent_id, p.market_slug, p.provider, p.outcome, p.side, \
                p.entry_price, p.quantity, p.notional_usdc, p.fees_usdc, p.hold_until, \
                a.status as agent_status \
         FROM managed_agent_positions p \
         JOIN managed_agents a ON a.id = p.agent_id \
         WHERE p.hold_until <= $1 OR a.status <> 'active' \
         LIMIT 200",
    )
    .bind(now)
    .fetch_all(state.db.pool())
    .await?;

    let fee_bps = state.config.paper_fee_bps;
    let mut closed = 0_u64;

    for row in rows {
        let position_id: i64 = row.try_get("id").unwrap_or_default();
        let agent_id: String = row.try_get("agent_id").unwrap_or_default();
        let market_slug: String = row.try_get("market_slug").unwrap_or_default();
        let provider: String = row.try_get("provider").unwrap_or_default();
        let outcome: String = row.try_get("outcome").unwrap_or_default();
        let side: String = row.try_get("side").unwrap_or_default();
        let entry_price: f64 = row.try_get("entry_price").unwrap_or(0.0);
        let quantity: f64 = row.try_get("quantity").unwrap_or(0.0);
        let entry_fees: f64 = row.try_get("fees_usdc").unwrap_or(0.0);

        // Exit side is the opposite of the entry side.
        let exit_side = if side == "buy" { "sell" } else { "buy" };

        let market_id_str = format!("{}:{}", provider, market_slug);
        let market_id = match ExternalMarketId::parse(&market_id_str) {
            Ok(m) => m,
            Err(e) => {
                warn!("sweep: failed to parse market id {}: {:?}", market_id_str, e);
                continue;
            }
        };

        let market = match fetch_market_by_id(&state.config, &market_id).await {
            Ok(m) => m,
            Err(e) => {
                debug!("sweep: fetch_market {} failed: {:?}", market_id_str, e);
                continue;
            }
        };

        let orderbook =
            match fetch_orderbook(&state.config, &state.redis, &market_id, &outcome, ORDERBOOK_DEPTH)
                .await
            {
                Ok(ob) => ob,
                Err(e) => {
                    debug!("sweep: fetch_orderbook {} failed: {:?}", market_id_str, e);
                    continue;
                }
            };

        // If the market resolved, mark the position out at final resolution price.
        let fill = if market.resolved {
            // Resolved outcomes: winner = 1.0, loser = 0.0 for buyer; inverse for seller.
            let winning_outcome = market.outcome.clone().unwrap_or_default();
            let is_winner = winning_outcome.eq_ignore_ascii_case(&outcome);
            let final_price = if is_winner { 1.0 } else { 0.0 };
            PaperFillComputation {
                requested_quantity: quantity,
                filled_quantity: quantity,
                average_price: final_price,
                mark_price: final_price,
                notional_usdc: final_price * quantity,
                fee_usdc: 0.0, // no fee on resolution
                slippage_bps: 0,
                partial_fill: false,
                used_orderbook_depth: false,
            }
        } else {
            simulate_fill(&market, &orderbook, &outcome, exit_side, quantity, fee_bps, None)
        };

        let realized = realized_pnl(
            &side,
            entry_price,
            fill.average_price,
            fill.filled_quantity,
            entry_fees + fill.fee_usdc,
        );

        let mut tx = state.db.pool().begin().await?;

        sqlx::query("DELETE FROM managed_agent_positions WHERE id = $1")
            .bind(position_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            "INSERT INTO managed_agent_trades \
             (agent_id, market_slug, outcome, side, price, quantity, pnl_usdc, provider) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(agent_id.as_str())
        .bind(market_slug.as_str())
        .bind(outcome.as_str())
        .bind(exit_side)
        .bind(fill.average_price)
        .bind(fill.filled_quantity)
        .bind(realized)
        .bind(provider.as_str())
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "UPDATE managed_agents \
             SET pnl_usdc = pnl_usdc + $1, \
                 total_trades = total_trades + 1, \
                 high_watermark_usdc = GREATEST(high_watermark_usdc, seed_usdc + pnl_usdc + $1), \
                 max_drawdown_pct = GREATEST( \
                    max_drawdown_pct, \
                    CASE WHEN high_watermark_usdc > 0 \
                         THEN 100.0 * (high_watermark_usdc - (seed_usdc + pnl_usdc + $1)) / high_watermark_usdc \
                         ELSE 0 END \
                 ), \
                 last_executed_at = NOW(), \
                 updated_at = NOW() \
             WHERE id = $2",
        )
        .bind(realized)
        .bind(agent_id.as_str())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        closed += 1;
    }

    Ok(closed)
}

/// Execute one active agent: count open positions, evaluate strategy for
/// any remaining capacity, and open new paper positions.
/// Returns the number of new positions opened.
async fn execute_agent(
    state: &AppState,
    agent: &AgentRecord,
    now: DateTime<Utc>,
) -> Result<u64, String> {
    // Drawdown guard: halt new entries if drawdown exceeds 50%.
    if agent.max_drawdown_pct >= 50.0 {
        debug!("agent {} halted: drawdown {:.1}%", agent.id, agent.max_drawdown_pct);
        return Ok(0);
    }

    // Solvency guard: need at least min_seed_usdc in equity to trade.
    let equity = agent.equity();
    if equity < agent.min_seed_usdc.max(10.0) {
        debug!("agent {} insolvent: equity={:.2}", agent.id, equity);
        return Ok(0);
    }

    let max_markets = agent.param_u64("maxMarkets", 5).min(20) as i64;
    let open_positions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM managed_agent_positions WHERE agent_id = $1",
    )
    .bind(&agent.id)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    let capacity = (max_markets - open_positions).max(0);
    if capacity == 0 {
        return Ok(0);
    }

    let max_position_pct = agent.param_f64("maxPositionPct", 10.0).clamp(0.5, 50.0);
    let position_budget_usdc = (equity * max_position_pct / 100.0).max(1.0);

    // Fetch candidate market universe.
    let markets = fetch_candidate_markets(state).await?;
    if markets.is_empty() {
        debug!("agent {}: empty market universe", agent.id);
        return Ok(0);
    }

    // Exclude markets the agent already has open positions in.
    let open_rows = sqlx::query(
        "SELECT market_slug, outcome, side FROM managed_agent_positions WHERE agent_id = $1",
    )
    .bind(&agent.id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;
    let blocked: std::collections::HashSet<(String, String, String)> = open_rows
        .iter()
        .map(|r| {
            (
                r.try_get::<String, _>("market_slug").unwrap_or_default(),
                r.try_get::<String, _>("outcome").unwrap_or_default(),
                r.try_get::<String, _>("side").unwrap_or_default(),
            )
        })
        .collect();

    // Evaluate strategy against each market.
    let signals = evaluate_strategy(&agent.strategy, agent, &markets);

    let mut opened = 0_u64;
    for signal in signals.into_iter().take(capacity as usize) {
        // The blocked set is keyed on the provider-scoped value (slug/id),
        // matching what we store in managed_agent_positions.market_slug.
        let provider_value = match ExternalMarketId::parse(&signal.market.id) {
            Ok(m) => m.value,
            Err(_) => continue,
        };
        let key = (
            provider_value,
            signal.outcome.clone(),
            signal.side.clone(),
        );
        if blocked.contains(&key) {
            continue;
        }

        match open_position(state, agent, &signal, position_budget_usdc, now).await {
            Ok(true) => opened += 1,
            Ok(false) => {}
            Err(e) => warn!("agent {}: open_position failed: {}", agent.id, e),
        }
    }

    Ok(opened)
}

async fn fetch_candidate_markets(state: &AppState) -> Result<Vec<ExternalMarketSnapshot>, String> {
    let mut markets = fetch_markets(
        &state.config,
        &state.redis,
        ExternalMarketSource::All,
        TradableFilter::Agent,
        MARKET_UNIVERSE_LIMIT,
        0,
        ExternalMarketsRequest::default(),
    )
    .await
    .map_err(|e| format!("{:?}", e))?;

    // Exclude resolved / low-liquidity / closing-soon markets.
    let now_ts = Utc::now().timestamp() as u64;
    markets.retain(|m| {
        !m.resolved
            && m.status.eq_ignore_ascii_case("active")
            && m.execution_agents
            && m.volume >= 1_000.0
            && m.close_time > now_ts + 3_600
    });

    markets.truncate(MAX_MARKETS_PER_TICK);
    Ok(markets)
}

#[derive(Debug, Clone)]
struct Signal {
    market: ExternalMarketSnapshot,
    outcome: String,
    side: String,
    reason: String,
}

fn evaluate_strategy(
    strategy: &str,
    agent: &AgentRecord,
    markets: &[ExternalMarketSnapshot],
) -> Vec<Signal> {
    match strategy {
        "momentum" => momentum_signals(agent, markets),
        "mean_reversion" => mean_reversion_signals(agent, markets),
        "tail_hunter" => tail_hunter_signals(agent, markets),
        // Not yet implemented — these strategies parse, deploy, and tick
        // cleanly but produce no entries. They will still close positions
        // and track PnL if seeded manually.
        "arbitrage" | "market_making" | "news_momentum" => Vec::new(),
        _ => Vec::new(),
    }
}

/// Momentum: buy outcomes trading strongly off 0.5 (the market has "picked a
/// side"), reading the snapshot's yes/no prices. We use the extreme side as
/// the conviction signal since we don't have a true lookback price stream.
fn momentum_signals(agent: &AgentRecord, markets: &[ExternalMarketSnapshot]) -> Vec<Signal> {
    let entry_threshold_pct = agent.param_f64("entryThresholdPct", 3.0);
    // Treat as "edge above 50%" — yes_price beyond 50% + threshold.
    let edge = entry_threshold_pct / 100.0;

    let mut signals: Vec<Signal> = Vec::new();
    for market in markets {
        let yes = market.yes_price.clamp(0.0, 1.0);
        let no = market.no_price.clamp(0.0, 1.0);
        // Skip markets priced at the extremes — no upside left.
        if yes >= 0.92 || yes <= 0.08 {
            continue;
        }
        if (yes - 0.5).abs() < edge {
            continue;
        }
        let (outcome, _price) = if yes > no {
            ("yes".to_string(), yes)
        } else {
            ("no".to_string(), no)
        };
        signals.push(Signal {
            market: market.clone(),
            outcome,
            side: "buy".to_string(),
            reason: format!("momentum edge {:.3}", (yes - 0.5).abs()),
        });
    }

    // Sort by conviction (distance from 0.5) desc.
    signals.sort_by(|a, b| {
        let da = (a.market.yes_price - 0.5).abs();
        let db = (b.market.yes_price - 0.5).abs();
        db.total_cmp(&da)
    });

    signals
}

/// Mean reversion: fade markets that have moved far from 0.5, betting on
/// retracement toward the mean.
fn mean_reversion_signals(
    agent: &AgentRecord,
    markets: &[ExternalMarketSnapshot],
) -> Vec<Signal> {
    // Use zscoreEntry / 10 as a rough "distance from 0.5" threshold
    // (standard default 2.0 → 0.2 distance → price beyond 0.7 or under 0.3).
    let z = agent.param_f64("zscoreEntry", 2.0);
    let distance = (z / 10.0).clamp(0.10, 0.40);

    let mut signals: Vec<Signal> = Vec::new();
    for market in markets {
        let yes = market.yes_price.clamp(0.0, 1.0);
        // Skip extreme markets — mean reversion too slow / resolution risk.
        if yes >= 0.95 || yes <= 0.05 {
            continue;
        }
        if (yes - 0.5).abs() < distance {
            continue;
        }
        // Fade the move: if yes is rich, buy no; if no is rich, buy yes.
        let outcome = if yes > 0.5 { "no".to_string() } else { "yes".to_string() };
        signals.push(Signal {
            market: market.clone(),
            outcome,
            side: "buy".to_string(),
            reason: format!("mean-reversion fade yes={:.3}", yes),
        });
    }

    // Sort by distance from 0.5 desc (strongest fades first).
    signals.sort_by(|a, b| {
        let da = (a.market.yes_price - 0.5).abs();
        let db = (b.market.yes_price - 0.5).abs();
        db.total_cmp(&da)
    });

    signals
}

/// Tail hunter: buy long-odds outcomes (below maxEntryPrice) when the
/// implied probability is cheap enough to justify the tail exposure.
fn tail_hunter_signals(
    agent: &AgentRecord,
    markets: &[ExternalMarketSnapshot],
) -> Vec<Signal> {
    let max_entry = agent.param_f64("maxEntryPrice", 0.10).clamp(0.01, 0.30);

    let mut signals: Vec<Signal> = Vec::new();
    for market in markets {
        let yes = market.yes_price.clamp(0.0, 1.0);
        let no = market.no_price.clamp(0.0, 1.0);
        // Prefer the cheap leg.
        let (outcome, price) = if yes <= no {
            ("yes".to_string(), yes)
        } else {
            ("no".to_string(), no)
        };
        if price == 0.0 || price > max_entry {
            continue;
        }
        signals.push(Signal {
            market: market.clone(),
            outcome,
            side: "buy".to_string(),
            reason: format!("tail entry {:.3}", price),
        });
    }

    // Cheapest first.
    signals.sort_by(|a, b| {
        let pa = a.market.yes_price.min(a.market.no_price);
        let pb = b.market.yes_price.min(b.market.no_price);
        pa.total_cmp(&pb)
    });

    signals
}

async fn open_position(
    state: &AppState,
    agent: &AgentRecord,
    signal: &Signal,
    position_budget_usdc: f64,
    now: DateTime<Utc>,
) -> Result<bool, String> {
    // Use the namespaced snapshot.id (e.g. "limitless:slug"), not
    // provider_market_ref (raw provider-internal id).
    let market_id_str = signal.market.id.clone();
    let market_id = ExternalMarketId::parse(&market_id_str).map_err(|e| format!("{:?}", e))?;

    // Pull a fresh orderbook — snapshot prices from list endpoint can be stale.
    let orderbook = fetch_orderbook(
        &state.config,
        &state.redis,
        &market_id,
        &signal.outcome,
        ORDERBOOK_DEPTH,
    )
    .await
    .map_err(|e| format!("{:?}", e))?;

    // Size the position by budget, not by share count. At the current best
    // price, quantity = budget / price. Cap at 10k shares to avoid absurd
    // sizes in near-zero-price markets.
    let probe_price =
        resolve_mark_price(&signal.market, &orderbook, &signal.outcome).max(0.01);
    let raw_quantity = (position_budget_usdc / probe_price).min(10_000.0).max(1.0);

    let fill = simulate_fill(
        &signal.market,
        &orderbook,
        &signal.outcome,
        &signal.side,
        raw_quantity,
        state.config.paper_fee_bps,
        None,
    );

    if fill.filled_quantity < 0.5 {
        debug!(
            "agent {}: skipped {} — insufficient depth (filled {:.2})",
            agent.id, market_id_str, fill.filled_quantity
        );
        return Ok(false);
    }

    // Don't open if slippage is awful (> 5%).
    if fill.slippage_bps > 500 {
        debug!(
            "agent {}: skipped {} — slippage {} bps",
            agent.id, market_id_str, fill.slippage_bps
        );
        return Ok(false);
    }

    let hold_secs = state.config.paper_hold_duration_seconds.max(60);
    let hold_until = now + ChronoDuration::seconds(hold_secs as i64);

    let mark_price = fill.mark_price;
    let u_pnl = unrealized_pnl(&signal.side, fill.average_price, mark_price, fill.filled_quantity);

    let mut tx = state.db.pool().begin().await.map_err(|e| e.to_string())?;

    // Insert position — ON CONFLICT DO NOTHING so a concurrent tick can't
    // double-enter on the unique index.
    let inserted = sqlx::query(
        "INSERT INTO managed_agent_positions \
         (agent_id, market_slug, provider, outcome, side, entry_price, quantity, \
          notional_usdc, fees_usdc, mark_price, unrealized_pnl_usdc, hold_until, metadata) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13::jsonb) \
         ON CONFLICT (agent_id, market_slug, outcome, side) DO NOTHING",
    )
    .bind(&agent.id)
    .bind(&market_id.value)
    .bind(market_id.provider.as_str())
    .bind(&signal.outcome)
    .bind(&signal.side)
    .bind(fill.average_price)
    .bind(fill.filled_quantity)
    .bind(fill.notional_usdc)
    .bind(fill.fee_usdc)
    .bind(mark_price)
    .bind(u_pnl)
    .bind(hold_until)
    .bind(format!(
        "{{\"reason\":{},\"slippageBps\":{}}}",
        serde_json::Value::String(signal.reason.clone()),
        fill.slippage_bps
    ))
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    if inserted.rows_affected() == 0 {
        tx.rollback().await.ok();
        return Ok(false);
    }

    // Record the entry as an immediate trade row (PnL null — only filled on close).
    sqlx::query(
        "INSERT INTO managed_agent_trades \
         (agent_id, market_slug, outcome, side, price, quantity, pnl_usdc, provider) \
         VALUES ($1, $2, $3, $4, $5, $6, NULL, $7)",
    )
    .bind(&agent.id)
    .bind(&market_id.value)
    .bind(&signal.outcome)
    .bind(&signal.side)
    .bind(fill.average_price)
    .bind(fill.filled_quantity)
    .bind(market_id.provider.as_str())
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    // Charge entry fees immediately against PnL so equity tracks reality.
    sqlx::query(
        "UPDATE managed_agents \
         SET pnl_usdc = pnl_usdc - $1, \
             last_executed_at = NOW(), \
             updated_at = NOW() \
         WHERE id = $2",
    )
    .bind(fill.fee_usdc)
    .bind(&agent.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(true)
}

async fn reschedule_agent(
    state: &AppState,
    agent_id: &str,
    now: DateTime<Utc>,
    success: bool,
    err: Option<&str>,
) -> anyhow::Result<()> {
    // Normal cadence: tick again in `interval * 2` (each agent runs every
    // ~2 tick cycles) on success. On failure, exponential-ish backoff.
    let interval = state.config.managed_agent_runner_interval_secs.max(15);
    let next_secs: i64 = if success {
        (interval as i64).saturating_mul(2).clamp(30, 600)
    } else {
        // Backoff: 2 min, then handled by consecutive_failures counter.
        120
    };

    let next = now + ChronoDuration::seconds(next_secs);

    if success {
        sqlx::query(
            "UPDATE managed_agents \
             SET next_execution_at = $1, consecutive_failures = 0, last_error = NULL, \
                 updated_at = NOW() \
             WHERE id = $2",
        )
        .bind(next)
        .bind(agent_id)
        .execute(state.db.pool())
        .await?;
    } else {
        sqlx::query(
            "UPDATE managed_agents \
             SET next_execution_at = $1, consecutive_failures = consecutive_failures + 1, \
                 last_error = $2, updated_at = NOW() \
             WHERE id = $3",
        )
        .bind(next)
        .bind(err.unwrap_or("unknown"))
        .bind(agent_id)
        .execute(state.db.pool())
        .await?;
    }

    Ok(())
}
