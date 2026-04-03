use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail, Result};
use clap::Subcommand;
use tabled::Tabled;

use crate::config::{Config, Workflow};
use crate::output::{self, Format};
use crate::runtime::{self, EffectiveInvocation, InvocationDefaults, InvocationSource};

#[derive(Subcommand, Clone)]
pub enum WorkflowCmd {
    /// List configured workflows
    List,
    /// Run a workflow
    Run {
        name: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Validate one workflow or all workflows
    Validate { name: Option<String> },
}

#[derive(Tabled)]
struct WorkflowRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Steps")]
    steps: String,
    #[tabled(rename = "Description")]
    description: String,
}

pub async fn run(
    cmd: WorkflowCmd,
    config: Arc<Mutex<Config>>,
    effective: &EffectiveInvocation,
    defaults: InvocationDefaults,
    _source: InvocationSource,
) -> Result<()> {
    match cmd {
        WorkflowCmd::List => {
            let config = config.lock().unwrap();
            let rows = config
                .workflows
                .iter()
                .map(|(name, workflow)| WorkflowRow {
                    name: name.clone(),
                    steps: workflow.steps.len().to_string(),
                    description: workflow.description.clone().unwrap_or_else(|| "—".into()),
                })
                .collect::<Vec<_>>();

            match effective.output {
                Format::Json => output::print_json(&serde_json::json!({
                    "workflows": config.workflows.iter().map(|(name, workflow)| serde_json::json!({
                        "name": name,
                        "description": workflow.description,
                        "steps": workflow.steps,
                    })).collect::<Vec<_>>()
                })),
                Format::Table => output::print_tabled(&rows),
            }
        }
        WorkflowCmd::Validate { name } => {
            let workflows = selected_workflows(&config.lock().unwrap(), name.as_deref())?;
            let mut results = Vec::new();
            for (name, workflow) in workflows {
                let rendered = render_workflow(&workflow, &sample_args(&workflow), effective)?;
                for step in &rendered {
                    let cli = runtime::parse_shell_line(step, &config.lock().unwrap())?;
                    if matches!(
                        cli.command,
                        crate::Commands::Workflow {
                            cmd: WorkflowCmd::Run { .. }
                        }
                    ) {
                        bail!("workflow '{name}' contains nested workflow execution");
                    }
                }
                results.push(serde_json::json!({
                    "name": name,
                    "status": "valid",
                    "steps": rendered,
                }));
            }

            match effective.output {
                Format::Json => output::print_json(&serde_json::json!({ "results": results })),
                Format::Table => output::success("workflow validation passed"),
            }
        }
        WorkflowCmd::Run {
            name,
            dry_run,
            args,
        } => {
            let workflow = {
                let config = config.lock().unwrap();
                config
                    .workflows
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| anyhow!("workflow '{name}' not found"))?
            };

            let steps = render_workflow(&workflow, &args, effective)?;
            if dry_run {
                match effective.output {
                    Format::Json => output::print_json(&serde_json::json!({
                        "name": name,
                        "steps": steps,
                    })),
                    Format::Table => {
                        for (index, step) in steps.iter().enumerate() {
                            println!("{:>2}. {}", index + 1, step);
                        }
                    }
                }
                return Ok(());
            }

            for step in steps {
                output::dimmed(&format!("workflow {name}: {step}"));
                let parsed = {
                    let config = config.lock().unwrap();
                    runtime::parse_shell_line(&step, &config)?
                };
                runtime::execute(
                    parsed,
                    config.clone(),
                    defaults.clone(),
                    InvocationSource::Workflow,
                )
                .await?;
            }
        }
    }

    Ok(())
}

fn selected_workflows(config: &Config, name: Option<&str>) -> Result<Vec<(String, Workflow)>> {
    if let Some(name) = name {
        let workflow = config
            .workflows
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow!("workflow '{name}' not found"))?;
        return Ok(vec![(name.to_string(), workflow)]);
    }

    Ok(config
        .workflows
        .iter()
        .map(|(name, workflow)| (name.clone(), workflow.clone()))
        .collect())
}

fn render_workflow(
    workflow: &Workflow,
    args: &[String],
    effective: &EffectiveInvocation,
) -> Result<Vec<String>> {
    if workflow.steps.is_empty() {
        bail!("workflow has no steps");
    }

    Ok(workflow
        .steps
        .iter()
        .map(|step| render_step(step, args, effective))
        .collect())
}

fn render_step(step: &str, args: &[String], effective: &EffectiveInvocation) -> String {
    let mut rendered = step.replace("{{profile}}", &shell_quote(&effective.profile));
    rendered = rendered.replace("{{api_url}}", &shell_quote(&effective.api_url));
    rendered = rendered.replace(
        "{{args}}",
        &args
            .iter()
            .map(|arg| shell_quote(arg))
            .collect::<Vec<_>>()
            .join(" "),
    );

    for index in 0..32 {
        let placeholder = format!("{{{{{}}}}}", index + 1);
        let value = args
            .get(index)
            .map(|arg| shell_quote(arg))
            .unwrap_or_else(String::new);
        rendered = rendered.replace(&placeholder, &value);
    }

    rendered
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

fn sample_args(workflow: &Workflow) -> Vec<String> {
    let max_index = (1..=32)
        .filter(|index| {
            let placeholder = format!("{{{{{index}}}}}");
            workflow
                .steps
                .iter()
                .any(|step| step.contains(&placeholder))
        })
        .max()
        .unwrap_or(0);
    (1..=max_index).map(|index| format!("arg{index}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_placeholder_arguments() {
        let effective = EffectiveInvocation {
            profile: "prod".into(),
            api_url: "https://relay44-api.onrender.com/v1".into(),
            output: Format::Table,
            quiet: false,
            verbose: false,
            timeout_secs: 30,
        };

        let rendered = render_step(
            "markets get {{1}} --profile {{profile}}",
            &["market-1".into()],
            &effective,
        );

        assert!(rendered.contains("market-1"));
        assert!(rendered.contains("prod"));
    }
}
