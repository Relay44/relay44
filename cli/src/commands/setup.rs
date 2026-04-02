use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::config::Config;
use crate::output;

pub async fn run(config: Arc<Mutex<Config>>, api_url: &str) -> Result<()> {
    println!();
    println!("╭─────────────────────────────────────╮");
    println!("│     r44 — first-time setup           │");
    println!("╰─────────────────────────────────────╯");
    println!();

    // Step 1: API connection
    println!("Step 1/3 — Checking API connection");
    let sp = output::spinner("Connecting…");
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let health = http
        .get(format!(
            "{}/health",
            api_url.trim_end_matches("/v1")
        ))
        .send()
        .await;
    sp.finish_and_clear();

    match health {
        Ok(r) if r.status().is_success() => {
            output::success(&format!("API reachable at {api_url}"));
        }
        _ => {
            output::warn(&format!("Could not reach API at {api_url}"));
            output::dimmed("  You can change this later: r44 config set-url <URL>");
        }
    }
    println!();

    // Step 2: Wallet configuration
    println!("Step 2/3 — Wallet configuration");
    println!("  How do you want to authenticate?");
    println!("  [1] I have a Solana private key (agent/automated use)");
    println!("  [2] I have a JWT token from the web app");
    println!("  [3] Skip for now (browse markets without auth)");
    println!();

    let choice = prompt("Choose [1/2/3]: ")?;

    match choice.trim() {
        "1" => {
            let wallet = prompt("Solana wallet address (base58): ")?;
            let wallet = wallet.trim().to_string();
            if wallet.is_empty() {
                output::warn("No wallet provided, skipping auth");
            } else {
                println!();
                output::dimmed("To complete login, run:");
                output::dimmed(&format!(
                    "  r44 login solana --wallet {wallet} --private-key <YOUR_KEY>"
                ));
                println!();
                output::dimmed("Or set environment variables:");
                output::dimmed(&format!("  export R44_WALLET={wallet}"));
                output::dimmed("  export R44_PRIVATE_KEY=<YOUR_KEY>");

                let mut cfg = config.lock().unwrap();
                cfg.wallet = Some(wallet);
                cfg.save()?;
            }
        }
        "2" => {
            let token = prompt("JWT access token: ")?;
            let token = token.trim().to_string();
            if token.is_empty() {
                output::warn("No token provided, skipping auth");
            } else {
                let mut cfg = config.lock().unwrap();
                cfg.access_token = Some(token);
                cfg.save()?;
                output::success("Token saved");
            }
        }
        _ => {
            output::dimmed("Skipping auth — you can set up later with `r44 login`");
        }
    }
    println!();

    // Step 3: Shell completions
    println!("Step 3/3 — Shell completions");
    let shell = std::env::var("SHELL").unwrap_or_default();
    if shell.contains("zsh") {
        output::dimmed("  r44 completions zsh > ~/.zfunc/_r44 && echo 'fpath=(~/.zfunc $fpath); autoload -Uz compinit && compinit' >> ~/.zshrc");
    } else if shell.contains("bash") {
        output::dimmed("  r44 completions bash >> ~/.bashrc");
    } else if shell.contains("fish") {
        output::dimmed("  r44 completions fish > ~/.config/fish/completions/r44.fish");
    } else {
        output::dimmed("  r44 completions <bash|zsh|fish>");
    }
    println!();

    // Done
    println!("╭─────────────────────────────────────╮");
    println!("│     Setup complete!                  │");
    println!("╰─────────────────────────────────────╯");
    println!();
    println!("  Get started:");
    println!("    r44 markets list          Browse markets");
    println!("    r44 agents public         See community agents");
    println!("    r44 shell                 Interactive mode");
    println!("    r44 --help                Full command list");
    println!();

    Ok(())
}

fn prompt(msg: &str) -> Result<String> {
    eprint!("{msg}");
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input)
}
