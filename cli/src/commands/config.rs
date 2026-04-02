use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::Subcommand;

use crate::config::Config;
use crate::output;

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Show current configuration
    #[command(
        long_about = "Display the current CLI configuration including auth status.\n\n\
                      Example:\n  r44 config show"
    )]
    Show,

    /// Set API URL
    #[command(
        long_about = "Override the default API URL. Useful for development or self-hosted instances.\n\n\
                      Example:\n  r44 config set-url http://localhost:3000/v1"
    )]
    SetUrl {
        /// API base URL (e.g. https://relay44-api.onrender.com/v1)
        url: String,
    },

    /// Set access token (escape hatch for external auth)
    #[command(
        long_about = "Manually set a JWT access token. Use this if you obtained a token \
                      outside the CLI (e.g. from the web app).\n\n\
                      Prefer `r44 login solana` for normal authentication.\n\n\
                      Example:\n  r44 config set-token eyJhbG..."
    )]
    SetToken {
        /// JWT access token
        token: String,
    },

    /// Show config file location
    #[command(long_about = "Print the path to the config file.\n\n\
                            Example:\n  r44 config path")]
    Path,

    /// Reset config to defaults
    #[command(long_about = "Reset all configuration to defaults and clear stored credentials.\n\n\
                            Example:\n  r44 config reset")]
    Reset {
        /// Skip confirmation
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

pub fn run(cmd: ConfigCmd, config: Arc<Mutex<Config>>) -> Result<()> {
    match cmd {
        ConfigCmd::Show => {
            let cfg = config.lock().unwrap();
            output::print_detail(&[
                ("API URL", cfg.api_url.clone()),
                (
                    "Auth",
                    if cfg.access_token.is_some() {
                        "authenticated".into()
                    } else {
                        "not configured".into()
                    },
                ),
                (
                    "Wallet",
                    cfg.wallet.clone().unwrap_or_else(|| "-".into()),
                ),
                ("Config", config_path()),
            ]);
        }
        ConfigCmd::SetUrl { url } => {
            if !url.starts_with("http://") && !url.starts_with("https://") {
                output::warn("URL should start with http:// or https://");
            }
            let mut cfg = config.lock().unwrap();
            cfg.api_url = url.clone();
            cfg.save()?;
            output::success(&format!("API URL → {url}"));
        }
        ConfigCmd::SetToken { token } => {
            let mut cfg = config.lock().unwrap();
            cfg.access_token = Some(token);
            cfg.save()?;
            output::success("Access token saved");
        }
        ConfigCmd::Path => {
            println!("{}", config_path());
        }
        ConfigCmd::Reset { yes } => {
            if !yes && !output::confirm("Reset all config and clear credentials?") {
                output::dimmed("cancelled");
                return Ok(());
            }
            let default = Config::default();
            default.save()?;
            *config.lock().unwrap() = default;
            output::success("Config reset to defaults");
        }
    }
    Ok(())
}

fn config_path() -> String {
    dirs::config_dir()
        .unwrap_or_default()
        .join("r44")
        .join("config.json")
        .display()
        .to_string()
}
