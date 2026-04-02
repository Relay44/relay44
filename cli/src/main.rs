mod client;
mod commands;
mod config;
mod output;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[derive(Parser)]
#[command(
    name = "r44",
    about = "relay44 — prediction markets and agent execution",
    long_about = "relay44 CLI — trade prediction markets, manage agents, and access \
                  market data across Polymarket, Limitless, and relay44's native venue.\n\n\
                  Get started:\n  \
                  r44 setup\n  \
                  r44 markets list\n  \
                  r44 orders place --market <ID> --side buy --price 0.65 --size 100\n  \
                  r44 shell",
    version,
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format (table, json)
    #[arg(long, global = true, default_value = "table", env = "R44_OUTPUT")]
    output: output::Format,

    /// API base URL
    #[arg(long, global = true, env = "R44_API_URL")]
    api_url: Option<String>,

    /// Suppress non-essential output
    #[arg(long, short, global = true, env = "R44_QUIET")]
    quiet: bool,

    /// Show HTTP requests and debug info
    #[arg(long, short, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Guided first-time setup
    #[command(
        long_about = "Interactive setup wizard for first-time users.\n\
                      Tests API connectivity, configures authentication, \
                      and sets up shell completions.\n\n\
                      Example:\n  r44 setup"
    )]
    Setup,

    /// Interactive shell with command history
    #[command(
        long_about = "Launch an interactive shell with readline and command history.\n\
                      Type commands without the 'r44' prefix.\n\n\
                      Examples:\n  \
                      r44 shell\n  \
                      r44> markets list\n  \
                      r44> agents public\n  \
                      r44> exit"
    )]
    Shell,

    /// Authenticate with relay44
    #[command(
        long_about = "Authenticate with relay44 using a Solana or EVM wallet.\n\n\
                      For automated agents, pass your private key directly. \
                      For manual use, sign the message externally and pass --signature.\n\n\
                      Examples:\n  \
                      r44 login solana --wallet <PUBKEY> --private-key <KEY>\n  \
                      r44 login siwe --address 0x... --signature 0x... --message <MSG>\n  \
                      r44 login status\n  \
                      r44 login logout"
    )]
    Login {
        #[command(subcommand)]
        cmd: commands::login::LoginCmd,
    },

    /// Browse and search markets
    #[command(
        long_about = "Browse and search prediction markets across all venues.\n\n\
                      Examples:\n  \
                      r44 markets list\n  \
                      r44 markets list --status open --query bitcoin\n  \
                      r44 markets get <MARKET_ID>\n  \
                      r44 markets orderbook <MARKET_ID>"
    )]
    Markets {
        #[command(subcommand)]
        cmd: commands::markets::MarketCmd,
    },

    /// Manage orders
    #[command(
        long_about = "Place, list, and cancel orders on prediction markets.\n\n\
                      Examples:\n  \
                      r44 orders list\n  \
                      r44 orders place --market <ID> --side buy --price 0.65 --size 100\n  \
                      r44 orders cancel <ORDER_ID>\n  \
                      r44 orders cancel-all"
    )]
    Orders {
        #[command(subcommand)]
        cmd: commands::orders::OrderCmd,
    },

    /// View positions and claim winnings
    #[command(
        long_about = "View your open positions and claim winnings from resolved markets.\n\n\
                      Examples:\n  \
                      r44 positions list\n  \
                      r44 positions get <MARKET_ID>\n  \
                      r44 positions claim <MARKET_ID>"
    )]
    Positions {
        #[command(subcommand)]
        cmd: commands::positions::PositionCmd,
    },

    /// Manage autonomous agents
    #[command(
        long_about = "Create, monitor, and control autonomous trading agents.\n\n\
                      Examples:\n  \
                      r44 agents public\n  \
                      r44 agents list\n  \
                      r44 agents create --name my-bot\n  \
                      r44 agents execute <AGENT_ID>"
    )]
    Agents {
        #[command(subcommand)]
        cmd: commands::agents::AgentCmd,
    },

    /// Wallet balance and deposit info
    #[command(
        long_about = "View your wallet balance and deposit address.\n\n\
                      Examples:\n  \
                      r44 wallet balance\n  \
                      r44 wallet deposit-address"
    )]
    Wallet {
        #[command(subcommand)]
        cmd: commands::wallet::WalletCmd,
    },

    /// Configure CLI settings
    #[command(
        long_about = "View and modify CLI configuration.\n\n\
                      Config is stored at ~/.config/r44/config.json (mode 0600).\n\
                      All settings can also be set via environment variables:\n  \
                      R44_API_URL, R44_ACCESS_TOKEN, R44_OUTPUT, R44_QUIET\n\n\
                      Examples:\n  \
                      r44 config show\n  \
                      r44 config set-url https://api.example.com/v1\n  \
                      r44 config path"
    )]
    Config {
        #[command(subcommand)]
        cmd: commands::config::ConfigCmd,
    },

    /// Generate shell completions
    #[command(
        long_about = "Generate shell completion scripts for bash, zsh, fish, or powershell.\n\n\
                      Examples:\n  \
                      r44 completions bash >> ~/.bashrc\n  \
                      r44 completions zsh > ~/.zfunc/_r44\n  \
                      r44 completions fish > ~/.config/fish/completions/r44.fish"
    )]
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
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

    if std::env::var("NO_COLOR").is_ok() || std::env::var("CI").is_ok() {
        colored::control::set_override(false);
    }

    if cli.quiet {
        colored::control::set_override(false);
        output::set_quiet(true);
    }

    if cli.verbose {
        output::set_verbose(true);
    }

    if let Commands::Completions { shell } = &cli.command {
        let mut cmd = Cli::command();
        clap_complete::generate(*shell, &mut cmd, "r44", &mut std::io::stdout());
        return;
    }

    let cfg = match config::Config::load() {
        Ok(c) => c,
        Err(e) => {
            output::error(&format!("Failed to load config: {e}"));
            std::process::exit(1);
        }
    };

    let cfg = if let Ok(token) = std::env::var("R44_ACCESS_TOKEN") {
        let mut c = cfg;
        c.access_token = Some(token);
        c
    } else {
        cfg
    };

    let api_url = cli
        .api_url
        .clone()
        .unwrap_or_else(|| cfg.api_url.clone());
    let config = Arc::new(Mutex::new(cfg));
    let api = match client::Client::new(&api_url, config.clone()) {
        Ok(c) => c,
        Err(e) => {
            output::error(&format!("Failed to initialize client: {e}"));
            std::process::exit(1);
        }
    };
    let fmt = cli.output;

    let result = match cli.command {
        Commands::Setup => commands::setup::run(config.clone(), &api_url).await,
        Commands::Shell => commands::shell::run(&api, config.clone(), &api_url, fmt).await,
        Commands::Login { cmd } => commands::login::run(cmd, config.clone(), &api_url).await,
        Commands::Markets { cmd } => commands::markets::run(cmd, &api, fmt).await,
        Commands::Orders { cmd } => commands::orders::run(cmd, &api, fmt).await,
        Commands::Positions { cmd } => commands::positions::run(cmd, &api, fmt).await,
        Commands::Agents { cmd } => commands::agents::run(cmd, &api, fmt).await,
        Commands::Wallet { cmd } => commands::wallet::run(cmd, &api, fmt).await,
        Commands::Config { cmd } => commands::config::run(cmd, config.clone()),
        Commands::Completions { .. } => unreachable!(),
    };

    if let Err(e) = result {
        output::error(&format!("{e:#}"));
        std::process::exit(1);
    }
}

pub fn is_interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}
