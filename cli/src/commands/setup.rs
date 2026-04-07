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
    let (shell_name, install_path, install_hint) = if shell.contains("zsh") {
        ("zsh", Some(dirs::home_dir().map(|h| h.join(".zfunc/_r44"))), "r44 completions zsh > ~/.zfunc/_r44")
    } else if shell.contains("bash") {
        let target = dirs::home_dir().map(|h| h.join(".local/share/bash-completion/completions/r44"));
        ("bash", Some(target), "r44 completions bash > ~/.local/share/bash-completion/completions/r44")
    } else if shell.contains("fish") {
        let target = dirs::config_dir().map(|c| c.join("fish/completions/r44.fish"));
        ("fish", Some(target), "r44 completions fish > ~/.config/fish/completions/r44.fish")
    } else {
        ("", None, "r44 completions <bash|zsh|fish>")
    };

    let installed = if let Some(Some(target)) = install_path {
        let choice = prompt(&format!("Install {shell_name} completions? [Y/n]: "))?;
        if choice.trim().is_empty() || choice.trim().eq_ignore_ascii_case("y") {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let comp_output = std::process::Command::new(std::env::current_exe()?)
                .args(["completions", shell_name])
                .output();
            match comp_output {
                Ok(result) if result.status.success() => {
                    std::fs::write(&target, &result.stdout)?;
                    output::success(&format!("completions installed → {}", target.display()));
                    true
                }
                _ => {
                    output::warn("could not generate completions, install manually:");
                    output::dimmed(&format!("  {install_hint}"));
                    false
                }
            }
        } else {
            output::dimmed(&format!("  {install_hint}"));
            false
        }
    } else {
        output::dimmed(&format!("  {install_hint}"));
        false
    };

    if installed && shell.contains("zsh") {
        output::dimmed("  ensure ~/.zfunc is in your fpath (add: fpath=(~/.zfunc $fpath) to .zshrc)");
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
