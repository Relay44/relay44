use anyhow::{bail, Result};
use clap::Subcommand;
use tabled::Tabled;

use crate::client::Client;
use crate::output::{self, Format};

#[derive(Subcommand, Clone)]
pub enum PositionCmd {
    /// List all positions
    #[command(long_about = "List all your open positions across markets.\n\n\
                      Example:\n  r44 positions list")]
    List,

    /// Get position for a specific market
    #[command(long_about = "Show your position in a specific market.\n\n\
                      Example:\n  r44 positions get abc123")]
    Get {
        /// Market ID
        market_id: String,
    },

    /// Claim winnings for a resolved market
    #[command(
        long_about = "Claim your winnings from a resolved market. This is irreversible.\n\n\
                      Example:\n  r44 positions claim abc123"
    )]
    Claim {
        /// Market ID
        market_id: String,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Tabled)]
struct PositionRow {
    #[tabled(rename = "Market")]
    market: String,
    #[tabled(rename = "Side")]
    side: String,
    #[tabled(rename = "Size")]
    size: String,
    #[tabled(rename = "Avg Price")]
    avg_price: String,
    #[tabled(rename = "P&L")]
    pnl: String,
}

pub async fn run(cmd: PositionCmd, api: &Client, fmt: Format) -> Result<()> {
    require_auth(api)?;

    match cmd {
        PositionCmd::List => {
            let sp = output::spinner("Fetching positions…");
            let data: serde_json::Value = api.get_raw("/positions").await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    let positions = data["positions"].as_array().or_else(|| data.as_array());
                    let Some(positions) = positions else {
                        output::dimmed("(no positions)");
                        return Ok(());
                    };
                    let rows: Vec<PositionRow> = positions
                        .iter()
                        .map(|p| PositionRow {
                            market: output::truncate(&output::str_val(p, "marketId"), 20),
                            side: output::str_val(p, "side"),
                            size: output::str_val(p, "size"),
                            avg_price: output::str_val(p, "avgPrice"),
                            pnl: output::str_val(p, "pnl"),
                        })
                        .collect();
                    output::print_tabled_with_cols(&rows, &[2, 3, 4]);
                }
            }
        }
        PositionCmd::Get { market_id } => {
            let sp = output::spinner("Fetching position…");
            let data: serde_json::Value = api.get_raw(&format!("/positions/{market_id}")).await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    output::print_detail(&[
                        ("Market", output::str_val(&data, "marketId")),
                        ("Side", output::str_val(&data, "side")),
                        ("Size", output::str_val(&data, "size")),
                        ("Avg Price", output::str_val(&data, "avgPrice")),
                        ("P&L", output::str_val(&data, "pnl")),
                    ]);
                }
            }
        }
        PositionCmd::Claim { market_id, yes } => {
            if !yes
                && !output::confirm(&format!(
                    "Claim winnings for market {market_id}? This cannot be undone."
                ))
            {
                output::dimmed("cancelled");
                return Ok(());
            }
            let sp = output::spinner("Claiming winnings…");
            let body = serde_json::json!({});
            let data: serde_json::Value = api
                .post_raw(&format!("/positions/{market_id}/claim"), &body)
                .await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    if let Some(amount) = data["amount"].as_f64() {
                        output::success(&format!(
                            "Claimed {} from market {market_id}",
                            output::usdc(amount)
                        ));
                    } else {
                        output::success(&format!("Claimed winnings for market {market_id}"));
                    }
                }
            }
        }
    }
    Ok(())
}

fn require_auth(api: &Client) -> Result<()> {
    if api.is_authenticated() {
        return Ok(());
    }
    bail!(
        "Not logged in.\n\n  \
         r44 login solana --wallet <PUBKEY> --private-key <KEY>\n  \
         r44 config set-token <TOKEN>  (if you have a token already)"
    );
}
