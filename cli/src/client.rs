use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
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
}

impl Client {
    pub fn new(base_url: &str, config: Arc<Mutex<Config>>) -> Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .context("failed to build HTTP client")?,
            base_url: base_url.trim_end_matches('/').to_string(),
            config,
        })
    }

    pub fn is_authenticated(&self) -> bool {
        self.config.lock().unwrap().access_token.is_some()
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        let cfg = self.config.lock().unwrap();
        if let Some(ref t) = cfg.access_token {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {t}")) {
                headers.insert(AUTHORIZATION, val);
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
        let mut last_err = None;

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay = INITIAL_BACKOFF_MS * 2u64.pow(attempt - 1);
                output::debug(&format!("retry {attempt}/{MAX_RETRIES} after {delay}ms"));
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }

            output::debug(&format!("{method} {url}"));

            let mut req = self.http.request(method.clone(), &url).headers(self.auth_headers());
            if let Some(b) = body {
                output::debug(&format!("body: {}", serde_json::to_string(b).unwrap_or_default()));
                req = req.json(b);
            }

            let resp = match req.send().await {
                Ok(r) => r,
                Err(e) if e.is_timeout() => {
                    last_err = Some(anyhow::anyhow!(
                        "Request timed out ({method} {path}). Check your connection or try --api-url."
                    ));
                    continue;
                }
                Err(e) if e.is_connect() => {
                    last_err = Some(anyhow::anyhow!(
                        "Cannot connect to {url}. Is the API running? Try --api-url to use a different server."
                    ));
                    continue;
                }
                Err(e) => return Err(e).with_context(|| format!("{method} {path}")),
            };

            let status = resp.status();
            output::debug(&format!("← {status}"));

            if status == StatusCode::UNAUTHORIZED {
                if self.try_refresh().await {
                    let mut req2 = self.http.request(method.clone(), &url).headers(self.auth_headers());
                    if let Some(b) = body {
                        req2 = req2.json(b);
                    }
                    let resp2 = req2.send().await.with_context(|| format!("{method} {path}"))?;
                    let status2 = resp2.status();
                    if status2.is_success() {
                        return resp2.json().await.with_context(|| format!("parse {path}"));
                    }
                    return Err(format_api_error(status2, &resp2.text().await.unwrap_or_default(), path));
                }
                return Err(anyhow::anyhow!(
                    "Session expired. Run `r44 login solana` to re-authenticate."
                ));
            }

            if is_retryable(status) {
                let text = resp.text().await.unwrap_or_default();
                last_err = Some(format_api_error(status, &text, path));
                continue;
            }

            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                return Err(format_api_error(status, &text, path));
            }

            return resp.json().await.with_context(|| format!("parse {path}"));
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("{method} {path}: max retries exceeded")))
    }

    async fn try_refresh(&self) -> bool {
        let refresh_token = {
            let cfg = self.config.lock().unwrap();
            cfg.refresh_token.clone()
        };

        let Some(token) = refresh_token else {
            return false;
        };

        output::debug("refreshing access token…");

        let url = format!("{}/auth/refresh", self.base_url);
        let body = serde_json::json!({ "refresh_token": token });
        let resp = match self.http.post(&url).json(&body).send().await {
            Ok(r) if r.status().is_success() => r,
            _ => return false,
        };

        let data: serde_json::Value = match resp.json().await {
            Ok(d) => d,
            Err(_) => return false,
        };

        let access = data["access_token"].as_str().map(String::from);
        let refresh = data["refresh_token"].as_str().map(String::from);

        if access.is_none() {
            return false;
        }

        let mut cfg = self.config.lock().unwrap();
        cfg.access_token = access;
        if refresh.is_some() {
            cfg.refresh_token = refresh;
        }
        let _ = cfg.save();
        output::debug("token refreshed");
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

fn format_api_error(status: StatusCode, body: &str, path: &str) -> anyhow::Error {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        let message = json["message"]
            .as_str()
            .or_else(|| json["error"].as_str())
            .unwrap_or(body);

        let hint = match status {
            StatusCode::NOT_FOUND => format!("\n  hint: resource not found at {path}"),
            StatusCode::FORBIDDEN => "\n  hint: you don't have permission. Check your account role.".into(),
            StatusCode::TOO_MANY_REQUESTS => "\n  hint: rate limited. Wait a moment and retry.".into(),
            StatusCode::UNPROCESSABLE_ENTITY => "\n  hint: check your input values.".into(),
            _ => String::new(),
        };

        anyhow::anyhow!("{message}{hint}")
    } else if body.is_empty() {
        anyhow::anyhow!("{status} on {path}")
    } else {
        anyhow::anyhow!("{status}: {body}")
    }
}
