use anyhow::Result;
use clap::Subcommand;
use tabled::Tabled;

use crate::client::Client;
use crate::output::{self, Format};

#[derive(Subcommand, Clone)]
pub enum ActivityCmd {
    /// List your transactions
    #[command(
        long_about = "Show transaction history: deposits, withdrawals, trades, claims.\n\n\
                      Examples:\n  \
                      r44 activity list\n  \
                      r44 activity list --type buy --limit 50\n  \
                      r44 activity list --type claim"
    )]
    List {
        /// Filter by type: deposit, withdraw, buy, sell, claim, mint, redeem
        #[arg(long, name = "type")]
        tx_type: Option<String>,
        /// Max results
        #[arg(long, short, default_value = "25")]
        limit: u32,
        /// Offset for pagination
        #[arg(long, default_value = "0")]
        offset: u32,
    },
}

#[derive(Tabled)]
struct TransactionRow {
    #[tabled(rename = "Type")]
    tx_type: String,
    #[tabled(rename = "Amount")]
    amount: String,
    #[tabled(rename = "Fee")]
    fee: String,
    #[tabled(rename = "Market")]
    market: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Time")]
    time: String,
}

pub async fn run(cmd: ActivityCmd, api: &Client, fmt: Format) -> Result<()> {
    api.require_auth()?;

    match cmd {
        ActivityCmd::List {
            tx_type,
            limit,
            offset,
        } => {
            let sp = output::spinner("Fetching transactions…");
            let mut path = format!("/user/transactions?limit={limit}&offset={offset}");
            if let Some(t) = &tx_type {
                path.push_str(&format!("&tx_type={t}"));
            }
            let data = api.get_raw(&path).await?;
            sp.finish_and_clear();

            let total = data["total"].as_u64();

            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    let Some(txs) = data["transactions"].as_array() else {
                        output::dimmed("(no transactions)");
                        return Ok(());
                    };
                    let rows: Vec<TransactionRow> = txs
                        .iter()
                        .map(|t| TransactionRow {
                            tx_type: output::str_val(t, "tx_type"),
                            amount: format_usdc(t["amount"].as_u64()),
                            fee: format_usdc(t["fee"].as_u64()),
                            market: t["market_id"]
                                .as_str()
                                .map(|s| output::truncate(s, 16))
                                .unwrap_or_else(|| "—".into()),
                            status: output::str_val(t, "status"),
                            time: output::format_date(t, "created_at"),
                        })
                        .collect();
                    output::print_tabled_with_cols(&rows, &[1, 2]);
                    output::pagination_hint(offset, limit, total);
                }
            }
        }
    }
    Ok(())
}

fn format_usdc(lamports: Option<u64>) -> String {
    match lamports {
        Some(l) => format!("{:.2}", l as f64 / 1_000_000.0),
        None => "—".into(),
    }
}
