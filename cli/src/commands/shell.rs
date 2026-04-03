use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Result;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};

use crate::config::Config;
use crate::output;
use crate::runtime::{self, InvocationDefaults, InvocationSource};
use crate::sessions::SessionLogger;

const COMMANDS: &[(&str, &[&str])] = &[
    ("setup", &[]),
    ("shell", &[]),
    ("doctor", &[]),
    ("login", &["solana", "siwe", "status", "logout"]),
    ("markets", &["list", "get", "orderbook", "trades"]),
    ("orders", &["list", "place", "cancel", "cancel-all", "get"]),
    ("positions", &["list", "get", "claim"]),
    ("agents", &["list", "get", "public", "create", "update", "execute"]),
    ("edge-scanner", &["signals", "curve"]),
    ("decisions", &["list", "get", "create"]),
    ("leaderboard", &["top", "rank"]),
    ("activity", &["list"]),
    ("wallet", &["balance", "deposit-address"]),
    ("config", &["show", "set-url", "set-token", "path", "reset"]),
    ("profile", &["list", "use", "show"]),
    ("workflow", &["list", "run", "validate"]),
    ("session", &["export", "replay"]),
];

struct ShellHelper {
    aliases: Vec<String>,
}

impl Completer for ShellHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let input = &line[..pos];
        let parts: Vec<&str> = input.split_whitespace().collect();
        let trailing_space = input.ends_with(' ');

        if parts.is_empty() || (parts.len() == 1 && !trailing_space) {
            let prefix = parts.first().copied().unwrap_or("");
            let start = input.len() - prefix.len();
            let mut candidates: Vec<Pair> = COMMANDS
                .iter()
                .map(|(cmd, _)| *cmd)
                .chain(self.aliases.iter().map(|s| s.as_str()))
                .chain(["help", "exit", "quit"].iter().copied())
                .filter(|cmd| cmd.starts_with(prefix))
                .map(|cmd| Pair {
                    display: cmd.to_string(),
                    replacement: cmd.to_string(),
                })
                .collect();
            candidates.sort_by(|a, b| a.display.cmp(&b.display));
            candidates.dedup_by(|a, b| a.display == b.display);
            return Ok((start, candidates));
        }

        if parts.len() == 1 && trailing_space || parts.len() == 2 && !trailing_space {
            let cmd = parts[0];
            let sub_prefix = if parts.len() == 2 && !trailing_space {
                parts[1]
            } else {
                ""
            };
            let start = pos - sub_prefix.len();

            if let Some((_, subs)) = COMMANDS.iter().find(|(c, _)| *c == cmd) {
                let candidates: Vec<Pair> = subs
                    .iter()
                    .filter(|s| s.starts_with(sub_prefix))
                    .map(|s| Pair {
                        display: s.to_string(),
                        replacement: s.to_string(),
                    })
                    .collect();
                return Ok((start, candidates));
            }
        }

        Ok((pos, vec![]))
    }
}

impl Hinter for ShellHelper {
    type Hint = String;
}
impl Highlighter for ShellHelper {}
impl Validator for ShellHelper {}
impl Helper for ShellHelper {}

pub async fn run(config: Arc<Mutex<Config>>, defaults: InvocationDefaults) -> Result<()> {
    output::banner();
    output::dimmed("  interactive shell");
    output::dimmed("  type 'help' for command help, 'exit' to quit");
    println!();

    let aliases = {
        let cfg = config.lock().expect("config lock");
        cfg.aliases.keys().cloned().collect()
    };
    let helper = ShellHelper { aliases };
    let mut editor = Editor::new()?;
    editor.set_helper(Some(helper));
    let history_path = Config::history_path()?;
    let logger = {
        let config = config.lock().expect("config lock");
        SessionLogger::new(&config)?
    };

    let _ = editor.load_history(&history_path);

    loop {
        if crate::is_interrupted() {
            break;
        }

        let prompt = shell_prompt(&config, &defaults);
        match editor.readline(&prompt) {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let _ = editor.add_history_entry(line);

                if matches!(line, "exit" | "quit") {
                    break;
                }

                if line == "help"
                    || line == "?"
                    || line.starts_with("help ")
                    || line.starts_with("? ")
                {
                    let args = line
                        .split_whitespace()
                        .skip(1)
                        .map(str::to_string)
                        .collect::<Vec<_>>();
                    print!("{}", runtime::render_help(&args));
                    continue;
                }

                let started_at = Instant::now();
                let command = {
                    let config = config.lock().expect("config lock");
                    runtime::parse_shell_line(line, &config)
                };

                let result = match command {
                    Ok(cli) => {
                        let profile_name = {
                            let config = config.lock().expect("config lock");
                            config
                                .selected_profile_name(
                                    cli.profile.as_deref(),
                                    defaults.profile.as_deref(),
                                )
                                .unwrap_or_else(|_| {
                                    defaults
                                        .profile
                                        .clone()
                                        .unwrap_or_else(|| config.active_profile.clone())
                                })
                        };
                        let result = runtime::execute(
                            cli,
                            config.clone(),
                            defaults.clone(),
                            InvocationSource::Shell,
                        )
                        .await;
                        let _ = logger.append(
                            &profile_name,
                            line,
                            if result.is_ok() { 0 } else { 1 },
                            started_at.elapsed().as_millis(),
                        );
                        result
                    }
                    Err(error) => Err(error),
                };

                if let Err(error) = result {
                    output::error(&format!("{error:#}"));
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(error) => {
                output::error(&format!("readline: {error}"));
                break;
            }
        }
    }

    let _ = std::fs::create_dir_all(history_path.parent().unwrap_or(std::path::Path::new(".")));
    let _ = editor.save_history(&history_path);
    output::dimmed("goodbye");
    Ok(())
}

fn shell_prompt(config: &Arc<Mutex<Config>>, defaults: &InvocationDefaults) -> String {
    let profile = {
        let config = config.lock().expect("config lock");
        config
            .selected_profile_name(defaults.profile.as_deref(), None)
            .unwrap_or_else(|_| config.active_profile.clone())
    };
    format!("r44({profile})> ")
}
