use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::Subcommand;
use tabled::Tabled;

use crate::config::Config;
use crate::output::{self, Format};

#[derive(Subcommand, Clone)]
pub enum ProfileCmd {
    /// List configured profiles
    List,
    /// Switch the active profile
    Use { name: String },
    /// Show one profile or the active profile
    Show { name: Option<String> },
}

#[derive(Tabled)]
struct ProfileRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Active")]
    active: String,
    #[tabled(rename = "API")]
    api: String,
    #[tabled(rename = "Auth")]
    auth: String,
    #[tabled(rename = "Wallet")]
    wallet: String,
    #[tabled(rename = "Output")]
    output: String,
}

pub fn run(cmd: ProfileCmd, config: Arc<Mutex<Config>>, format: Format) -> Result<()> {
    match cmd {
        ProfileCmd::List => {
            let config = config.lock().unwrap();
            let rows = config
                .profiles
                .iter()
                .map(|(name, profile)| ProfileRow {
                    name: name.clone(),
                    active: if name == &config.active_profile {
                        "yes".into()
                    } else {
                        String::new()
                    },
                    api: profile.api_url.clone(),
                    auth: if profile.access_token.is_some() {
                        "configured".into()
                    } else {
                        "—".into()
                    },
                    wallet: profile.wallet.clone().unwrap_or_else(|| "—".into()),
                    output: profile
                        .output
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "table".into()),
                })
                .collect::<Vec<_>>();

            match format {
                Format::Json => output::print_json(&serde_json::json!({
                    "active": config.active_profile,
                    "profiles": rows.iter().map(|row| serde_json::json!({
                        "name": row.name,
                        "active": row.active == "yes",
                        "apiUrl": row.api,
                        "auth": row.auth,
                        "wallet": row.wallet,
                        "output": row.output,
                    })).collect::<Vec<_>>()
                })),
                Format::Table => output::print_tabled(&rows),
            }
        }
        ProfileCmd::Use { name } => {
            let mut config = config.lock().unwrap();
            config.set_active_profile(&name)?;
            config.save()?;
            output::success(&format!("active profile → {name}"));
        }
        ProfileCmd::Show { name } => {
            let config = config.lock().unwrap();
            let selected = config.selected_profile_name(name.as_deref(), None)?;
            let profile = config.profile(&selected).unwrap();

            match format {
                Format::Json => output::print_json(&serde_json::json!({
                    "name": selected,
                    "active": selected == config.active_profile,
                    "apiUrl": profile.api_url,
                    "authenticated": profile.access_token.is_some(),
                    "wallet": profile.wallet,
                    "output": profile.output.map(|value| value.to_string()).unwrap_or_else(|| "table".into()),
                })),
                Format::Table => output::print_detail(&[
                    ("Name", selected.clone()),
                    (
                        "Active",
                        if selected == config.active_profile {
                            "yes".into()
                        } else {
                            "no".into()
                        },
                    ),
                    ("API URL", profile.api_url.clone()),
                    (
                        "Auth",
                        if profile.access_token.is_some() {
                            "configured".into()
                        } else {
                            "not configured".into()
                        },
                    ),
                    (
                        "Wallet",
                        profile.wallet.clone().unwrap_or_else(|| "—".into()),
                    ),
                    (
                        "Output",
                        profile
                            .output
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "table".into()),
                    ),
                ]),
            }
        }
    }

    Ok(())
}
