use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::services::evm_rpc::RpcLog;
use crate::services::EvmRpcService;

#[derive(Clone)]
pub struct IndexedEvmLog {
    pub topic0: String,
    pub log: RpcLog,
}

struct IndexerState {
    last_synced_block: u64,
    logs: Vec<IndexedEvmLog>,
}

impl IndexerState {
    fn new() -> Self {
        Self {
            last_synced_block: 0,
            logs: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct EvmIndexerService {
    rpc: EvmRpcService,
    state: Arc<RwLock<IndexerState>>,
    max_logs: usize,
}

impl EvmIndexerService {
    pub fn new(rpc: EvmRpcService, max_logs: usize) -> Self {
        Self {
            rpc,
            state: Arc::new(RwLock::new(IndexerState::new())),
            max_logs,
        }
    }

    pub async fn sync(
        &self,
        market_core_address: &str,
        order_book_address: &str,
        lookback_blocks: u64,
        topics: &[&str],
        latest_block_override: Option<u64>,
    ) -> Result<usize> {
        let latest_block = match latest_block_override {
            Some(value) => value,
            None => self.rpc.eth_block_number().await?,
        };
        let from_block = {
            let state = self.state.read().await;
            if state.last_synced_block == 0 {
                latest_block.saturating_sub(lookback_blocks)
            } else {
                state.last_synced_block.saturating_add(1)
            }
        };

        if from_block > latest_block {
            return Ok(0);
        }

        let mut additions = Vec::new();
        for topic0 in topics {
            additions.extend(
                self.fetch_indexed_logs(order_book_address, topic0, from_block, latest_block)
                    .await?,
            );
            additions.extend(
                self.fetch_indexed_logs(market_core_address, topic0, from_block, latest_block)
                    .await?,
            );
        }

        if additions.is_empty() {
            let mut state = self.state.write().await;
            state.last_synced_block = latest_block;
            return Ok(0);
        }

        let mut state = self.state.write().await;
        for item in additions {
            let tx = item.log.transaction_hash.as_deref().unwrap_or_default();
            let idx = item.log.log_index.as_deref().unwrap_or_default();
            let exists = state.logs.iter().any(|existing| {
                existing.topic0 == item.topic0
                    && existing.log.transaction_hash.as_deref().unwrap_or_default() == tx
                    && existing.log.log_index.as_deref().unwrap_or_default() == idx
            });
            if !exists {
                state.logs.push(item);
            }
        }
        state.logs.sort_by(|a, b| {
            let block_a = a.log.block_number.as_deref().unwrap_or_default();
            let block_b = b.log.block_number.as_deref().unwrap_or_default();
            block_b.cmp(block_a).then_with(|| {
                let idx_a = a.log.log_index.as_deref().unwrap_or_default();
                let idx_b = b.log.log_index.as_deref().unwrap_or_default();
                idx_b.cmp(idx_a)
            })
        });
        if state.logs.len() > self.max_logs {
            state.logs.truncate(self.max_logs);
        }
        state.last_synced_block = latest_block;

        Ok(state.logs.len())
    }

    pub async fn set_last_synced_block(&self, block: u64) {
        let mut state = self.state.write().await;
        state.last_synced_block = block;
    }

    pub async fn last_synced_block(&self) -> u64 {
        let state = self.state.read().await;
        state.last_synced_block
    }

    pub async fn logs_by_topic(&self, topic0: &str) -> Vec<RpcLog> {
        let state = self.state.read().await;
        state
            .logs
            .iter()
            .filter(|item| item.topic0.eq_ignore_ascii_case(topic0))
            .map(|item| item.log.clone())
            .collect()
    }

    async fn fetch_indexed_logs(
        &self,
        address: &str,
        topic0: &str,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<IndexedEvmLog>> {
        if address.is_empty() {
            return Ok(Vec::new());
        }

        let logs = self
            .rpc
            .eth_get_logs(address, topic0, from_block, to_block)
            .await?;
        let _observed_at = Utc::now().to_rfc3339();

        Ok(logs
            .into_iter()
            .map(|log| IndexedEvmLog {
                topic0: topic0.to_string(),
                log,
            })
            .collect())
    }
}
