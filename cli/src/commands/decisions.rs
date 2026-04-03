use anyhow::Result;
use clap::Subcommand;
use tabled::Tabled;

use crate::client::Client;
use crate::output::{self, Format};

#[derive(Subcommand, Clone)]
pub enum DecisionCmd {
    /// List your decision cells
    #[command(
        long_about = "List decision cells — DAG-based automation nodes for market trading rules.\n\n\
                      Examples:\n  \
                      r44 decisions list\n  \
                      r44 decisions list --status active --limit 10"
    )]
    List {
        /// Filter by status: active, inactive, draft
        #[arg(long)]
        status: Option<String>,
        /// Max results
        #[arg(long, short, default_value = "25")]
        limit: u32,
        /// Offset for pagination
        #[arg(long, default_value = "0")]
        offset: u32,
    },

    /// Get a decision cell by ID
    #[command(
        long_about = "Show full details of a decision cell including nodes, actions, and alerts.\n\n\
                      Example:\n  r44 decisions get <CELL_ID>"
    )]
    Get {
        /// Decision cell ID
        id: String,
    },

    /// Create a new decision cell
    #[command(
        long_about = "Create a new decision cell for automated market trading.\n\n\
                      Types: timing, choice, hedge, allocation\n\n\
                      Examples:\n  \
                      r44 decisions create --title \"BTC hedge\" --statement \"Should I hedge?\" --type hedge\n  \
                      r44 decisions create --title \"Election timing\" --statement \"When to enter?\" --type timing --horizon 2025-11-05T00:00:00Z"
    )]
    Create {
        /// Cell title (max 160 chars)
        #[arg(long)]
        title: String,
        /// Decision statement (max 4000 chars)
        #[arg(long)]
        statement: String,
        /// Decision type: timing, choice, hedge, allocation
        #[arg(long, name = "type")]
        decision_type: String,
        /// Decision horizon (RFC3339 datetime)
        #[arg(long)]
        horizon: Option<String>,
        /// Action labels (comma-separated)
        #[arg(long)]
        actions: Option<String>,
    },
}

#[derive(Tabled)]
struct DecisionRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Type")]
    decision_type: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Recommendation")]
    recommendation: String,
    #[tabled(rename = "Confidence")]
    confidence: String,
}

pub async fn run(cmd: DecisionCmd, api: &Client, fmt: Format) -> Result<()> {
    api.require_auth()?;

    match cmd {
        DecisionCmd::List {
            status,
            limit,
            offset,
        } => {
            let sp = output::spinner("Fetching decisions…");
            let mut path = format!("/decisions?limit={limit}&offset={offset}");
            if let Some(s) = &status {
                path.push_str(&format!("&status={s}"));
            }
            let data = api.get_raw(&path).await?;
            sp.finish_and_clear();

            let total = data["total"].as_u64();

            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    let Some(cells) = data["data"].as_array() else {
                        output::dimmed("(no decision cells)");
                        return Ok(());
                    };
                    let rows: Vec<DecisionRow> = cells.iter().map(decision_row).collect();
                    output::print_tabled(&rows);
                    output::pagination_hint(offset, limit, total);
                }
            }
        }
        DecisionCmd::Get { id } => {
            let sp = output::spinner("Fetching decision…");
            let data = api.get_raw(&format!("/decisions/{id}")).await?;
            sp.finish_and_clear();

            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    let rec = &data["recommendation"];
                    output::print_detail(&[
                        ("Title", output::str_val(&data, "title")),
                        ("ID", output::str_val(&data, "id")),
                        ("Type", output::str_val(&data, "decision_type")),
                        ("Status", output::str_val(&data, "status")),
                        ("Automation", format!("{}", data["automation_enabled"].as_bool().unwrap_or(false))),
                        ("Recommendation", output::str_val(rec, "state")),
                        ("Confidence", format_bps(rec["confidence_bps"].as_i64())),
                        ("Created", output::format_date(&data, "created_at")),
                    ]);

                    if let Some(nodes) = data["nodes"].as_array() {
                        if !nodes.is_empty() {
                            println!("\nNodes ({}):", nodes.len());
                            for n in nodes {
                                let label = output::str_val(n, "label");
                                let source = output::str_val(n, "source_type");
                                let status = output::str_val(n, "status");
                                let prob = n["last_probability_bps"]
                                    .as_i64()
                                    .map(|b| format!(" {}%", b as f64 / 100.0))
                                    .unwrap_or_default();
                                println!("  • {label} [{source}] {status}{prob}");
                            }
                        }
                    }

                    if let Some(actions) = data["actions"].as_array() {
                        if !actions.is_empty() {
                            println!("\nActions:");
                            for a in actions {
                                let label = output::str_val(a, "label");
                                let score = format_bps(a["score_bps"].as_i64());
                                println!("  • {label} — {score}");
                            }
                        }
                    }
                }
            }
        }
        DecisionCmd::Create {
            title,
            statement,
            decision_type,
            horizon,
            actions,
        } => {
            let sp = output::spinner("Creating decision cell…");
            let mut body = serde_json::json!({
                "title": title,
                "statement": statement,
                "decision_type": decision_type,
            });
            if let Some(h) = horizon {
                body["horizon_at"] = serde_json::Value::String(h);
            }
            if let Some(a) = actions {
                let labels: Vec<&str> = a.split(',').map(|s| s.trim()).collect();
                body["actions"] = serde_json::json!(labels);
            }
            let data = api.post_raw("/decisions", &body).await?;
            sp.finish_and_clear();

            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    output::success(&format!(
                        "Created decision cell '{}'",
                        output::str_val(&data, "title")
                    ));
                    output::print_detail(&[
                        ("ID", output::str_val(&data, "id")),
                        ("Type", output::str_val(&data, "decision_type")),
                        ("Status", output::str_val(&data, "status")),
                    ]);
                }
            }
        }
    }
    Ok(())
}

fn decision_row(val: &serde_json::Value) -> DecisionRow {
    let rec = &val["recommendation"];
    DecisionRow {
        id: output::truncate(&output::str_val(val, "id"), 12),
        title: output::truncate(&output::str_val(val, "title"), 40),
        decision_type: output::str_val(val, "decision_type"),
        status: output::str_val(val, "status"),
        recommendation: output::str_val(rec, "state"),
        confidence: format_bps(rec["confidence_bps"].as_i64()),
    }
}

fn format_bps(bps: Option<i64>) -> String {
    match bps {
        Some(b) => format!("{:.1}%", b as f64 / 100.0),
        None => "—".into(),
    }
}
