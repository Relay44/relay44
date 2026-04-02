use anyhow::{bail, Result};
use clap::Subcommand;
use tabled::Tabled;

use crate::client::Client;
use crate::output::{self, Format};

#[derive(Subcommand)]
pub enum AgentCmd {
    /// List your agents
    #[command(long_about = "List all agents owned by your account.\n\n\
                            Example:\n  r44 agents list")]
    List,

    /// Get agent details
    #[command(long_about = "Show full details for a specific agent including strategy config.\n\n\
                            Example:\n  r44 agents get <AGENT_ID>")]
    Get { id: String },

    /// List public community agents
    #[command(long_about = "Browse all public agents and their performance. No auth required.\n\n\
                            Examples:\n  r44 agents public\n  r44 --output json agents public")]
    Public,

    /// Create a new agent
    #[command(long_about = "Create a new autonomous trading agent.\n\n\
                            Examples:\n  \
                            r44 agents create --name my-bot\n  \
                            r44 agents create --name arb-bot --config '{\"strategy\":\"momentum\"}'")]
    Create {
        #[arg(long, short)]
        name: String,
        /// Strategy config as JSON string
        #[arg(long, short)]
        config: Option<String>,
    },

    /// Update an existing agent
    #[command(long_about = "Update agent configuration. Pass fields as a JSON object.\n\n\
                            Example:\n  r44 agents update <ID> --data '{\"active\":false}'")]
    Update {
        id: String,
        #[arg(long, short)]
        data: String,
    },

    /// Trigger a manual execution tick
    #[command(long_about = "Manually trigger one execution cycle of an agent.\n\n\
                            Example:\n  r44 agents execute <AGENT_ID>")]
    Execute { id: String },
}

#[derive(Tabled)]
struct AgentRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Strategy")]
    strategy: String,
    #[tabled(rename = "Active")]
    active: String,
    #[tabled(rename = "P&L")]
    pnl: String,
}

pub async fn run(cmd: AgentCmd, api: &Client, fmt: Format) -> Result<()> {
    if let AgentCmd::Public = &cmd {
        let sp = output::spinner("Fetching public agents…");
        let data: serde_json::Value = api.get_raw("/external/agents/public").await?;
        sp.finish_and_clear();
        match fmt {
            Format::Json => output::print_json(&data),
            Format::Table => print_agent_table(&data),
        }
        return Ok(());
    }

    require_auth(api)?;

    match cmd {
        AgentCmd::List => {
            let sp = output::spinner("Fetching agents…");
            let data: serde_json::Value = api.get_raw("/external/agents").await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => print_agent_table(&data),
            }
        }
        AgentCmd::Get { id } => {
            let sp = output::spinner("Fetching agent…");
            let data: serde_json::Value =
                api.get_raw(&format!("/external/agents/{id}")).await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    output::print_detail(&[
                        ("Name", output::str_val(&data, "name")),
                        ("ID", output::str_val(&data, "id")),
                        ("Strategy", output::str_val(&data, "strategy")),
                        ("Active", output::str_val(&data, "active")),
                        ("Provider", output::str_val(&data, "provider")),
                        ("Market", output::str_val(&data, "marketId")),
                        ("P&L", output::str_val(&data, "pnl")),
                        ("Last Run", output::format_date(&data, "lastExecutedAt")),
                        ("Created", output::format_date(&data, "createdAt")),
                    ]);
                }
            }
        }
        AgentCmd::Create { name, config } => {
            let mut body = serde_json::json!({ "name": name });
            if let Some(cfg_str) = &config {
                let cfg_val: serde_json::Value = serde_json::from_str(cfg_str)
                    .map_err(|e| anyhow::anyhow!("invalid JSON config: {e}\n  hint: wrap in single quotes and use double quotes inside"))?;
                body["config"] = cfg_val;
            }
            let sp = output::spinner("Creating agent…");
            let data: serde_json::Value = api.post_raw("/external/agents", &body).await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => {
                    output::success(&format!("Agent '{}' created", name));
                    output::print_detail(&[("ID", output::str_val(&data, "id"))]);
                }
            }
        }
        AgentCmd::Update { id, data: data_str } => {
            let body: serde_json::Value = serde_json::from_str(&data_str)
                .map_err(|e| anyhow::anyhow!("invalid JSON: {e}\n  hint: wrap in single quotes and use double quotes inside"))?;
            let sp = output::spinner("Updating agent…");
            let data: serde_json::Value =
                api.patch_raw(&format!("/external/agents/{id}"), &body).await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => output::success(&format!("Agent {id} updated")),
            }
        }
        AgentCmd::Execute { id } => {
            let sp = output::spinner("Executing agent…");
            let body = serde_json::json!({});
            let data: serde_json::Value =
                api.post_raw(&format!("/external/agents/{id}/execute"), &body).await?;
            sp.finish_and_clear();
            match fmt {
                Format::Json => output::print_json(&data),
                Format::Table => output::success(&format!("Agent {id} executed")),
            }
        }
        AgentCmd::Public => unreachable!(),
    }
    Ok(())
}

fn print_agent_table(data: &serde_json::Value) {
    let agents = data["agents"].as_array().or_else(|| data.as_array());
    let Some(agents) = agents else {
        output::dimmed("(no agents)");
        return;
    };
    let rows: Vec<AgentRow> = agents
        .iter()
        .map(|a| AgentRow {
            id: output::truncate(&output::str_val(a, "id"), 20),
            name: output::truncate(&output::str_val(a, "name"), 25),
            strategy: output::str_val(a, "strategy"),
            active: output::str_val(a, "active"),
            pnl: output::str_val(a, "pnl"),
        })
        .collect();
    output::print_tabled(&rows);
}

fn require_auth(api: &Client) -> Result<()> {
    if api.is_authenticated() {
        return Ok(());
    }
    bail!(
        "Not logged in.\n\n  \
         r44 login solana --wallet <PUBKEY> --private-key <KEY>\n  \
         r44 config set-token <TOKEN>  (if you have a token already)"
    );
}
