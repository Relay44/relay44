use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::config::Config;
use crate::output::{self, Format};

pub async fn run(config: Arc<Mutex<Config>>, profile_name: &str, api_url: &str) -> Result<()> {
    println!();
    output::banner();
    output::dimmed(&format!("  setup profile '{profile_name}'"));
    println!();

    {
        let mut config = config.lock().unwrap();
        config.ensure_profile(profile_name).api_url = api_url.to_string();
        config.save()?;
    }

    println!("1/4 API connectivity");
    let health = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?
        .get(format!("{}/health", api_url.trim_end_matches("/v1")))
        .send()
        .await;

    match health {
        Ok(response) if response.status().is_success() => {
            output::success(&format!("API reachable at {api_url}"));
        }
        Ok(response) => {
            output::warn(&format!("health check returned {}", response.status()));
        }
        Err(error) => {
            output::warn(&format!("could not reach API at {api_url}: {error}"));
        }
    }
    println!();

    println!("2/4 authentication");
    println!("  [1] Solana wallet (agent mode)");
    println!("  [2] JWT token");
    println!("  [3] Skip for now");
    println!();

    match prompt("Choose [1/2/3]: ")?.trim() {
        "1" => {
            let wallet = prompt("Solana wallet address: ")?;
            let wallet = wallet.trim();
            if wallet.is_empty() {
                output::warn("wallet was empty, skipping auth");
            } else {
                let mut config = config.lock().unwrap();
                config.ensure_profile(profile_name).wallet = Some(wallet.to_string());
                config.save()?;
                output::dimmed(&format!(
                    "next: r44 --profile {profile_name} login solana --wallet {wallet} --private-key <KEY>"
                ));
            }
        }
        "2" => {
            let token = prompt("JWT access token: ")?;
            let token = token.trim();
            if token.is_empty() {
                output::warn("token was empty, skipping auth");
            } else {
                let mut config = config.lock().unwrap();
                config.ensure_profile(profile_name).access_token = Some(token.to_string());
                config.save()?;
                output::success("token saved");
            }
        }
        _ => output::dimmed("skipping auth"),
    }
    println!();

    println!("3/4 default output");
    println!("  [1] table");
    println!("  [2] json");
    println!();
    let output_choice = prompt("Choose [1/2]: ")?;
    let format = match output_choice.trim() {
        "2" => Format::Json,
        _ => Format::Table,
    };
    {
        let mut config = config.lock().unwrap();
        config.ensure_profile(profile_name).output = Some(format);
        config.save()?;
    }
    output::success(&format!("default output → {format}"));
    println!();

    println!("4/4 shell completions");
    let shell = std::env::var("SHELL").unwrap_or_default();
    if shell.contains("zsh") {
        output::dimmed("  r44 completions zsh > ~/.zfunc/_r44");
    } else if shell.contains("bash") {
        output::dimmed("  r44 completions bash >> ~/.bashrc");
    } else if shell.contains("fish") {
        output::dimmed("  r44 completions fish > ~/.config/fish/completions/r44.fish");
    } else {
        output::dimmed("  r44 completions <bash|zsh|fish>");
    }
    println!();

    output::success("setup complete");
    output::dimmed(&format!("  r44 --profile {profile_name} doctor"));
    output::dimmed(&format!("  r44 --profile {profile_name} markets list"));
    output::dimmed(&format!("  r44 --profile {profile_name} shell"));
    println!();

    Ok(())
}

fn prompt(message: &str) -> Result<String> {
    eprint!("{message}");
    io::stderr().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(value)
}
