use anyhow::Result;
use clap::Subcommand;

use crate::client::Client;
use crate::output::{self, Format};

#[derive(Subcommand, Clone)]
pub enum WalletCmd {
    /// Show wallet balance
    #[command(
        long_about = "Show your current wallet balance, available funds, and locked amount.\n\n\
                      Example:\n  r44 wallet balance"
    )]
    Balance,

    /// Show deposit address
    #[command(long_about = "Show the deposit address for your account.\n\n\
                      Example:\n  r44 wallet deposit-address")]
    DepositAddress,
}

pub async fn run(cmd: WalletCmd, api: &Client, fmt: Format) -> Result<()> {
    api.require_auth()?;

    match cmd {
        WalletCmd::Balance => {
            let sp = output::spinner("Fetching balance…");
            let data: serde_json::Value = api.get_raw("/wallet/balance").await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    let balance = data["balance"]
                        .as_f64()
                        .map(output::usdc)
                        .unwrap_or_else(|| output::str_val(&data, "balance"));
                    let available = data["available"]
                        .as_f64()
                        .map(output::usdc)
                        .unwrap_or_else(|| "—".into());
                    let locked = data["locked"]
                        .as_f64()
                        .map(output::usdc)
                        .unwrap_or_else(|| "—".into());
                    output::print_detail(&[
                        ("Balance", balance),
                        ("Available", available),
                        ("In orders", locked),
                    ]);
                }
            }
        }
        WalletCmd::DepositAddress => {
            let sp = output::spinner("Fetching deposit address…");
            let data: serde_json::Value = api.get_raw("/wallet/deposit/address").await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    println!("{}", output::str_val(&data, "address"));
                }
            }
        }
    }
    Ok(())
}

