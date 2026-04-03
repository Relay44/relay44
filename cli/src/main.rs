mod client;
mod commands;
mod config;
mod hooks;
mod output;
mod runtime;
mod sessions;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use clap::{Parser, Subcommand};
use clap_complete::Shell;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[derive(Parser, Clone)]
#[command(
    name = "r44",
    about = "relay44 — prediction markets and agent execution",
    long_about = "relay44 CLI — trade prediction markets, manage agents, and access \
                  market data across Polymarket, Limitless, and relay44's native venue.",
    version,
    propagate_version = true
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Output format (table, json)
    #[arg(long, global = true, env = "R44_OUTPUT")]
    pub output: Option<output::Format>,

    /// API base URL
    #[arg(long, global = true, env = "R44_API_URL")]
    pub api_url: Option<String>,

    /// Profile name
    #[arg(long, global = true, env = "R44_PROFILE")]
    pub profile: Option<String>,

    /// Suppress non-essential output
    #[arg(long, short, global = true, env = "R44_QUIET")]
    pub quiet: bool,

    /// Show HTTP requests and debug info
    #[arg(long, short, global = true)]
    pub verbose: bool,

    /// Disable colored output
    #[arg(long, global = true, env = "NO_COLOR")]
    pub no_color: bool,

    /// HTTP request timeout in seconds
    #[arg(long, global = true, env = "R44_TIMEOUT", default_value = "30")]
    pub timeout: u64,
}

#[derive(Subcommand, Clone)]
pub(crate) enum Commands {
    /// Guided setup for a profile
    Setup,

    /// Interactive shell with history and shared command parsing
    Shell,

    /// Diagnose API/auth/profile setup
    Doctor,

    /// Authenticate with relay44
    Login {
        #[command(subcommand)]
        cmd: commands::login::LoginCmd,
    },

    /// Browse and search markets
    Markets {
        #[command(subcommand)]
        cmd: commands::markets::MarketCmd,
    },

    /// Manage orders
    Orders {
        #[command(subcommand)]
        cmd: commands::orders::OrderCmd,
    },

    /// View positions and claim winnings
    Positions {
        #[command(subcommand)]
        cmd: commands::positions::PositionCmd,
    },

    /// Manage autonomous agents
    Agents {
        #[command(subcommand)]
        cmd: commands::agents::AgentCmd,
    },

    /// Calibration and time-decay edge signals
    #[command(name = "edge-scanner")]
    EdgeScanner {
        #[command(subcommand)]
        cmd: commands::edge_scanner::EdgeScannerCmd,
    },

    /// Wallet balance and deposit info
    Wallet {
        #[command(subcommand)]
        cmd: commands::wallet::WalletCmd,
    },

    /// Show and edit CLI config
    Config {
        #[command(subcommand)]
        cmd: commands::config::ConfigCmd,
    },

    /// Manage named profiles
    Profile {
        #[command(subcommand)]
        cmd: commands::profile::ProfileCmd,
    },

    /// Run declarative command workflows
    Workflow {
        #[command(subcommand)]
        cmd: commands::workflow::WorkflowCmd,
    },

    /// Export and replay shell sessions
    Session {
        #[command(subcommand)]
        cmd: commands::session::SessionCmd,
    },

    /// Generate shell completions
    Completions { shell: Shell },
}

impl Cli {
    pub fn command_path(&self) -> String {
        match &self.command {
            Commands::Setup => "setup".into(),
            Commands::Shell => "shell".into(),
            Commands::Doctor => "doctor".into(),
            Commands::Login { cmd } => format!("login {}", login_path(cmd)),
            Commands::Markets { cmd } => format!("markets {}", market_path(cmd)),
            Commands::Orders { cmd } => format!("orders {}", order_path(cmd)),
            Commands::Positions { cmd } => format!("positions {}", position_path(cmd)),
            Commands::Agents { cmd } => format!("agents {}", agent_path(cmd)),
            Commands::EdgeScanner { cmd } => format!("edge-scanner {}", edge_scanner_path(cmd)),
            Commands::Wallet { cmd } => format!("wallet {}", wallet_path(cmd)),
            Commands::Config { cmd } => format!("config {}", config_path(cmd)),
            Commands::Profile { cmd } => format!("profile {}", profile_path(cmd)),
            Commands::Workflow { cmd } => format!("workflow {}", workflow_path(cmd)),
            Commands::Session { cmd } => format!("session {}", session_path(cmd)),
            Commands::Completions { .. } => "completions".into(),
        }
    }
}

#[tokio::main]
async fn main() {
    ctrlc::set_handler(|| {
        if INTERRUPTED.swap(true, Ordering::SeqCst) {
            std::process::exit(130);
        }
    })
    .ok();

    let cli = Cli::parse();

    if cli.no_color || std::env::var("CI").is_ok() {
        colored::control::set_override(false);
    }

    let config = match config::Config::load() {
        Ok(config) => Arc::new(Mutex::new(config)),
        Err(error) => {
            output::error(&format!("failed to load config: {error}"));
            std::process::exit(1);
        }
    };

    if let Err(error) = runtime::execute(
        cli,
        config,
        runtime::InvocationDefaults::default(),
        runtime::InvocationSource::Cli,
    )
    .await
    {
        output::error(&format!("{error:#}"));
        std::process::exit(1);
    }
}

pub(crate) fn is_interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

fn login_path(cmd: &commands::login::LoginCmd) -> &'static str {
    match cmd {
        commands::login::LoginCmd::Solana { .. } => "solana",
        commands::login::LoginCmd::Siwe { .. } => "siwe",
        commands::login::LoginCmd::Status => "status",
        commands::login::LoginCmd::Logout => "logout",
    }
}

fn market_path(cmd: &commands::markets::MarketCmd) -> &'static str {
    match cmd {
        commands::markets::MarketCmd::List { .. } => "list",
        commands::markets::MarketCmd::Get { .. } => "get",
        commands::markets::MarketCmd::Orderbook { .. } => "orderbook",
        commands::markets::MarketCmd::Trades { .. } => "trades",
    }
}

fn order_path(cmd: &commands::orders::OrderCmd) -> &'static str {
    match cmd {
        commands::orders::OrderCmd::List { .. } => "list",
        commands::orders::OrderCmd::Place { .. } => "place",
        commands::orders::OrderCmd::Cancel { .. } => "cancel",
        commands::orders::OrderCmd::CancelAll { .. } => "cancel-all",
        commands::orders::OrderCmd::Get { .. } => "get",
    }
}

fn position_path(cmd: &commands::positions::PositionCmd) -> &'static str {
    match cmd {
        commands::positions::PositionCmd::List { .. } => "list",
        commands::positions::PositionCmd::Get { .. } => "get",
        commands::positions::PositionCmd::Claim { .. } => "claim",
    }
}

fn agent_path(cmd: &commands::agents::AgentCmd) -> &'static str {
    match cmd {
        commands::agents::AgentCmd::List { .. } => "list",
        commands::agents::AgentCmd::Get { .. } => "get",
        commands::agents::AgentCmd::Public => "public",
        commands::agents::AgentCmd::Create { .. } => "create",
        commands::agents::AgentCmd::Update { .. } => "update",
        commands::agents::AgentCmd::Execute { .. } => "execute",
    }
}

fn wallet_path(cmd: &commands::wallet::WalletCmd) -> &'static str {
    match cmd {
        commands::wallet::WalletCmd::Balance => "balance",
        commands::wallet::WalletCmd::DepositAddress => "deposit-address",
    }
}

fn config_path(cmd: &commands::config::ConfigCmd) -> &'static str {
    match cmd {
        commands::config::ConfigCmd::Show => "show",
        commands::config::ConfigCmd::SetUrl { .. } => "set-url",
        commands::config::ConfigCmd::SetToken { .. } => "set-token",
        commands::config::ConfigCmd::Path => "path",
        commands::config::ConfigCmd::Reset { .. } => "reset",
    }
}

fn profile_path(cmd: &commands::profile::ProfileCmd) -> &'static str {
    match cmd {
        commands::profile::ProfileCmd::List => "list",
        commands::profile::ProfileCmd::Use { .. } => "use",
        commands::profile::ProfileCmd::Show { .. } => "show",
    }
}

fn workflow_path(cmd: &commands::workflow::WorkflowCmd) -> &'static str {
    match cmd {
        commands::workflow::WorkflowCmd::List => "list",
        commands::workflow::WorkflowCmd::Run { .. } => "run",
        commands::workflow::WorkflowCmd::Validate { .. } => "validate",
    }
}

fn session_path(cmd: &commands::session::SessionCmd) -> &'static str {
    match cmd {
        commands::session::SessionCmd::Export { .. } => "export",
        commands::session::SessionCmd::Replay { .. } => "replay",
    }
}

fn edge_scanner_path(cmd: &commands::edge_scanner::EdgeScannerCmd) -> &'static str {
    match cmd {
        commands::edge_scanner::EdgeScannerCmd::Signals { .. } => "signals",
        commands::edge_scanner::EdgeScannerCmd::Curve => "curve",
    }
}
