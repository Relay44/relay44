use anyhow::{bail, Result};
use clap::Subcommand;
use tabled::Tabled;

use crate::client::Client;
use crate::output::{self, Format};

#[derive(Subcommand, Clone)]
pub enum OrderCmd {
    /// List your open orders
    #[command(
        long_about = "List your open orders, optionally filtered by market.\n\n\
                      Examples:\n  \
                      r44 orders list\n  \
                      r44 orders list --market abc123"
    )]
    List {
        /// Filter by market ID
        #[arg(long, short)]
        market: Option<String>,
        /// Max results
        #[arg(long, short, default_value = "50")]
        limit: u32,
        /// Offset for pagination
        #[arg(long, default_value = "0")]
        offset: u32,
    },

    /// Place a new order
    #[command(long_about = "Place a limit order on a prediction market.\n\n\
                      Price is a probability between 0.01 and 0.99.\n\
                      Side is either 'buy' (you think YES) or 'sell' (you think NO).\n\n\
                      Examples:\n  \
                      r44 orders place --market abc123 --side buy --price 0.65 --size 100\n  \
                      r44 orders place --market abc123 --side sell --price 0.30 --size 50 -y")]
    Place {
        /// Market ID
        #[arg(long, short)]
        market: String,
        /// Side: buy or sell
        #[arg(long, short)]
        side: String,
        /// Price (probability, 0.01–0.99)
        #[arg(long, short)]
        price: f64,
        /// Number of shares
        #[arg(long)]
        size: f64,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Cancel an open order
    #[command(long_about = "Cancel an open order by ID.\n\n\
                      Example:\n  r44 orders cancel abc123\n  \
                      r44 orders cancel abc123 -y")]
    Cancel {
        /// Order ID
        id: String,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Cancel all open orders
    #[command(long_about = "Cancel all your open orders.\n\n\
                      Example:\n  r44 orders cancel-all")]
    CancelAll {
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Get order details
    #[command(long_about = "Show details for a specific order.\n\n\
                      Example:\n  r44 orders get abc123")]
    Get {
        /// Order ID
        id: String,
    },
}

#[derive(Tabled)]
struct OrderRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Market")]
    market: String,
    #[tabled(rename = "Side")]
    side: String,
    #[tabled(rename = "Price")]
    price: String,
    #[tabled(rename = "Size")]
    size: String,
    #[tabled(rename = "Status")]
    status: String,
}

pub async fn run(cmd: OrderCmd, api: &Client, fmt: Format) -> Result<()> {
    api.require_auth()?;

    match cmd {
        OrderCmd::List { market, limit, offset } => {
            let sp = output::spinner("Fetching orders…");
            let mut path = format!("/orders?limit={limit}&offset={offset}");
            if let Some(m) = &market {
                path.push_str(&format!("&marketId={m}"));
            }
            let data: serde_json::Value = api.get_raw(&path).await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    let orders = data["orders"].as_array().or_else(|| data.as_array());
                    let Some(orders) = orders else {
                        output::dimmed("(no open orders)");
                        return Ok(());
                    };
                    let rows: Vec<OrderRow> = orders
                        .iter()
                        .map(|o| OrderRow {
                            id: output::truncate(&output::str_val(o, "id"), 20),
                            market: output::truncate(&output::str_val(o, "marketId"), 20),
                            side: output::str_val(o, "side"),
                            price: output::str_val(o, "price"),
                            size: output::str_val(o, "size"),
                            status: output::str_val(o, "status"),
                        })
                        .collect();
                    output::print_tabled_with_cols(&rows, &[3, 4]);
                }
            }
        }
        OrderCmd::Place {
            market,
            side,
            price,
            size,
            yes,
        } => {
            let side_lower = side.to_lowercase();
            if !["buy", "sell"].contains(&side_lower.as_str()) {
                bail!("Side must be 'buy' or 'sell' (got '{side}')");
            }
            if !(0.01..=0.99).contains(&price) {
                bail!("Price must be between 0.01 and 0.99 (got {price}). Price represents probability.");
            }
            if size <= 0.0 {
                bail!("Size must be positive (got {size})");
            }

            let cost = price * size;
            if !yes {
                let prompt = format!(
                    "Place {side_lower} order: {size} shares @ {:.1}¢ (cost: ${cost:.2})?",
                    price * 100.0
                );
                if !output::confirm(&prompt) {
                    output::dimmed("cancelled");
                    return Ok(());
                }
            }

            let sp = output::spinner("Placing order…");
            let body = serde_json::json!({
                "marketId": market,
                "side": side_lower,
                "price": price,
                "size": size,
            });
            let data: serde_json::Value = api.post_raw("/orders", &body).await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    output::success(&format!(
                        "Order placed: {side_lower} {size} @ {:.1}¢",
                        price * 100.0
                    ));
                    output::print_detail(&[
                        ("ID", output::str_val(&data, "id")),
                        ("Market", market),
                        ("Status", output::str_val(&data, "status")),
                    ]);
                }
            }
        }
        OrderCmd::Cancel { id, yes } => {
            if !yes && !output::confirm(&format!("Cancel order {id}?")) {
                output::dimmed("cancelled");
                return Ok(());
            }
            let sp = output::spinner("Cancelling order…");
            let data = api.delete_raw(&format!("/orders/{id}")).await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => output::success(&format!("Order {id} cancelled")),
            }
        }
        OrderCmd::CancelAll { yes } => {
            if !yes && !output::confirm("Cancel ALL open orders?") {
                output::dimmed("cancelled");
                return Ok(());
            }
            let sp = output::spinner("Fetching open orders…");
            let data: serde_json::Value = api.get_raw("/orders").await?;
            sp.finish_and_clear();

            let orders = data["orders"].as_array().or_else(|| data.as_array());
            let Some(orders) = orders else {
                output::dimmed("(no open orders)");
                return Ok(());
            };

            let count = orders.len();
            let sp = output::spinner(&format!("Cancelling {count} orders…"));
            let mut cancelled = 0;
            let mut failed = 0;
            for order in orders {
                if let Some(id) = order["id"].as_str() {
                    match api.delete_raw(&format!("/orders/{id}")).await {
                        Ok(_) => cancelled += 1,
                        Err(_) => failed += 1,
                    }
                }
            }
            sp.finish_and_clear();

            match fmt {
                Format::Json => output::print_json(&serde_json::json!({
                    "cancelled": cancelled,
                    "failed": failed,
                })),
                Format::Table => {
                    output::success(&format!("{cancelled} orders cancelled"));
                    if failed > 0 {
                        output::warn(&format!("{failed} orders failed to cancel"));
                    }
                }
            }
        }
        OrderCmd::Get { id } => {
            let sp = output::spinner("Fetching order…");
            let data: serde_json::Value = api.get_raw(&format!("/orders/{id}")).await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    output::print_detail(&[
                        ("ID", output::str_val(&data, "id")),
                        ("Market", output::str_val(&data, "marketId")),
                        ("Side", output::str_val(&data, "side")),
                        ("Price", output::str_val(&data, "price")),
                        ("Size", output::str_val(&data, "size")),
                        ("Filled", output::str_val(&data, "filledSize")),
                        ("Status", output::str_val(&data, "status")),
                        ("Created", output::format_date(&data, "createdAt")),
                    ]);
                }
            }
        }
    }
    Ok(())
}

