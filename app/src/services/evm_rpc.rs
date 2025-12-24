use anyhow::{anyhow, Result};
use log::warn;
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone)]
pub struct EvmRpcService {
    client: reqwest::Client,
    primary_url: String,
    read_urls: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcLog {
    pub address: Option<String>,
    pub transaction_hash: Option<String>,
    pub block_number: Option<String>,
    pub log_index: Option<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub data: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcTransaction {
    pub hash: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub input: String,
    pub value: String,
    pub block_number: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcTransactionReceipt {
    pub transaction_hash: Option<String>,
    pub status: Option<String>,
    pub block_number: Option<String>,
    #[serde(default)]
    pub logs: Vec<RpcLog>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: String,
}

impl EvmRpcService {
    pub fn new(primary_url: &str, fallback_urls: &[String]) -> Self {
        let mut read_urls = vec![primary_url.trim().to_string()];
        for candidate in fallback_urls {
            let candidate = candidate.trim();
            if candidate.is_empty()
                || read_urls
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(candidate))
            {
                continue;
            }
            read_urls.push(candidate.to_string());
        }

        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            primary_url: read_urls
                .first()
                .cloned()
                .unwrap_or_else(|| primary_url.trim().to_string()),
            read_urls,
        }
    }

    pub async fn eth_call(&self, to: &str, data: &str) -> Result<String> {
        let params = serde_json::json!([
            {
                "to": to,
                "data": data
            },
            "latest"
        ]);
        let value = self.rpc_value_call("eth_call", params).await?;
        value
            .as_str()
            .map(|v| v.to_string())
            .ok_or_else(|| anyhow!("Invalid eth_call response"))
    }

    pub async fn eth_block_number(&self) -> Result<u64> {
        let value = self
            .rpc_value_call("eth_blockNumber", serde_json::json!([]))
            .await?;
        let raw = value
            .as_str()
            .ok_or_else(|| anyhow!("Invalid eth_blockNumber response"))?;
        parse_u64_hex(raw)
    }

    pub async fn eth_get_logs(
        &self,
        address: &str,
        topic0: &str,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<RpcLog>> {
        let params = serde_json::json!([
            {
                "address": address,
                "fromBlock": quantity_hex(from_block),
                "toBlock": quantity_hex(to_block),
                "topics": [topic0]
            }
        ]);
        let value = self.rpc_value_call("eth_getLogs", params).await?;
        serde_json::from_value(value).map_err(|e| anyhow!("Invalid eth_getLogs payload: {}", e))
    }

    pub async fn eth_get_block_timestamp(&self, block_number: u64) -> Result<u64> {
        let params = serde_json::json!([quantity_hex(block_number), false]);
        let value = self.rpc_value_call("eth_getBlockByNumber", params).await?;
        let timestamp = value
            .get("timestamp")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Invalid block payload"))?;
        parse_u64_hex(timestamp)
    }

    pub async fn eth_send_raw_transaction(&self, raw_tx: &str) -> Result<String> {
        let params = serde_json::json!([raw_tx]);
        let value = self
            .rpc_value_call("eth_sendRawTransaction", params)
            .await?;
        value
            .as_str()
            .map(|v| v.to_string())
            .ok_or_else(|| anyhow!("Invalid eth_sendRawTransaction response"))
    }

    pub async fn eth_get_transaction_by_hash(
        &self,
        tx_hash: &str,
    ) -> Result<Option<RpcTransaction>> {
        let value = self
            .rpc_value_call("eth_getTransactionByHash", serde_json::json!([tx_hash]))
            .await?;
        if value.is_null() {
            return Ok(None);
        }

        let tx = serde_json::from_value(value)
            .map_err(|e| anyhow!("Invalid eth_getTransactionByHash payload: {}", e))?;
        Ok(Some(tx))
    }

    pub async fn eth_get_transaction_receipt(
        &self,
        tx_hash: &str,
    ) -> Result<Option<RpcTransactionReceipt>> {
        let value = self
            .rpc_value_call("eth_getTransactionReceipt", serde_json::json!([tx_hash]))
            .await?;
        if value.is_null() {
            return Ok(None);
        }

        let receipt = serde_json::from_value(value)
            .map_err(|e| anyhow!("Invalid eth_getTransactionReceipt payload: {}", e))?;
        Ok(Some(receipt))
    }

    async fn rpc_value_call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let allow_failover = method != "eth_sendRawTransaction";
        let mut last_err = None;

        let endpoints: Vec<&str> = if allow_failover {
            self.read_urls.iter().map(String::as_str).collect()
        } else {
            vec![self.primary_url.as_str()]
        };

        for (index, url) in endpoints.iter().enumerate() {
            match self
                .rpc_value_call_to_url(url, method, params.clone())
                .await
            {
                Ok(value) => return Ok(value),
                Err(err) => {
                    let should_retry = allow_failover
                        && index + 1 < endpoints.len()
                        && is_retryable_rpc_error(err.to_string().as_str());
                    if should_retry {
                        warn!(
                            "Base RPC {} failed on endpoint {}: {}",
                            method,
                            index + 1,
                            err
                        );
                        last_err = Some(err);
                        continue;
                    }
                    return Err(err);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("Base RPC response missing result")))
    }

    async fn rpc_value_call_to_url(
        &self,
        rpc_url: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let response = self
            .client
            .post(rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow!("Base RPC request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Base RPC returned non-success status: {}",
                response.status()
            ));
        }

        let payload: JsonRpcResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to decode Base RPC response: {}", e))?;

        if let Some(error) = payload.error {
            return Err(anyhow!("Base RPC error: {}", error.message));
        }

        payload
            .result
            .ok_or_else(|| anyhow!("Base RPC response missing result"))
    }
}

fn is_retryable_rpc_error(message: &str) -> bool {
    let message = message.trim().to_ascii_lowercase();
    message.contains("429")
        || message.contains("403")
        || message.contains("forbidden")
        || message.contains("401")
        || message.contains("unauthorized")
        || message.contains("too many requests")
        || message.contains("timeout")
        || message.contains("connection")
        || message.contains("bad gateway")
        || message.contains("service unavailable")
        || message.contains("gateway timeout")
        || message.contains("temporarily unavailable")
}

pub fn quantity_hex(value: u64) -> String {
    format!("0x{:x}", value)
}

pub fn parse_u64_hex(value: &str) -> Result<u64> {
    let trimmed = value.trim_start_matches("0x");
    if trimmed.is_empty() {
        return Err(anyhow!("Invalid RPC hex value"));
    }

    let normalized = trimmed.trim_start_matches('0');
    if normalized.is_empty() {
        return Ok(0);
    }
    if normalized.len() > 16 {
        return Err(anyhow!("RPC value out of range for u64"));
    }

    u64::from_str_radix(normalized, 16).map_err(|_| anyhow!("Invalid RPC hex value"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evm_rpc_service_deduplicates_fallback_endpoints() {
        let service = EvmRpcService::new(
            "https://primary.example",
            &[
                "https://backup-a.example".to_string(),
                "https://primary.example".to_string(),
                "https://backup-b.example".to_string(),
            ],
        );

        assert_eq!(
            service.read_urls,
            vec![
                "https://primary.example".to_string(),
                "https://backup-a.example".to_string(),
                "https://backup-b.example".to_string()
            ]
        );
        assert_eq!(service.primary_url, "https://primary.example");
    }

    #[test]
    fn retryable_rpc_errors_cover_rate_limits_and_transport_failures() {
        assert!(is_retryable_rpc_error("429 Too Many Requests"));
        assert!(is_retryable_rpc_error(
            "Base RPC request failed: connection reset"
        ));
        assert!(!is_retryable_rpc_error(
            "Base RPC error: execution reverted"
        ));
    }
}

