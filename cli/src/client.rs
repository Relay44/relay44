use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::{Method, StatusCode};

use crate::config::Config;
use crate::output;

const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 500;

pub struct Client {
    http: reqwest::Client,
    base_url: String,
    config: Arc<Mutex<Config>>,
    profile: String,
}

impl Client {
    pub fn new(
        base_url: &str,
        config: Arc<Mutex<Config>>,
        profile: impl Into<String>,
        timeout_secs: u64,
    ) -> Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(timeout_secs))
                .build()
                .context("failed to build HTTP client")?,
            base_url: base_url.trim_end_matches('/').to_string(),
            config,
            profile: profile.into(),
        })
    }

    pub fn is_authenticated(&self) -> bool {
        std::env::var("R44_ACCESS_TOKEN").ok().is_some()
            || self
                .config
                .lock()
                .expect("config lock")
                .profile(&self.profile)
                .and_then(|profile| profile.access_token.as_ref())
                .is_some()
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(token) = self.access_token() {
            if let Ok(value) = HeaderValue::from_str(&format!("Bearer {token}")) {
                headers.insert(AUTHORIZATION, value);
            }
        }
        headers
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let url = format!("{}{}", self.base_url, path);
        let mut last_error = None;

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay = INITIAL_BACKOFF_MS * 2u64.pow(attempt - 1);
                output::debug(&format!(
                    "retry {attempt}/{MAX_RETRIES} after {delay}ms: {}",
                    retry_reason(&last_error)
                ));
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }

            output::debug(&format!("{method} {url}"));
            let mut request = self
                .http
                .request(method.clone(), &url)
                .headers(self.auth_headers());
            if let Some(payload) = body {
                output::debug(&format!(
                    "body: {}",
                    serde_json::to_string(payload).unwrap_or_default()
                ));
                request = request.json(payload);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(error) if error.is_timeout() => {
                    last_error = Some(anyhow!(
                        "request timed out ({method} {path}). retrying as a network timeout"
                    ));
                    continue;
                }
                Err(error) if error.is_connect() => {
                    last_error = Some(anyhow!(
                        "cannot connect to {url}. retrying as a network error"
                    ));
                    continue;
                }
                Err(error) => return Err(error).with_context(|| format!("{method} {path}")),
            };

            let status = response.status();
            output::debug(&format!("← {status}"));

            if status == StatusCode::UNAUTHORIZED {
                if self.try_refresh().await {
                    output::debug("retrying after auth refresh");
                    continue;
                }
                return Err(anyhow!(
                    "session expired for profile '{}'. run `r44 login solana` or `r44 login siwe` again",
                    self.profile
                ));
            }

            if is_retryable(status) {
                let text = response.text().await.unwrap_or_default();
                last_error = Some(format_api_error(status, &text, path));
                continue;
            }

            if !status.is_success() {
                let text = response.text().await.unwrap_or_default();
                return Err(format_api_error(status, &text, path));
            }

            return response
                .json()
                .await
                .with_context(|| format!("parse {path}"));
        }

        Err(last_error.unwrap_or_else(|| anyhow!("{method} {path}: max retries exceeded")))
    }

    async fn try_refresh(&self) -> bool {
        if std::env::var("R44_ACCESS_TOKEN").ok().is_some() {
            return false;
        }

        let refresh_token = {
            let config = self.config.lock().expect("config lock");
            config
                .profile(&self.profile)
                .and_then(|profile| profile.refresh_token.clone())
        };

        let Some(token) = refresh_token else {
            return false;
        };

        output::debug("refreshing access token");

        let url = format!("{}/auth/refresh", self.base_url);
        let body = serde_json::json!({ "refresh_token": token });
        let response = match self.http.post(&url).json(&body).send().await {
            Ok(response) if response.status().is_success() => response,
            _ => return false,
        };

        let data: serde_json::Value = match response.json().await {
            Ok(data) => data,
            Err(_) => return false,
        };

        let access_token = data["access_token"].as_str().map(String::from);
        let refresh_token = data["refresh_token"].as_str().map(String::from);
        let Some(access_token) = access_token else {
            return false;
        };

        let mut config = self.config.lock().expect("config lock");
        let profile = config.ensure_profile(&self.profile);
        profile.access_token = Some(access_token);
        if refresh_token.is_some() {
            profile.refresh_token = refresh_token;
        }
        let _ = config.save();
        true
    }

    pub async fn get_raw(&self, path: &str) -> Result<serde_json::Value> {
        self.request(Method::GET, path, None).await
    }

    pub async fn post_raw(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.request(Method::POST, path, Some(body)).await
    }

    pub async fn patch_raw(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.request(Method::PATCH, path, Some(body)).await
    }

    pub async fn delete_raw(&self, path: &str) -> Result<serde_json::Value> {
        self.request(Method::DELETE, path, None).await
    }

    pub fn require_auth(&self) -> Result<()> {
        if self.is_authenticated() {
            return Ok(());
        }
        Err(anyhow!(
            "Not logged in.\n\n  \
             r44 login solana --wallet <PUBKEY> --private-key <KEY>\n  \
             r44 login siwe --address 0x... --signature 0x... --message <MSG>\n  \
             r44 config set-token <TOKEN>  (if you have a token already)"
        ))
    }

    fn access_token(&self) -> Option<String> {
        std::env::var("R44_ACCESS_TOKEN").ok().or_else(|| {
            self.config
                .lock()
                .expect("config lock")
                .profile(&self.profile)
                .and_then(|profile| profile.access_token.clone())
        })
    }
}

fn is_retryable(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn retry_reason(last_error: &Option<anyhow::Error>) -> &'static str {
    let Some(error) = last_error else {
        return "transient failure";
    };
    let message = error.to_string();
    if message.contains("rate limited") {
        "rate limit"
    } else if message.contains("timeout")
        || message.contains("connect")
        || message.contains("network")
    {
        "network"
    } else {
        "server error"
    }
}

fn format_api_error(status: StatusCode, body: &str, path: &str) -> anyhow::Error {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        let message = json["message"]
            .as_str()
            .or_else(|| json["error"].as_str())
            .unwrap_or(body);

        let hint = match status {
            StatusCode::NOT_FOUND => format!("\n  hint: resource not found at {path}"),
            StatusCode::FORBIDDEN => {
                "\n  hint: you do not have permission for this resource".into()
            }
            StatusCode::TOO_MANY_REQUESTS => {
                "\n  hint: rate limited. wait a moment and retry".into()
            }
            StatusCode::UNPROCESSABLE_ENTITY => "\n  hint: check your input values".into(),
            StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT => {
                "\n  hint: upstream service is unavailable. retry shortly".into()
            }
            _ => String::new(),
        };

        anyhow!("{message}{hint}")
    } else if body.is_empty() {
        anyhow!("{status} on {path}")
    } else {
        anyhow!("{status}: {body}")
    }
}
