use anyhow::Result;
use clap::Subcommand;
use tabled::Tabled;

use crate::client::Client;
use crate::output::{self, Format};

#[derive(Subcommand, Clone)]
pub enum LeaderboardCmd {
    /// Show top traders
    #[command(
        long_about = "Show the prediction market leaderboard, ranked by PnL, volume, trade count, or win rate.\n\n\
                      Examples:\n  \
                      r44 leaderboard top\n  \
                      r44 leaderboard top --metric volume --period weekly\n  \
                      r44 leaderboard top --limit 50"
    )]
    Top {
        /// Ranking metric: pnl, volume, trades, win_rate
        #[arg(long, default_value = "pnl")]
        metric: String,
        /// Time period: daily, weekly, monthly, all_time
        #[arg(long, default_value = "all_time")]
        period: String,
        /// Max results
        #[arg(long, short, default_value = "25")]
        limit: u32,
        /// Offset for pagination
        #[arg(long, default_value = "0")]
        offset: u32,
    },

    /// Show rank for a specific wallet
    #[command(
        long_about = "Look up a wallet's leaderboard rank and score.\n\n\
                      Examples:\n  \
                      r44 leaderboard rank <WALLET>\n  \
                      r44 leaderboard rank <WALLET> --metric win_rate --period weekly"
    )]
    Rank {
        /// Wallet address
        wallet: String,
        /// Ranking metric: pnl, volume, trades, win_rate
        #[arg(long, default_value = "pnl")]
        metric: String,
        /// Time period: daily, weekly, monthly, all_time
        #[arg(long, default_value = "all_time")]
        period: String,
    },
}

#[derive(Tabled)]
struct LeaderboardRow {
    #[tabled(rename = "#")]
    rank: String,
    #[tabled(rename = "Wallet")]
    wallet: String,
    #[tabled(rename = "Value")]
    value: String,
    #[tabled(rename = "Change")]
    change: String,
}

pub async fn run(cmd: LeaderboardCmd, api: &Client, fmt: Format) -> Result<()> {
    match cmd {
        LeaderboardCmd::Top {
            metric,
            period,
            limit,
            offset,
        } => {
            let sp = output::spinner("Fetching leaderboard…");
            let path = format!(
                "/leaderboard?metric={metric}&period={period}&limit={limit}&offset={offset}"
            );
            let data = api.get_raw(&path).await?;
            sp.finish_and_clear();

            let total = data["total"].as_u64();

            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    let Some(entries) = data["entries"].as_array() else {
                        output::dimmed("(no entries)");
                        return Ok(());
                    };
                    let rows: Vec<LeaderboardRow> = entries
                        .iter()
                        .map(|e| LeaderboardRow {
                            rank: output::str_val(e, "rank"),
                            wallet: output::truncate(&output::str_val(e, "wallet"), 20),
                            value: output::str_val(e, "value"),
                            change: e["change"]
                                .as_f64()
                                .map(|c| format!("{c:+.2}"))
                                .unwrap_or_else(|| "—".into()),
                        })
                        .collect();
                    output::print_tabled_with_cols(&rows, &[0, 2, 3]);
                    output::pagination_hint(offset, limit, total);
                }
            }
        }
        LeaderboardCmd::Rank {
            wallet,
            metric,
            period,
        } => {
            let sp = output::spinner("Fetching rank…");
            let path = format!("/leaderboard/rank/{wallet}?metric={metric}&period={period}");
            let data = api.get_raw(&path).await?;
            sp.finish_and_clear();

            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    output::print_detail(&[
                        ("Wallet", output::str_val(&data, "wallet")),
                        ("Rank", format!("#{}", output::str_val(&data, "rank"))),
                        ("Value", output::str_val(&data, "value")),
                        ("Metric", output::str_val(&data, "metric")),
                        ("Period", output::str_val(&data, "period")),
                    ]);
                }
            }
        }
    }
    Ok(())
}
