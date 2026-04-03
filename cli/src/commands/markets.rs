use anyhow::Result;
use clap::Subcommand;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use tabled::Tabled;

use crate::client::Client;
use crate::output::{self, Format};

#[derive(Subcommand, Clone)]
pub enum MarketCmd {
    /// List markets
    #[command(
        long_about = "List prediction markets, optionally filtered by status or search query.\n\n\
                      Examples:\n  \
                      r44 markets list\n  \
                      r44 markets list --status open --limit 50\n  \
                      r44 markets list --query \"bitcoin\" --offset 25"
    )]
    List {
        /// Filter by status: open, closed, resolved
        #[arg(long)]
        status: Option<String>,
        /// Search query
        #[arg(long)]
        query: Option<String>,
        /// Max results
        #[arg(long, short, default_value = "25")]
        limit: u32,
        /// Offset for pagination
        #[arg(long, default_value = "0")]
        offset: u32,
    },

    /// Get a single market by ID or slug
    #[command(long_about = "Show detailed information about a specific market.\n\
                            Accepts both numeric IDs and slug strings.\n\n\
                            Examples:\n  \
                            r44 markets get abc123\n  \
                            r44 markets get will-bitcoin-hit-100k")]
    Get {
        /// Market ID or slug
        id: String,
    },

    /// Show orderbook for a market
    #[command(long_about = "Display the live bid/ask orderbook for a market.\n\n\
                            Example:\n  r44 markets orderbook abc123")]
    Orderbook {
        /// Market ID
        id: String,
    },

    /// Show recent trades for a market
    #[command(long_about = "Show recent trade history for a market.\n\n\
                            Example:\n  r44 markets trades abc123 --limit 50")]
    Trades {
        /// Market ID
        id: String,
        /// Max results
        #[arg(long, short, default_value = "20")]
        limit: u32,
    },
}

#[derive(Tabled)]
struct MarketRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Price")]
    price: String,
    #[tabled(rename = "Volume")]
    volume: String,
}

#[derive(Tabled)]
struct BookRow {
    #[tabled(rename = "Price")]
    price: String,
    #[tabled(rename = "Size")]
    size: String,
}

#[derive(Tabled)]
struct TradeRow {
    #[tabled(rename = "Side")]
    side: String,
    #[tabled(rename = "Price")]
    price: String,
    #[tabled(rename = "Size")]
    size: String,
    #[tabled(rename = "Time")]
    time: String,
}

pub async fn run(cmd: MarketCmd, api: &Client, fmt: Format) -> Result<()> {
    match cmd {
        MarketCmd::List {
            status,
            query,
            limit,
            offset,
        } => {
            let sp = output::spinner("Fetching markets…");
            let mut path = format!("/markets?limit={limit}&offset={offset}");
            if let Some(s) = &status {
                path.push_str(&format!("&status={s}"));
            }
            if let Some(q) = &query {
                let encoded = utf8_percent_encode(q, NON_ALPHANUMERIC).to_string();
                path.push_str(&format!("&q={encoded}"));
            }
            let data: serde_json::Value = api.get_raw(&path).await?;
            sp.finish_and_clear();

            let total = data["total"].as_u64();

            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    let markets = data["markets"].as_array().or_else(|| data.as_array());
                    let Some(markets) = markets else {
                        output::dimmed("(no markets)");
                        return Ok(());
                    };
                    let rows: Vec<MarketRow> = markets
                        .iter()
                        .map(|m| MarketRow {
                            id: output::truncate(&output::str_val(m, "id"), 20),
                            title: output::truncate(&output::str_val(m, "title"), 50),
                            status: output::str_val(m, "status"),
                            price: output::price_field(m, "lastPrice"),
                            volume: output::str_val(m, "volume"),
                        })
                        .collect();
                    output::print_tabled_with_cols(&rows, &[3, 4]);
                    output::pagination_hint(offset, limit, total);
                }
            }
        }
        MarketCmd::Get { id } => {
            let sp = output::spinner("Fetching market…");
            let data: serde_json::Value = api.get_raw(&format!("/markets/{id}")).await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    output::print_detail(&[
                        ("Title", output::str_val(&data, "title")),
                        ("ID", output::str_val(&data, "id")),
                        ("Status", output::active_status(&data)),
                        ("Price", output::price_field(&data, "lastPrice")),
                        ("Volume", output::str_val(&data, "volume")),
                        ("Created", output::format_date(&data, "createdAt")),
                    ]);
                    if let Some(desc) = data["description"].as_str() {
                        if !desc.is_empty() {
                            println!("\n{desc}");
                        }
                    }
                }
            }
        }
        MarketCmd::Orderbook { id } => {
            let sp = output::spinner("Fetching orderbook…");
            let data: serde_json::Value = api.get_raw(&format!("/markets/{id}/orderbook")).await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    println!("BIDS");
                    print_book_side(&data["bids"]);
                    println!("\nASKS");
                    print_book_side(&data["asks"]);
                }
            }
        }
        MarketCmd::Trades { id, limit } => {
            let sp = output::spinner("Fetching trades…");
            let data: serde_json::Value = api
                .get_raw(&format!("/markets/{id}/trades?limit={limit}"))
                .await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    let trades = data["trades"].as_array().or_else(|| data.as_array());
                    let Some(trades) = trades else {
                        output::dimmed("(no trades)");
                        return Ok(());
                    };
                    let rows: Vec<TradeRow> = trades
                        .iter()
                        .map(|t| TradeRow {
                            side: output::str_val(t, "side"),
                            price: output::price_field(t, "price"),
                            size: output::str_val(t, "size"),
                            time: output::format_date(t, "createdAt"),
                        })
                        .collect();
                    output::print_tabled_with_cols(&rows, &[1, 2]);
                }
            }
        }
    }
    Ok(())
}

fn print_book_side(side: &serde_json::Value) {
    let Some(entries) = side.as_array() else {
        output::dimmed("  (empty)");
        return;
    };
    let rows: Vec<BookRow> = entries
        .iter()
        .map(|e| BookRow {
            price: output::price_field(e, "price"),
            size: output::str_val(e, "size"),
        })
        .collect();
    output::print_tabled_with_cols(&rows, &[0, 1]);
}
