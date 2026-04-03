use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{anyhow, Result};
use async_recursion::async_recursion;
use clap::{error::ErrorKind, CommandFactory, Parser};

use crate::client::Client;
use crate::commands;
use crate::config::Config;
use crate::hooks::{self, HookContext};
use crate::output::{self, Format};
use crate::{Cli, Commands};

#[derive(Clone, Debug, Default)]
pub struct InvocationDefaults {
    pub profile: Option<String>,
    pub api_url: Option<String>,
    pub output: Option<Format>,
    pub quiet: bool,
    pub verbose: bool,
    pub timeout_secs: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvocationSource {
    Cli,
    Shell,
    Workflow,
    SessionReplay,
}

pub fn parse_shell_line(line: &str, config: &Config) -> Result<Cli> {
    let line = expand_alias(line, &config.aliases);
    let args = shlex::split(&line).ok_or_else(|| anyhow!("invalid shell quoting"))?;
    let argv = std::iter::once("r44".to_string())
        .chain(args.clone())
        .collect::<Vec<_>>();
    Cli::try_parse_from(argv).map_err(|error| {
        let msg = error.to_string();
        if !msg.contains("a similar subcommand exists") {
            if let Some(suggestion) =
                suggest_command(args.first().map(|s| s.as_str()).unwrap_or(""))
            {
                return anyhow!("{msg}\n  tip: did you mean '{suggestion}'?");
            }
        }
        anyhow!(msg)
    })
}

pub fn render_help(args: &[String]) -> String {
    let argv = std::iter::once("r44".to_string())
        .chain(args.iter().cloned())
        .chain(std::iter::once("--help".to_string()))
        .collect::<Vec<_>>();

    match Cli::try_parse_from(argv) {
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error.to_string()
        }
        Err(error) => error.to_string(),
        Ok(_) => {
            let mut buffer = Vec::new();
            let mut command = Cli::command();
            let _ = command.write_long_help(&mut buffer);
            String::from_utf8_lossy(&buffer).into_owned()
        }
    }
}

#[async_recursion]
pub async fn execute(
    cli: Cli,
    config: Arc<Mutex<Config>>,
    defaults: InvocationDefaults,
    source: InvocationSource,
) -> Result<()> {
    if source != InvocationSource::Cli && matches!(cli.command, Commands::Shell) {
        output::warn("already in shell mode");
        return Ok(());
    }

    if source == InvocationSource::Workflow
        && matches!(
            cli.command,
            Commands::Workflow {
                cmd: commands::workflow::WorkflowCmd::Run { .. }
            }
        )
    {
        return Err(anyhow!("workflow nesting is not supported"));
    }

    if source == InvocationSource::SessionReplay
        && matches!(
            cli.command,
            Commands::Session {
                cmd: commands::session::SessionCmd::Replay { .. }
            }
        )
    {
        return Err(anyhow!(
            "session replay cannot recursively replay another session"
        ));
    }

    if let Commands::Completions { shell } = &cli.command {
        let mut command = Cli::command();
        clap_complete::generate(*shell, &mut command, "r44", &mut std::io::stdout());
        return Ok(());
    }

    let effective = EffectiveInvocation::resolve(&cli, &config, &defaults)?;
    let previous_quiet = output::is_quiet();
    let previous_verbose = output::is_verbose();
    output::set_quiet(effective.quiet);
    output::set_verbose(effective.verbose);

    let hook_context = HookContext {
        command_path: cli.command_path(),
        profile: effective.profile.clone(),
        api_url: effective.api_url.clone(),
        source: source_name(source),
    };

    let snapshot = config.lock().expect("config lock").clone();
    hooks::run_pre_hooks(&snapshot, &hook_context).await?;

    let started_at = Instant::now();
    let client = Client::new(
        &effective.api_url,
        config.clone(),
        effective.profile.clone(),
        effective.timeout_secs,
    )?;
    let result = execute_command(
        cli,
        config.clone(),
        &client,
        &effective,
        defaults.clone(),
        source,
    )
    .await;
    let exit_status = if result.is_ok() { 0 } else { 1 };

    let snapshot = config.lock().expect("config lock").clone();
    let post_hook_result = hooks::run_post_hooks(
        &snapshot,
        &hook_context,
        exit_status,
        started_at.elapsed().as_millis(),
    )
    .await;

    output::set_quiet(previous_quiet);
    output::set_verbose(previous_verbose);

    post_hook_result?;
    result
}

const KNOWN_COMMANDS: &[&str] = &[
    "setup",
    "shell",
    "doctor",
    "login",
    "markets",
    "orders",
    "positions",
    "agents",
    "edge-scanner",
    "decisions",
    "leaderboard",
    "activity",
    "wallet",
    "config",
    "profile",
    "workflow",
    "session",
    "completions",
];

fn suggest_command(input: &str) -> Option<&'static str> {
    if input.is_empty() || KNOWN_COMMANDS.contains(&input) {
        return None;
    }
    KNOWN_COMMANDS
        .iter()
        .map(|cmd| (*cmd, strsim::jaro_winkler(input, cmd)))
        .filter(|(_, score)| *score > 0.75)
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(cmd, _)| cmd)
}

fn expand_alias(line: &str, aliases: &std::collections::BTreeMap<String, String>) -> String {
    let Some(parts) = shlex::split(line) else {
        return line.to_string();
    };
    let Some(first) = parts.first() else {
        return line.to_string();
    };
    let Some(alias) = aliases.get(first) else {
        return line.to_string();
    };

    let suffix = parts
        .iter()
        .skip(1)
        .map(|part| shell_quote(part))
        .collect::<Vec<_>>()
        .join(" ");

    if suffix.is_empty() {
        alias.clone()
    } else {
        format!("{alias} {suffix}")
    }
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".into();
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./:".contains(ch))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

async fn execute_command(
    cli: Cli,
    config: Arc<Mutex<Config>>,
    client: &Client,
    effective: &EffectiveInvocation,
    defaults: InvocationDefaults,
    source: InvocationSource,
) -> Result<()> {
    match cli.command {
        Commands::Setup => {
            commands::setup::run(config, &effective.profile, &effective.api_url).await
        }
        Commands::Shell => {
            let shell_defaults = InvocationDefaults {
                profile: Some(effective.profile.clone()),
                api_url: Some(effective.api_url.clone()),
                output: Some(effective.output),
                quiet: effective.quiet,
                verbose: effective.verbose,
                timeout_secs: Some(effective.timeout_secs),
            };
            commands::shell::run(config, shell_defaults).await
        }
        Commands::Doctor => {
            commands::doctor::run(
                config,
                &effective.profile,
                &effective.api_url,
                effective.output,
            )
            .await
        }
        Commands::Login { cmd } => {
            commands::login::run(
                cmd,
                config,
                &effective.profile,
                &effective.api_url,
                effective.output,
            )
            .await
        }
        Commands::Markets { cmd } => commands::markets::run(cmd, client, effective.output).await,
        Commands::Orders { cmd } => commands::orders::run(cmd, client, effective.output).await,
        Commands::Positions { cmd } => {
            commands::positions::run(cmd, client, effective.output).await
        }
        Commands::Agents { cmd } => commands::agents::run(cmd, client, effective.output).await,
        Commands::EdgeScanner { cmd } => {
            commands::edge_scanner::run(cmd, client, effective.output).await
        }
        Commands::Decisions { cmd } => {
            commands::decisions::run(cmd, client, effective.output).await
        }
        Commands::Leaderboard { cmd } => {
            commands::leaderboard::run(cmd, client, effective.output).await
        }
        Commands::Activity { cmd } => {
            commands::activity::run(cmd, client, effective.output).await
        }
        Commands::Wallet { cmd } => commands::wallet::run(cmd, client, effective.output).await,
        Commands::Config { cmd } => {
            commands::config::run(cmd, config, &effective.profile, effective.output)
        }
        Commands::Profile { cmd } => commands::profile::run(cmd, config, effective.output),
        Commands::Workflow { cmd } => {
            commands::workflow::run(cmd, config, effective, defaults, source).await
        }
        Commands::Session { cmd } => {
            commands::session::run(cmd, config, effective, defaults, source).await
        }
        Commands::Completions { .. } => unreachable!(),
    }
}

#[derive(Clone, Debug)]
pub struct EffectiveInvocation {
    pub profile: String,
    pub api_url: String,
    pub output: Format,
    pub quiet: bool,
    pub verbose: bool,
    pub timeout_secs: u64,
}

impl EffectiveInvocation {
    fn resolve(
        cli: &Cli,
        config: &Arc<Mutex<Config>>,
        defaults: &InvocationDefaults,
    ) -> Result<Self> {
        let config = config.lock().expect("config lock");
        let requested_profile = cli
            .profile
            .as_deref()
            .or(defaults.profile.as_deref())
            .unwrap_or(&config.active_profile)
            .to_string();
        let profile = match config
            .selected_profile_name(cli.profile.as_deref(), defaults.profile.as_deref())
        {
            Ok(profile) => profile,
            Err(_) if can_create_profile(&cli.command) => requested_profile.clone(),
            Err(error) => return Err(error),
        };
        let profile_config = config.profile(&profile).cloned().unwrap_or_default();

        Ok(Self {
            profile,
            api_url: cli
                .api_url
                .clone()
                .or_else(|| defaults.api_url.clone())
                .unwrap_or(profile_config.api_url),
            output: cli
                .output
                .or(defaults.output)
                .or(profile_config.output)
                .unwrap_or(Format::Table),
            quiet: cli.quiet || defaults.quiet,
            verbose: cli.verbose || defaults.verbose,
            timeout_secs: defaults.timeout_secs.unwrap_or(cli.timeout),
        })
    }
}

fn can_create_profile(command: &Commands) -> bool {
    matches!(
        command,
        Commands::Setup
            | Commands::Login { .. }
            | Commands::Config {
                cmd: commands::config::ConfigCmd::SetUrl { .. }
            }
            | Commands::Config {
                cmd: commands::config::ConfigCmd::SetToken { .. }
            }
    )
}

fn source_name(source: InvocationSource) -> &'static str {
    match source {
        InvocationSource::Cli => "cli",
        InvocationSource::Shell => "shell",
        InvocationSource::Workflow => "workflow",
        InvocationSource::SessionReplay => "session-replay",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Profile};
    use std::collections::BTreeMap;

    #[test]
    fn expands_alias_before_parse() {
        let mut config = Config::default();
        config
            .aliases
            .insert("ob".into(), "markets orderbook".into());

        let cli = parse_shell_line("ob market-1", &config).unwrap();
        assert_eq!(cli.command_path(), "markets orderbook");
    }

    #[test]
    fn suggests_similar_commands() {
        assert_eq!(suggest_command("markts"), Some("markets"));
        assert_eq!(suggest_command("ordrs"), Some("orders"));
        assert_eq!(suggest_command("wal"), Some("wallet"));
        assert_eq!(suggest_command("agnt"), Some("agents"));
        assert_eq!(suggest_command("markets"), None);
        assert_eq!(suggest_command("zzzzz"), None);
        assert_eq!(suggest_command(""), None);
    }

    #[test]
    fn resolves_shell_defaults() {
        let config = Arc::new(Mutex::new(Config {
            active_profile: "prod".into(),
            profiles: BTreeMap::from([(
                "prod".into(),
                Profile {
                    api_url: "https://prod.example.com/v1".into(),
                    access_token: None,
                    refresh_token: None,
                    wallet: None,
                    output: Some(Format::Json),
                },
            )]),
            ..Config::default()
        }));

        let cli = Cli::try_parse_from(["r44", "markets", "list"]).unwrap();
        let effective =
            EffectiveInvocation::resolve(&cli, &config, &InvocationDefaults::default()).unwrap();
        assert_eq!(effective.profile, "prod");
        assert_eq!(effective.api_url, "https://prod.example.com/v1");
        assert_eq!(effective.output, Format::Json);
    }
}
