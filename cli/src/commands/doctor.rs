use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use tabled::Tabled;

use crate::config::Config;
use crate::output::{self, Format};

#[derive(Tabled)]
struct CheckRow {
    #[tabled(rename = "Check")]
    name: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Details")]
    details: String,
}

pub async fn run(
    config: Arc<Mutex<Config>>,
    profile_name: &str,
    api_url: &str,
    format: Format,
) -> Result<()> {
    let profile = {
        let config = config.lock().unwrap();
        config.profile(profile_name).cloned().unwrap_or_default()
    };

    let api_check = check_api(api_url).await;
    let auth_check = check_auth(api_url, profile.access_token.as_deref()).await;
    let wallet_check = match profile.wallet.as_deref() {
        Some(w) => ("ok".to_string(), w.to_string()),
        None => ("warn".to_string(), "wallet not configured".to_string()),
    };
    let completion_check = check_completions();
    let profile_check = if profile.api_url.trim().is_empty() {
        ("error".to_string(), "API URL is empty".to_string())
    } else {
        (
            "ok".to_string(),
            format!("profile '{profile_name}' is configured"),
        )
    };

    let rows = vec![
        CheckRow {
            name: "profile".into(),
            status: profile_check.0.clone(),
            details: profile_check.1.clone(),
        },
        CheckRow {
            name: "api".into(),
            status: api_check.0.clone(),
            details: api_check.1.clone(),
        },
        CheckRow {
            name: "auth".into(),
            status: auth_check.0.clone(),
            details: auth_check.1.clone(),
        },
        CheckRow {
            name: "wallet".into(),
            status: wallet_check.0.clone(),
            details: wallet_check.1.clone(),
        },
        CheckRow {
            name: "completions".into(),
            status: completion_check.0.clone(),
            details: completion_check.1.clone(),
        },
    ];

    match format {
        Format::Json => output::print_json(&serde_json::json!({
            "profile": profile_name,
            "checks": rows.iter().map(|row| serde_json::json!({
                "name": row.name,
                "status": row.status,
                "details": row.details,
            })).collect::<Vec<_>>()
        })),
        Format::Table => output::print_tabled(&rows),
    }

    Ok(())
}

async fn check_api(api_url: &str) -> (String, String) {
    let url = format!("{}/health", api_url.trim_end_matches("/v1"));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build();

    let Ok(client) = client else {
        return ("error".into(), "failed to build HTTP client".into());
    };

    match client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            ("ok".into(), format!("reachable at {api_url}"))
        }
        Ok(response) => (
            "warn".into(),
            format!("health returned {}", response.status()),
        ),
        Err(error) => ("error".into(), format!("request failed: {error}")),
    }
}

async fn check_auth(api_url: &str, token: Option<&str>) -> (String, String) {
    let env_token = std::env::var("R44_ACCESS_TOKEN").ok();
    let token = token.map(str::to_string).or(env_token).unwrap_or_default();

    if token.is_empty() {
        return ("warn".into(), "access token not configured".into());
    }

    let mut headers = HeaderMap::new();
    let Ok(value) = HeaderValue::from_str(&format!("Bearer {token}")) else {
        return ("error".into(), "access token format is invalid".into());
    };
    headers.insert(AUTHORIZATION, value);

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .default_headers(headers)
        .build()
    {
        Ok(client) => client,
        Err(error) => return ("error".into(), error.to_string()),
    };

    match client.get(format!("{api_url}/wallet/balance")).send().await {
        Ok(response) if response.status().is_success() => {
            ("ok".into(), "authenticated requests succeed".into())
        }
        Ok(response) if response.status() == reqwest::StatusCode::UNAUTHORIZED => {
            ("error".into(), "token was rejected by the API".into())
        }
        Ok(response) => (
            "warn".into(),
            format!("auth probe returned {}", response.status()),
        ),
        Err(error) => ("warn".into(), format!("auth probe failed: {error}")),
    }
}

fn check_completions() -> (String, String) {
    let shell = std::env::var("SHELL").unwrap_or_default();
    let home = dirs::home_dir().unwrap_or_default();
    let candidates = if shell.contains("zsh") {
        vec![home.join(".zfunc").join("_r44")]
    } else if shell.contains("fish") {
        vec![home.join(".config").join("fish/completions/r44.fish")]
    } else if shell.contains("bash") {
        vec![
            home.join(".local/share/bash-completion/completions/r44"),
            home.join(".bash_completion"),
            home.join(".bashrc"),
        ]
    } else {
        Vec::<PathBuf>::new()
    };

    if candidates.is_empty() {
        return (
            "warn".into(),
            "shell completion location could not be inferred".into(),
        );
    }

    if candidates.iter().any(|path| completion_installed(path)) {
        ("ok".into(), "completion files found".into())
    } else {
        ("warn".into(), "completion files not found".into())
    }
}

fn completion_installed(path: &PathBuf) -> bool {
    if !path.exists() {
        return false;
    }
    if path.is_file() {
        if let Ok(contents) = std::fs::read_to_string(path) {
            if contents.contains("r44") {
                return true;
            }
        }
    }
    true
}
