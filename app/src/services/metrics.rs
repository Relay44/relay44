//! Metrics and observability
//!
//! Application metrics, health checks, latency histograms.

use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::Instant;

/// Histogram bucket boundaries in milliseconds
const LATENCY_BUCKETS: &[f64] = &[
    1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0,
];

/// Thread-safe histogram for latency tracking
pub struct Histogram {
    buckets: RwLock<Vec<u64>>,
    sum: AtomicU64,
    count: AtomicU64,
}

impl Histogram {
    pub fn new() -> Self {
        Self {
            buckets: RwLock::new(vec![0; LATENCY_BUCKETS.len() + 1]),
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    /// Observe a value in the histogram
    pub fn observe(&self, value_ms: f64) {
        let value_bits = (value_ms * 1000.0) as u64; // Store as microseconds for precision
        self.sum.fetch_add(value_bits, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);

        let bucket_idx = LATENCY_BUCKETS
            .iter()
            .position(|&b| value_ms <= b)
            .unwrap_or(LATENCY_BUCKETS.len());

        if let Ok(mut buckets) = self.buckets.write() {
            for i in bucket_idx..buckets.len() {
                buckets[i] += 1;
            }
        }
    }

    /// Get histogram stats
    pub fn get_stats(&self) -> HistogramStats {
        let buckets = self
            .buckets
            .read()
            .map(|b| b.clone())
            .unwrap_or_else(|_| vec![0; LATENCY_BUCKETS.len() + 1]);
        let sum_micros = self.sum.load(Ordering::Relaxed);
        let count = self.count.load(Ordering::Relaxed);

        HistogramStats {
            buckets: LATENCY_BUCKETS
                .iter()
                .zip(buckets.iter())
                .map(|(&le, &count)| (le, count))
                .collect(),
            inf_bucket: *buckets.last().unwrap_or(&0),
            sum_ms: sum_micros as f64 / 1000.0,
            count,
        }
    }

    /// Export in Prometheus format
    pub fn export_prometheus(&self, name: &str, help: &str) -> String {
        let stats = self.get_stats();
        let mut output = String::new();

        output.push_str(&format!("# HELP {} {}\n", name, help));
        output.push_str(&format!("# TYPE {} histogram\n", name));

        for (le, count) in &stats.buckets {
            output.push_str(&format!("{}_bucket{{le=\"{}\"}} {}\n", name, le, count));
        }
        output.push_str(&format!(
            "{}_bucket{{le=\"+Inf\"}} {}\n",
            name, stats.inf_bucket
        ));
        output.push_str(&format!("{}_sum {}\n", name, stats.sum_ms));
        output.push_str(&format!("{}_count {}\n\n", name, stats.count));

        output
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

/// Histogram statistics
#[derive(Debug, Clone, Serialize)]
pub struct HistogramStats {
    pub buckets: Vec<(f64, u64)>,
    pub inf_bucket: u64,
    pub sum_ms: f64,
    pub count: u64,
}

impl HistogramStats {
    /// Calculate average latency
    pub fn avg_ms(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum_ms / self.count as f64
        }
    }

    /// Estimate percentile (approximate)
    pub fn percentile(&self, p: f64) -> f64 {
        if self.count == 0 {
            return 0.0;
        }

        let target = (self.count as f64 * p / 100.0).ceil() as u64;
        for (le, count) in &self.buckets {
            if *count >= target {
                return *le;
            }
        }
        *LATENCY_BUCKETS.last().unwrap_or(&0.0)
    }
}

/// Application metrics collector
pub struct MetricsService {
    start_time: Instant,
    requests_total: AtomicU64,
    requests_success: AtomicU64,
    requests_error: AtomicU64,
    orders_placed: AtomicU64,
    orders_cancelled: AtomicU64,
    trades_executed: AtomicU64,
    total_volume: AtomicU64,
    // Latency histograms
    request_latency: Histogram,
    order_latency: Histogram,
    trade_latency: Histogram,
    database_latency: Histogram,
}

impl MetricsService {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            requests_total: AtomicU64::new(0),
            requests_success: AtomicU64::new(0),
            requests_error: AtomicU64::new(0),
            orders_placed: AtomicU64::new(0),
            orders_cancelled: AtomicU64::new(0),
            trades_executed: AtomicU64::new(0),
            total_volume: AtomicU64::new(0),
            request_latency: Histogram::new(),
            order_latency: Histogram::new(),
            trade_latency: Histogram::new(),
            database_latency: Histogram::new(),
        }
    }

    /// Record an incoming request
    pub fn record_request(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a successful request
    pub fn record_success(&self) {
        self.requests_success.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a failed request
    pub fn record_error(&self) {
        self.requests_error.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an order placement
    pub fn record_order_placed(&self) {
        self.orders_placed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an order cancellation
    pub fn record_order_cancelled(&self) {
        self.orders_cancelled.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a trade execution with volume
    pub fn record_trade(&self, volume: u64) {
        self.trades_executed.fetch_add(1, Ordering::Relaxed);
        self.total_volume.fetch_add(volume, Ordering::Relaxed);
    }

    /// Record request latency
    pub fn observe_request_latency(&self, latency_ms: f64) {
        self.request_latency.observe(latency_ms);
    }

    /// Record order processing latency
    pub fn observe_order_latency(&self, latency_ms: f64) {
        self.order_latency.observe(latency_ms);
    }

    /// Record trade execution latency
    pub fn observe_trade_latency(&self, latency_ms: f64) {
        self.trade_latency.observe(latency_ms);
    }

    /// Record database query latency
    pub fn observe_database_latency(&self, latency_ms: f64) {
        self.database_latency.observe(latency_ms);
    }

    /// Get latency statistics
    pub fn get_latency_stats(&self) -> LatencyMetrics {
        LatencyMetrics {
            request: self.request_latency.get_stats(),
            order: self.order_latency.get_stats(),
            trade: self.trade_latency.get_stats(),
            database: self.database_latency.get_stats(),
        }
    }

    /// Get current metrics snapshot
    pub fn get_metrics(&self) -> AppMetrics {
        AppMetrics {
            uptime_seconds: self.start_time.elapsed().as_secs(),
            requests: RequestMetrics {
                total: self.requests_total.load(Ordering::Relaxed),
                success: self.requests_success.load(Ordering::Relaxed),
                error: self.requests_error.load(Ordering::Relaxed),
            },
            orders: OrderMetrics {
                placed: self.orders_placed.load(Ordering::Relaxed),
                cancelled: self.orders_cancelled.load(Ordering::Relaxed),
            },
            trades: TradeMetrics {
                executed: self.trades_executed.load(Ordering::Relaxed),
                total_volume: self.total_volume.load(Ordering::Relaxed),
            },
        }
    }

    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> String {
        let metrics = self.get_metrics();
        let mut output = String::new();

        // Uptime
        output.push_str("# HELP relay44_uptime_seconds Seconds since service start\n");
        output.push_str("# TYPE relay44_uptime_seconds gauge\n");
        output.push_str(&format!(
            "relay44_uptime_seconds {}\n\n",
            metrics.uptime_seconds
        ));

        // Requests
        output.push_str("# HELP relay44_requests_total Total HTTP requests\n");
        output.push_str("# TYPE relay44_requests_total counter\n");
        output.push_str(&format!(
            "relay44_requests_total{{status=\"success\"}} {}\n",
            metrics.requests.success
        ));
        output.push_str(&format!(
            "relay44_requests_total{{status=\"error\"}} {}\n\n",
            metrics.requests.error
        ));

        // Orders
        output.push_str("# HELP relay44_orders_total Total orders processed\n");
        output.push_str("# TYPE relay44_orders_total counter\n");
        output.push_str(&format!(
            "relay44_orders_total{{action=\"placed\"}} {}\n",
            metrics.orders.placed
        ));
        output.push_str(&format!(
            "relay44_orders_total{{action=\"cancelled\"}} {}\n\n",
            metrics.orders.cancelled
        ));

        // Trades
        output.push_str("# HELP relay44_trades_total Total trades executed\n");
        output.push_str("# TYPE relay44_trades_total counter\n");
        output.push_str(&format!(
            "relay44_trades_total {}\n\n",
            metrics.trades.executed
        ));

        // Volume
        output.push_str("# HELP relay44_volume_total Total trading volume in lamports\n");
        output.push_str("# TYPE relay44_volume_total counter\n");
        output.push_str(&format!(
            "relay44_volume_total {}\n\n",
            metrics.trades.total_volume
        ));

        // Latency histograms
        output.push_str(&self.request_latency.export_prometheus(
            "relay44_request_duration_ms",
            "HTTP request duration in milliseconds",
        ));
        output.push_str(&self.order_latency.export_prometheus(
            "relay44_order_duration_ms",
            "Order processing duration in milliseconds",
        ));
        output.push_str(&self.trade_latency.export_prometheus(
            "relay44_trade_duration_ms",
            "Trade execution duration in milliseconds",
        ));
        output.push_str(&self.database_latency.export_prometheus(
            "relay44_database_duration_ms",
            "Database query duration in milliseconds",
        ));

        output
    }
}

impl Default for MetricsService {
    fn default() -> Self {
        Self::new()
    }
}

/// Application metrics snapshot
#[derive(Debug, Clone, Serialize)]
pub struct AppMetrics {
    pub uptime_seconds: u64,
    pub requests: RequestMetrics,
    pub orders: OrderMetrics,
    pub trades: TradeMetrics,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestMetrics {
    pub total: u64,
    pub success: u64,
    pub error: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrderMetrics {
    pub placed: u64,
    pub cancelled: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradeMetrics {
    pub executed: u64,
    pub total_volume: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LatencyMetrics {
    pub request: HistogramStats,
    pub order: HistogramStats,
    pub trade: HistogramStats,
    pub database: HistogramStats,
}

/// System health information
#[derive(Debug, Clone, Serialize)]
pub struct SystemHealth {
    pub status: HealthStatus,
    pub version: &'static str,
    pub uptime_seconds: u64,
    pub checks: HealthChecks,
}

/// Health check results for all components
#[derive(Debug, Clone, Serialize)]
pub struct HealthChecks {
    pub database: ComponentHealth,
    pub redis: ComponentHealth,
    pub base: ComponentHealth,
    pub solana: ComponentHealth,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentHealth {
    pub status: HealthStatus,
    pub latency_ms: Option<u64>,
    pub message: Option<String>,
}

