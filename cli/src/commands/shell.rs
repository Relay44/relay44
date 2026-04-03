use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use crate::config::Config;
use crate::output;
use crate::runtime::{self, InvocationDefaults, InvocationSource};
use crate::sessions::SessionLogger;

pub async fn run(config: Arc<Mutex<Config>>, defaults: InvocationDefaults) -> Result<()> {
    output::banner();
    output::dimmed("  interactive shell");
    output::dimmed("  type 'help' for command help, 'exit' to quit");
    println!();

    let mut editor = DefaultEditor::new()?;
    let history_path = Config::history_path()?;
    let logger = {
        let config = config.lock().unwrap();
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
                    let config = config.lock().unwrap();
                    runtime::parse_shell_line(line, &config)
                };

                let result = match command {
                    Ok(cli) => {
                        let profile_name = {
                            let config = config.lock().unwrap();
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
        let config = config.lock().unwrap();
        config
            .selected_profile_name(defaults.profile.as_deref(), None)
            .unwrap_or_else(|_| config.active_profile.clone())
    };
    format!("r44({profile})> ")
}
