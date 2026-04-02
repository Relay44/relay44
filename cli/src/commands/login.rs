use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use clap::Subcommand;
use ed25519_dalek::{Signer, SigningKey};

use crate::config::Config;
use crate::output;

#[derive(Subcommand)]
pub enum LoginCmd {
    /// Login with a Solana wallet
    #[command(
        long_about = "Authenticate with relay44 using a Solana Ed25519 keypair.\n\n\
                      Agent mode (headless):\n  \
                      r44 login solana --wallet <PUBKEY> --private-key <KEY>\n\n\
                      Manual mode (sign externally):\n  \
                      r44 login solana --wallet <PUBKEY> --signature <SIG> --message <MSG>\n\n\
                      Environment variables:\n  \
                      R44_WALLET       Solana wallet address\n  \
                      R44_PRIVATE_KEY  Ed25519 private key (base58)"
    )]
    Solana {
        /// Solana wallet address (base58 pubkey)
        #[arg(long, env = "R44_WALLET")]
        wallet: String,

        /// Ed25519 private key (base58, 32 or 64 bytes). For headless/agent use.
        #[arg(long, env = "R44_PRIVATE_KEY")]
        private_key: Option<String>,

        /// Pre-signed signature (base58)
        #[arg(long, requires = "message")]
        signature: Option<String>,

        /// The signed message (required with --signature)
        #[arg(long, requires = "signature")]
        message: Option<String>,
    },

    /// Login with an EVM/Ethereum wallet (SIWE)
    #[command(
        long_about = "Authenticate with relay44 using Sign-In With Ethereum (SIWE).\n\n\
                      Provide a pre-signed EIP-4361 message and signature.\n\n\
                      Example:\n  \
                      r44 login siwe --address 0x... --signature 0x... --message <MSG>"
    )]
    Siwe {
        /// Ethereum address (0x...)
        #[arg(long)]
        address: String,

        /// SIWE message that was signed
        #[arg(long)]
        message: String,

        /// Hex-encoded signature (0x...)
        #[arg(long)]
        signature: String,
    },

    /// Show current login status
    #[command(
        long_about = "Check if you are currently authenticated and show wallet info.\n\n\
                      Example:\n  r44 login status"
    )]
    Status,

    /// Clear stored credentials
    #[command(
        long_about = "Log out by clearing stored access and refresh tokens.\n\n\
                      Example:\n  r44 login logout"
    )]
    Logout,
}

pub async fn run(
    cmd: LoginCmd,
    config: Arc<Mutex<Config>>,
    api_url: &str,
) -> Result<()> {
    match cmd {
        LoginCmd::Solana {
            wallet,
            private_key,
            signature,
            message,
        } => {
            login_solana(config, api_url, &wallet, private_key, signature, message).await
        }
        LoginCmd::Siwe {
            address,
            message,
            signature,
        } => {
            login_siwe(config, api_url, &address, &message, &signature).await
        }
        LoginCmd::Status => {
            let cfg = config.lock().unwrap();
            if cfg.access_token.is_some() {
                let wallet_str = cfg.wallet.as_deref().unwrap_or("unknown");
                output::print_detail(&[
                    ("Status", "authenticated".into()),
                    ("Wallet", wallet_str.into()),
                    ("API", cfg.api_url.clone()),
                ]);
            } else {
                output::warn("Not logged in");
                output::dimmed("  r44 login solana --wallet <PUBKEY> --private-key <KEY>");
            }
            Ok(())
        }
        LoginCmd::Logout => {
            let mut cfg = config.lock().unwrap();
            cfg.access_token = None;
            cfg.refresh_token = None;
            cfg.wallet = None;
            cfg.save()?;
            output::success("Logged out");
            Ok(())
        }
    }
}

async fn login_solana(
    config: Arc<Mutex<Config>>,
    api_url: &str,
    wallet: &str,
    private_key: Option<String>,
    signature: Option<String>,
    message_str: Option<String>,
) -> Result<()> {
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let (final_message, final_signature) = if let (Some(sig), Some(msg)) = (signature, message_str)
    {
        (msg, sig)
    } else if let Some(pk_str) = private_key {
        let sp = output::spinner("Signing in…");

        let nonce_url = format!("{api_url}/auth/solana/nonce");
        let nonce_resp: serde_json::Value = http
            .get(&nonce_url)
            .send()
            .await
            .context("failed to fetch nonce — is the API reachable?")?
            .json()
            .await
            .context("parse nonce response")?;

        let nonce = nonce_resp["nonce"]
            .as_str()
            .context("server returned no nonce — unexpected response format")?;

        let now = Utc::now().to_rfc3339();
        let domain = extract_domain(api_url);
        let msg = format!(
            "{domain} wants you to sign in with your Solana account:\n\
             {wallet}\n\
             \n\
             Sign in to relay44\n\
             \n\
             Chain: solana\n\
             Nonce: {nonce}\n\
             Issued At: {now}"
        );

        let key_bytes = bs58::decode(&pk_str)
            .into_vec()
            .context("invalid base58 private key — check R44_PRIVATE_KEY")?;

        let signing_key = match key_bytes.len() {
            64 => SigningKey::from_bytes(
                key_bytes[..32].try_into().context("key slice")?,
            ),
            32 => SigningKey::from_bytes(
                key_bytes.as_slice().try_into().context("key slice")?,
            ),
            n => bail!("private key must be 32 or 64 bytes, got {n}"),
        };

        let sig = signing_key.sign(msg.as_bytes());
        let sig_bs58 = bs58::encode(sig.to_bytes()).into_string();

        sp.finish_and_clear();
        (msg, sig_bs58)
    } else {
        bail!(
            "Provide --private-key for agent mode, or --signature + --message for manual signing.\n\n  \
             Agent mode:\n    \
             r44 login solana --wallet <PUBKEY> --private-key <KEY>\n\n  \
             Manual mode:\n    \
             r44 login solana --wallet <PUBKEY> --signature <SIG> --message <MSG>"
        );
    };

    finish_login(&http, &config, api_url, wallet, "/auth/solana/login", &final_message, &final_signature).await
}

async fn login_siwe(
    config: Arc<Mutex<Config>>,
    api_url: &str,
    address: &str,
    message: &str,
    signature: &str,
) -> Result<()> {
    if !address.starts_with("0x") || address.len() != 42 {
        bail!("Invalid Ethereum address: must be 0x followed by 40 hex characters");
    }

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    finish_login(&http, &config, api_url, address, "/auth/siwe/login", message, signature).await
}

async fn finish_login(
    http: &reqwest::Client,
    config: &Arc<Mutex<Config>>,
    api_url: &str,
    wallet: &str,
    login_path: &str,
    message: &str,
    signature: &str,
) -> Result<()> {
    let sp = output::spinner("Authenticating…");

    let login_url = format!("{api_url}{login_path}");
    let login_body = serde_json::json!({
        "wallet": wallet,
        "message": message,
        "signature": signature,
    });

    let resp = http
        .post(&login_url)
        .json(&login_body)
        .send()
        .await
        .context("login request failed — check your connection")?;

    let status = resp.status();
    if !status.is_success() {
        sp.finish_and_clear();
        let body = resp.text().await.unwrap_or_default();
        if status.as_u16() == 401 {
            bail!("Authentication failed — invalid wallet or signature.\n  hint: check that --wallet matches the signing key");
        }
        bail!("Login failed ({status}): {body}");
    }

    let data: serde_json::Value = resp.json().await.context("parse login response")?;

    let access_token = data["access_token"]
        .as_str()
        .context("server returned no access_token")?
        .to_string();
    let refresh_token = data["refresh_token"].as_str().map(String::from);

    {
        let mut cfg = config.lock().unwrap();
        cfg.access_token = Some(access_token);
        cfg.refresh_token = refresh_token;
        cfg.wallet = Some(wallet.to_string());
        cfg.save()?;
    }

    sp.finish_and_clear();
    output::success(&format!("Logged in as {wallet}"));
    Ok(())
}

fn extract_domain(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or("relay44.com")
        .to_string()
}
