use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::Subcommand;

use crate::config::Config;
use crate::output::{self, Format};

#[derive(Subcommand, Clone)]
pub enum ConfigCmd {
    /// Show the current configuration
    Show,
    /// Set the API URL for the selected profile
    SetUrl { url: String },
    /// Set the access token for the selected profile
    SetToken { token: String },
    /// Show the config file location
    Path,
    /// Reset the entire config to defaults
    Reset {
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

pub fn run(
    cmd: ConfigCmd,
    config: Arc<Mutex<Config>>,
    profile_name: &str,
    format: Format,
) -> Result<()> {
    match cmd {
        ConfigCmd::Show => {
            let config = config.lock().unwrap();
            let profile = config.profile(profile_name).cloned().unwrap_or_default();

            match format {
                Format::Json => output::print_json(&serde_json::json!({
                    "activeProfile": config.active_profile,
                    "selectedProfile": profile_name,
                    "profile": {
                        "apiUrl": profile.api_url,
                        "authenticated": profile.access_token.is_some(),
                        "wallet": profile.wallet,
                        "output": profile.output.map(|value| value.to_string()).unwrap_or_else(|| "table".into()),
                    },
                    "workflows": config.workflows.len(),
                    "hooks": config.hooks.len(),
                    "sessionLog": {
                        "enabled": config.session_log.enabled,
                        "path": config.session_log_path()?.display().to_string(),
                    }
                })),
                Format::Table => output::print_detail(&[
                    ("Active profile", config.active_profile.clone()),
                    ("Selected profile", profile_name.into()),
                    ("API URL", profile.api_url),
                    (
                        "Auth",
                        if profile.access_token.is_some() {
                            "configured".into()
                        } else {
                            "not configured".into()
                        },
                    ),
                    ("Wallet", profile.wallet.unwrap_or_else(|| "—".into())),
                    (
                        "Output",
                        profile
                            .output
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "table".into()),
                    ),
                    ("Workflows", config.workflows.len().to_string()),
                    ("Hooks", config.hooks.len().to_string()),
                    ("Config path", Config::path()?.display().to_string()),
                ]),
            }
        }
        ConfigCmd::SetUrl { url } => {
            let mut config = config.lock().unwrap();
            if !url.starts_with("http://") && !url.starts_with("https://") {
                output::warn("URL should start with http:// or https://");
            }
            config.ensure_profile(profile_name).api_url = url.clone();
            config.save()?;
            output::success(&format!("{profile_name} API URL → {url}"));
        }
        ConfigCmd::SetToken { token } => {
            let mut config = config.lock().unwrap();
            config.ensure_profile(profile_name).access_token = Some(token);
            config.save()?;
            output::success(&format!("access token saved for profile '{profile_name}'"));
        }
        ConfigCmd::Path => {
            println!("{}", Config::path()?.display());
        }
        ConfigCmd::Reset { yes } => {
            if !yes && !output::confirm("Reset all CLI config and profiles?") {
                output::dimmed("cancelled");
                return Ok(());
            }
            let default = Config::default();
            default.save()?;
            *config.lock().unwrap() = default;
            output::success("config reset to defaults");
        }
    }

    Ok(())
}
