use std::process::Stdio;

use anyhow::{anyhow, Result};
use tokio::process::Command;

use crate::config::{Config, HookStage};
use crate::output;

#[derive(Clone)]
pub struct HookContext {
    pub command_path: String,
    pub profile: String,
    pub api_url: String,
    pub source: &'static str,
}

pub async fn run_pre_hooks(config: &Config, context: &HookContext) -> Result<()> {
    run_hooks(config, HookStage::Pre, context, None).await
}

pub async fn run_post_hooks(
    config: &Config,
    context: &HookContext,
    exit_status: i32,
    duration_ms: u128,
) -> Result<()> {
    run_hooks(
        config,
        HookStage::Post,
        context,
        Some((exit_status, duration_ms)),
    )
    .await
}

async fn run_hooks(
    config: &Config,
    stage: HookStage,
    context: &HookContext,
    result: Option<(i32, u128)>,
) -> Result<()> {
    for hook in config
        .hooks
        .iter()
        .filter(|hook| hook.enabled && hook.stage == stage && hook.command == context.command_path)
    {
        let status = run_hook_command(hook.run.as_str(), context, stage, result).await?;
        if status.success() {
            continue;
        }

        let code = status.code().unwrap_or(1);
        match stage {
            HookStage::Pre if hook.required => {
                return Err(anyhow!(
                    "pre-hook failed for '{}': {} (exit {code})",
                    context.command_path,
                    hook.run
                ));
            }
            HookStage::Pre => {
                output::warn(&format!(
                    "pre-hook failed for '{}': {} (exit {code})",
                    context.command_path, hook.run
                ));
            }
            HookStage::Post => {
                output::warn(&format!(
                    "post-hook failed for '{}': {} (exit {code})",
                    context.command_path, hook.run
                ));
            }
        }
    }

    Ok(())
}

async fn run_hook_command(
    script: &str,
    context: &HookContext,
    stage: HookStage,
    result: Option<(i32, u128)>,
) -> Result<std::process::ExitStatus> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut command = Command::new(shell);
    command
        .arg("-lc")
        .arg(script)
        .stdin(Stdio::null())
        .env("R44_HOOK_STAGE", stage_name(stage))
        .env("R44_COMMAND_PATH", &context.command_path)
        .env("R44_PROFILE", &context.profile)
        .env("R44_API_URL", &context.api_url)
        .env("R44_SOURCE", context.source);

    if let Some((exit_status, duration_ms)) = result {
        command
            .env("R44_EXIT_STATUS", exit_status.to_string())
            .env("R44_DURATION_MS", duration_ms.to_string());
    }

    Ok(command.status().await?)
}

fn stage_name(stage: HookStage) -> &'static str {
    match stage {
        HookStage::Pre => "pre",
        HookStage::Post => "post",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Hook, Profile, SessionLogConfig};
    use std::collections::BTreeMap;

    #[tokio::test]
    async fn optional_pre_hook_does_not_block() {
        let config = Config {
            active_profile: "default".into(),
            profiles: BTreeMap::from([("default".into(), Profile::default())]),
            workflows: BTreeMap::new(),
            hooks: vec![Hook {
                command: "markets list".into(),
                run: "exit 1".into(),
                stage: HookStage::Pre,
                required: false,
                enabled: true,
            }],
            session_log: SessionLogConfig::default(),
            aliases: BTreeMap::new(),
        };

        let result = run_pre_hooks(
            &config,
            &HookContext {
                command_path: "markets list".into(),
                profile: "default".into(),
                api_url: "https://relay44-api.onrender.com/v1".into(),
                source: "cli",
            },
        )
        .await;

        assert!(result.is_ok());
    }
}
