use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::Subcommand;
use tabled::Tabled;

use crate::config::Config;
use crate::output::{self, Format};
use crate::runtime::{self, EffectiveInvocation, InvocationDefaults, InvocationSource};
use crate::sessions::{read_entries, SessionEntry};

#[derive(Subcommand, Clone)]
pub enum SessionCmd {
    /// Export recent shell session entries
    Export {
        #[arg(long, default_value = "100")]
        limit: usize,
    },
    /// Replay a recorded session file
    Replay {
        file: PathBuf,
        #[arg(long)]
        execute: bool,
    },
}

#[derive(Tabled)]
struct SessionRow {
    #[tabled(rename = "Timestamp")]
    timestamp: String,
    #[tabled(rename = "Profile")]
    profile: String,
    #[tabled(rename = "Command")]
    command: String,
    #[tabled(rename = "Exit")]
    exit_status: String,
    #[tabled(rename = "Duration")]
    duration: String,
}

pub async fn run(
    cmd: SessionCmd,
    config: Arc<Mutex<Config>>,
    effective: &EffectiveInvocation,
    defaults: InvocationDefaults,
    _source: InvocationSource,
) -> Result<()> {
    match cmd {
        SessionCmd::Export { limit } => {
            let path = config.lock().unwrap().session_log_path()?;
            let mut entries = if path.exists() {
                read_entries(&path)?
            } else {
                Vec::new()
            };
            if entries.len() > limit {
                entries = entries.split_off(entries.len() - limit);
            }
            render_entries(&entries, effective.output);
        }
        SessionCmd::Replay { file, execute } => {
            let entries = read_entries(&file)?;
            if !execute {
                render_entries(&entries, effective.output);
                output::dimmed("dry run only. pass --execute to rerun these commands");
                return Ok(());
            }

            for entry in entries {
                output::dimmed(&format!("replay: {}", entry.command));
                let parsed = {
                    let config = config.lock().unwrap();
                    runtime::parse_shell_line(&entry.command, &config)?
                };
                runtime::execute(
                    parsed,
                    config.clone(),
                    defaults.clone(),
                    InvocationSource::SessionReplay,
                )
                .await?;
            }
        }
    }

    Ok(())
}

fn render_entries(entries: &[SessionEntry], format: Format) {
    match format {
        Format::Json => output::print_json(&serde_json::json!({
            "sessions": entries,
        })),
        Format::Table => {
            let rows = entries
                .iter()
                .map(|entry| SessionRow {
                    timestamp: entry.timestamp.clone(),
                    profile: entry.profile.clone(),
                    command: entry.command.clone(),
                    exit_status: entry.exit_status.to_string(),
                    duration: format!("{}ms", entry.duration_ms),
                })
                .collect::<Vec<_>>();
            output::print_tabled(&rows);
        }
    }
}
