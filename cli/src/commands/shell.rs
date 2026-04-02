use std::sync::{Arc, Mutex};

use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use crate::client::Client;
use crate::config::Config;
use crate::output::{self, Format};

pub async fn run(api: &Client, config: Arc<Mutex<Config>>, api_url: &str, fmt: Format) -> Result<()> {
    output::banner();
    output::dimmed("  interactive shell — type commands without the 'r44' prefix");
    output::dimmed("  type 'help' for commands, 'exit' to quit");
    println!();

    let mut rl = DefaultEditor::new()?;

    let history_path = dirs::config_dir()
        .unwrap_or_default()
        .join("r44")
        .join("history");
    let _ = rl.load_history(&history_path);

    loop {
        if crate::is_interrupted() {
            break;
        }

        let readline = rl.readline("r44> ");
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(line);

                match line {
                    "exit" | "quit" => break,
                    "help" => print_help(),
                    _ => {
                        let args = split_args(line);
                        if let Err(e) = dispatch(&args, api, config.clone(), api_url, fmt).await {
                            output::error(&format!("{e:#}"));
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(e) => {
                output::error(&format!("readline: {e}"));
                break;
            }
        }
    }

    let _ = std::fs::create_dir_all(
        history_path.parent().unwrap_or(std::path::Path::new(".")),
    );
    let _ = rl.save_history(&history_path);

    output::dimmed("goodbye");
    Ok(())
}

async fn dispatch(
    args: &[String],
    api: &Client,
    config: Arc<Mutex<Config>>,
    api_url: &str,
    fmt: Format,
) -> Result<()> {
    if args.is_empty() {
        return Ok(());
    }

    let cmd = args[0].as_str();
    let rest = &args[1..];

    match cmd {
        "markets" => dispatch_markets(rest, api, fmt).await,
        "orders" => dispatch_orders(rest, api, fmt).await,
        "positions" => dispatch_positions(rest, api, fmt).await,
        "agents" => dispatch_agents(rest, api, fmt).await,
        "wallet" => dispatch_wallet(rest, api, fmt).await,
        "config" => {
            dispatch_config(rest, config);
            Ok(())
        }
        "login" => dispatch_login(rest, config, api_url).await,
        "shell" => {
            output::warn("already in shell mode");
            Ok(())
        }
        "setup" => {
            output::warn("setup must be run outside the shell: r44 setup");
            Ok(())
        }
        _ => {
            output::warn(&format!("unknown command: {cmd}. Type 'help' for available commands."));
            Ok(())
        }
    }
}

async fn dispatch_markets(args: &[String], api: &Client, fmt: Format) -> Result<()> {
    use super::markets::MarketCmd;
    match args.first().map(|s| s.as_str()) {
        Some("list") | None => {
            super::markets::run(
                MarketCmd::List { status: None, query: None, limit: 25, offset: 0 },
                api,
                fmt,
            )
            .await
        }
        Some("get") if args.len() > 1 => {
            super::markets::run(MarketCmd::Get { id: args[1].clone() }, api, fmt).await
        }
        Some("orderbook") if args.len() > 1 => {
            super::markets::run(MarketCmd::Orderbook { id: args[1].clone() }, api, fmt).await
        }
        Some("trades") if args.len() > 1 => {
            super::markets::run(
                MarketCmd::Trades { id: args[1].clone(), limit: 20 },
                api,
                fmt,
            )
            .await
        }
        _ => {
            output::dimmed("usage: markets [list|get|orderbook|trades] [id]");
            Ok(())
        }
    }
}

async fn dispatch_orders(args: &[String], api: &Client, fmt: Format) -> Result<()> {
    use super::orders::OrderCmd;
    match args.first().map(|s| s.as_str()) {
        Some("list") | None => {
            super::orders::run(OrderCmd::List { market: None }, api, fmt).await
        }
        Some("get") if args.len() > 1 => {
            super::orders::run(OrderCmd::Get { id: args[1].clone() }, api, fmt).await
        }
        Some("cancel") if args.len() > 1 => {
            super::orders::run(
                OrderCmd::Cancel { id: args[1].clone(), yes: false },
                api,
                fmt,
            )
            .await
        }
        Some("cancel-all") => {
            super::orders::run(OrderCmd::CancelAll { yes: false }, api, fmt).await
        }
        _ => {
            output::dimmed("usage: orders [list|get|cancel|cancel-all] [id]");
            Ok(())
        }
    }
}

async fn dispatch_positions(args: &[String], api: &Client, fmt: Format) -> Result<()> {
    use super::positions::PositionCmd;
    match args.first().map(|s| s.as_str()) {
        Some("list") | None => {
            super::positions::run(PositionCmd::List, api, fmt).await
        }
        Some("get") if args.len() > 1 => {
            super::positions::run(
                PositionCmd::Get { market_id: args[1].clone() },
                api,
                fmt,
            )
            .await
        }
        Some("claim") if args.len() > 1 => {
            super::positions::run(
                PositionCmd::Claim { market_id: args[1].clone(), yes: false },
                api,
                fmt,
            )
            .await
        }
        _ => {
            output::dimmed("usage: positions [list|get|claim] [market_id]");
            Ok(())
        }
    }
}

async fn dispatch_agents(args: &[String], api: &Client, fmt: Format) -> Result<()> {
    use super::agents::AgentCmd;
    match args.first().map(|s| s.as_str()) {
        Some("list") | None => {
            super::agents::run(AgentCmd::List, api, fmt).await
        }
        Some("public") => {
            super::agents::run(AgentCmd::Public, api, fmt).await
        }
        Some("get") if args.len() > 1 => {
            super::agents::run(AgentCmd::Get { id: args[1].clone() }, api, fmt).await
        }
        Some("execute") if args.len() > 1 => {
            super::agents::run(AgentCmd::Execute { id: args[1].clone() }, api, fmt).await
        }
        _ => {
            output::dimmed("usage: agents [list|public|get|execute] [id]");
            Ok(())
        }
    }
}

async fn dispatch_wallet(args: &[String], api: &Client, fmt: Format) -> Result<()> {
    use super::wallet::WalletCmd;
    match args.first().map(|s| s.as_str()) {
        Some("balance") | None => {
            super::wallet::run(WalletCmd::Balance, api, fmt).await
        }
        Some("deposit-address") => {
            super::wallet::run(WalletCmd::DepositAddress, api, fmt).await
        }
        _ => {
            output::dimmed("usage: wallet [balance|deposit-address]");
            Ok(())
        }
    }
}

fn dispatch_config(args: &[String], config: Arc<Mutex<Config>>) {
    use super::config::ConfigCmd;
    let cmd = match args.first().map(|s| s.as_str()) {
        Some("show") | None => ConfigCmd::Show,
        Some("path") => ConfigCmd::Path,
        _ => {
            output::dimmed("usage: config [show|path]");
            return;
        }
    };
    if let Err(e) = super::config::run(cmd, config) {
        output::error(&format!("{e:#}"));
    }
}

async fn dispatch_login(args: &[String], config: Arc<Mutex<Config>>, api_url: &str) -> Result<()> {
    use super::login::LoginCmd;
    match args.first().map(|s| s.as_str()) {
        Some("status") | None => {
            super::login::run(LoginCmd::Status, config, api_url).await
        }
        Some("logout") => {
            super::login::run(LoginCmd::Logout, config, api_url).await
        }
        _ => {
            output::dimmed("usage: login [status|logout] (use full r44 command for login flow)");
            Ok(())
        }
    }
}

fn split_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '"';

    for ch in input.chars() {
        if in_quotes {
            if ch == quote_char {
                in_quotes = false;
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            in_quotes = true;
            quote_char = ch;
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                args.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

fn print_help() {
    println!("Available commands:");
    println!("  markets [list|get|orderbook|trades]  Browse prediction markets");
    println!("  orders [list|get|cancel|cancel-all]  Manage orders");
    println!("  positions [list|get|claim]            View positions");
    println!("  agents [list|public|get|execute]      Manage agents");
    println!("  wallet [balance|deposit-address]      Wallet info");
    println!("  config [show|path]                    CLI settings");
    println!("  login [status|logout]                 Auth status");
    println!("  help                                  This message");
    println!("  exit                                  Quit shell");
}
