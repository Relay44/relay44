use serde_json::Value;

use super::types::{ExternalProvider, ExternalTradeSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceLedgerKind {
    Paper,
    Live,
}

#[derive(Debug, Clone, Copy)]
pub struct PerformanceLedgerTables {
    pub positions: &'static str,
    pub fills: &'static str,
    pub marks: &'static str,
    pub outcomes: &'static str,
}

impl PerformanceLedgerKind {
    pub fn tables(self) -> PerformanceLedgerTables {
        match self {
            Self::Paper => PerformanceLedgerTables {
                positions: "paper_positions",
                fills: "paper_fills",
                marks: "paper_marks",
                outcomes: "paper_outcomes",
            },
            Self::Live => PerformanceLedgerTables {
                positions: "external_positions",
                fills: "external_fills",
                marks: "external_marks",
                outcomes: "external_outcomes",
            },
        }
    }

    pub fn execution_mode(self) -> &'static str {
        match self {
            Self::Paper => "paper",
            Self::Live => "live",
        }
    }
}

pub fn live_trade_reconciliation_supported(provider: ExternalProvider) -> bool {
    !matches!(provider, ExternalProvider::Polymarket)
}

pub fn extract_reference_keys(payload: &Value) -> Vec<String> {
    let mut keys = Vec::new();
    for key in [
        "txHash",
        "transactionHash",
        "tx_hash",
        "providerOrderId",
        "provider_order_id",
        "orderId",
        "orderID",
        "order_id",
    ] {
        if let Some(value) = payload.get(key).and_then(|entry| entry.as_str()) {
            let value = value.trim();
            if !value.is_empty() {
                keys.push(value.to_string());
            }
        }
    }

    if let Some(order) = payload.get("order").and_then(|entry| entry.as_object()) {
        for key in [
            "txHash",
            "transactionHash",
            "tx_hash",
            "orderId",
            "orderID",
            "order_id",
            "id",
        ] {
            if let Some(value) = order.get(key).and_then(|entry| entry.as_str()) {
                let value = value.trim();
                if !value.is_empty() {
                    keys.push(value.to_string());
                }
            }
        }
    }

    keys.sort();
    keys.dedup();
    keys
}

fn trade_reference_keys(trade: &ExternalTradeSnapshot) -> Vec<&str> {
    let mut keys = Vec::new();
    if !trade.id.trim().is_empty() {
        keys.push(trade.id.as_str());
        if let Some((_, suffix)) = trade.id.split_once(':') {
            if !suffix.trim().is_empty() {
                keys.push(suffix);
            }
        }
    }
    if !trade.tx_hash.trim().is_empty() {
        keys.push(trade.tx_hash.as_str());
    }
    keys
}

fn reference_segments(value: &str) -> impl Iterator<Item = &str> {
    value
        .split([':', '/', '#'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
}

fn reference_matches(candidate: &str, reference: &str) -> bool {
    if candidate.eq_ignore_ascii_case(reference) {
        return true;
    }

    let reference = reference.trim();
    if reference.is_empty() {
        return false;
    }

    reference_segments(candidate).any(|segment| segment.eq_ignore_ascii_case(reference))
        || reference_segments(reference).any(|segment| segment.eq_ignore_ascii_case(candidate))
}

pub fn trade_matches_reference(trade: &ExternalTradeSnapshot, payload: &Value) -> bool {
    let reference_keys = extract_reference_keys(payload);
    if reference_keys.is_empty() {
        return false;
    }

    let trade_keys = trade_reference_keys(trade);
    reference_keys.iter().any(|reference| {
        trade_keys
            .iter()
            .any(|candidate| reference_matches(candidate, reference))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_tables_match_scope() {
        assert_eq!(
            PerformanceLedgerKind::Paper.tables().positions,
            "paper_positions"
        );
        assert_eq!(PerformanceLedgerKind::Live.tables().fills, "external_fills");
        assert_eq!(PerformanceLedgerKind::Live.execution_mode(), "live");
    }

    #[test]
    fn extracts_nested_trade_references() {
        let payload = serde_json::json!({
            "txHash": "0xaaa",
            "order": {
                "orderId": "ord-1"
            }
        });
        let keys = extract_reference_keys(&payload);

        assert!(keys.contains(&"0xaaa".to_string()));
        assert!(keys.contains(&"ord-1".to_string()));
    }

    #[test]
    fn matches_trade_by_tx_hash_or_order_id() {
        let payload = serde_json::json!({
            "providerOrderId": "ord-2",
            "order": {
                "id": "ord-3"
            }
        });
        let trade = ExternalTradeSnapshot {
            id: "limitless:ord-2".to_string(),
            market_id: "limitless:test".to_string(),
            outcome: "yes".to_string(),
            price: 0.64,
            price_bps: 6400,
            quantity: 10,
            tx_hash: "0xabc".to_string(),
            block_number: 1,
            created_at: "2026-04-03T00:00:00Z".to_string(),
        };

        assert!(trade_matches_reference(&trade, &payload));
        assert!(live_trade_reconciliation_supported(
            ExternalProvider::Limitless
        ));
        assert!(!live_trade_reconciliation_supported(
            ExternalProvider::Polymarket
        ));
    }
}
