use anyhow::Result;
use clap::Subcommand;
use tabled::Tabled;

use crate::client::Client;
use crate::output::{self, Format};

#[derive(Subcommand, Clone)]
pub enum EdgeScannerCmd {
    /// List active edge scanner signals
    #[command(
        long_about = "Show signals from the calibration and time-decay strategies.\n\n\
                       Signals include Kelly-optimal sizing and edge estimates.\n\n\
                       Examples:\n  \
                       r44 edge-scanner signals\n  \
                       r44 edge-scanner signals --strategy calibration_arb\n  \
                       r44 edge-scanner signals --min-edge 800\n  \
                       r44 --output json edge-scanner signals"
    )]
    Signals {
        /// Filter by strategy: calibration_arb or time_decay
        #[arg(long, short)]
        strategy: Option<String>,
        /// Minimum edge in basis points
        #[arg(long)]
        min_edge: Option<i32>,
        /// Include expired/inactive signals
        #[arg(long)]
        all: bool,
        /// Max results
        #[arg(long, default_value = "50")]
        limit: i64,
    },

    /// Show the current calibration curve
    #[command(
        long_about = "Display the favourite-longshot bias calibration curve.\n\n\
                       Shows how often markets in each price bucket actually resolve YES\n\
                       versus the price-implied probability. Buckets with large edge values\n\
                       indicate systematic crowd mispricing.\n\n\
                       Examples:\n  \
                       r44 edge-scanner curve\n  \
                       r44 --output json edge-scanner curve"
    )]
    Curve,
}

#[derive(Tabled)]
struct SignalRow {
    #[tabled(rename = "Strategy")]
    strategy: String,
    #[tabled(rename = "Direction")]
    direction: String,
    #[tabled(rename = "Edge")]
    edge: String,
    #[tabled(rename = "Price")]
    price: String,
    #[tabled(rename = "Fair")]
    fair: String,
    #[tabled(rename = "Kelly")]
    kelly: String,
    #[tabled(rename = "Days")]
    days: String,
    #[tabled(rename = "Market")]
    market: String,
}

#[derive(Tabled)]
struct CurveRow {
    #[tabled(rename = "Bucket")]
    bucket: String,
    #[tabled(rename = "Samples")]
    samples: String,
    #[tabled(rename = "Actual")]
    actual: String,
    #[tabled(rename = "Expected")]
    expected: String,
    #[tabled(rename = "Edge")]
    edge: String,
}

pub async fn run(cmd: EdgeScannerCmd, api: &Client, fmt: Format) -> Result<()> {
    api.require_auth()?;

    match cmd {
        EdgeScannerCmd::Signals {
            strategy,
            min_edge,
            all,
            limit,
        } => {
            let mut params = vec![format!("limit={limit}")];
            if !all {
                params.push("activeOnly=true".into());
            } else {
                params.push("activeOnly=false".into());
            }
            if let Some(ref s) = strategy {
                params.push(format!("strategy={s}"));
            }
            if let Some(edge) = min_edge {
                params.push(format!("minEdgeBps={edge}"));
            }

            let qs = params.join("&");
            let sp = output::spinner("Fetching edge scanner signals…");
            let data: serde_json::Value = api
                .get_raw(&format!("/external/edge-scanner/signals?{qs}"))
                .await?;
            sp.finish_and_clear();

            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => print_signals_table(&data),
            }
        }
        EdgeScannerCmd::Curve => {
            let sp = output::spinner("Fetching calibration curve…");
            let data: serde_json::Value =
                api.get_raw("/external/edge-scanner/calibration").await?;
            sp.finish_and_clear();

            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => print_curve_table(&data),
            }
        }
    }
    Ok(())
}

fn print_signals_table(data: &serde_json::Value) {
    let signals = data["signals"].as_array().or_else(|| data.as_array());
    let Some(signals) = signals else {
        output::dimmed("(no signals)");
        return;
    };
    if signals.is_empty() {
        output::dimmed("(no signals)");
        return;
    }

    let rows: Vec<SignalRow> = signals
        .iter()
        .map(|s| {
            let edge_bps = s["edgeBps"].as_i64().unwrap_or(0);
            let market_price = s["marketPrice"].as_f64().unwrap_or(0.0);
            let fair_value = s["fairValue"].as_f64().unwrap_or(0.0);
            let kelly = s["kellyFraction"].as_f64().unwrap_or(0.0);
            let question = s["metadata"]["question"]
                .as_str()
                .or_else(|| s["rationale"].as_str())
                .unwrap_or("-");

            SignalRow {
                strategy: match output::str_val(s, "strategy").as_str() {
                    "calibration_arb" => "calib".into(),
                    "time_decay" => "decay".into(),
                    other => other.to_string(),
                },
                direction: output::str_val(s, "direction").to_uppercase(),
                edge: format!("{:.1}%", edge_bps as f64 / 100.0),
                price: format!("{:.1}¢", market_price * 100.0),
                fair: format!("{:.1}¢", fair_value * 100.0),
                kelly: format!("{:.2}%", kelly * 100.0),
                days: s["daysRemaining"]
                    .as_i64()
                    .map(|d| format!("{d}"))
                    .unwrap_or_else(|| "-".into()),
                market: output::truncate(question, 45),
            }
        })
        .collect();

    output::print_tabled_with_cols(&rows, &[2, 3, 4, 5]);
}

fn print_curve_table(data: &serde_json::Value) {
    let buckets = data["buckets"].as_array().or_else(|| data.as_array());
    let Some(buckets) = buckets else {
        output::dimmed("(no calibration data)");
        return;
    };
    if buckets.is_empty() {
        output::dimmed("(no calibration data)");
        return;
    }

    let rows: Vec<CurveRow> = buckets
        .iter()
        .map(|b| {
            let low = b["bucketLowBps"].as_i64().unwrap_or(0) as f64 / 100.0;
            let high = b["bucketHighBps"].as_i64().unwrap_or(0) as f64 / 100.0;
            let actual = b["actualRateBps"].as_i64().unwrap_or(0) as f64 / 100.0;
            let expected = b["expectedMidpointBps"].as_i64().unwrap_or(0) as f64 / 100.0;
            let edge = b["edgeBps"].as_i64().unwrap_or(0) as f64 / 100.0;
            let samples = b["sampleCount"].as_i64().unwrap_or(0);

            CurveRow {
                bucket: format!("{low:.0}%-{high:.0}%"),
                samples: format!("{samples}"),
                actual: format!("{actual:.1}%"),
                expected: format!("{expected:.1}%"),
                edge: if edge > 0.0 {
                    format!("+{edge:.1}%")
                } else {
                    format!("{edge:.1}%")
                },
            }
        })
        .collect();

    output::print_tabled_with_cols(&rows, &[1, 2, 3, 4]);
}

