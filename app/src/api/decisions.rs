use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Postgres, QueryBuilder, Row};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use uuid::Uuid;

use crate::api::auth::{extract_authenticated_user, extract_jwt_user};
use crate::api::external::{
    execute_agent_record, load_external_agent_for_owner, run_status_from_error,
    skip_reason_from_error,
};
use crate::api::jwt::{check_role, UserRole};
use crate::api::notifications::{create_notification, NewNotification, NotificationType};
use crate::api::ApiError;
use crate::services::external::{self, types::ExternalMarketId};
use crate::AppState;

const MAX_PAGE_SIZE: i64 = 100;
const RECOMMENDATION_LEAD_THRESHOLD_BPS: i32 = 750;
const MIN_LIVE_NODES_FOR_SIGNAL: usize = 2;

const DECISION_TYPE_TIMING: &str = "timing";
const DECISION_TYPE_CHOICE: &str = "choice";
const DECISION_TYPE_HEDGE: &str = "hedge";
const DECISION_TYPE_ALLOCATION: &str = "allocation";

const SOURCE_TYPE_INTERNAL_MARKET: &str = "internal_market";
const SOURCE_TYPE_EXTERNAL_MARKET: &str = "external_market";
const SOURCE_TYPE_DRAFT_MARKET: &str = "draft_market";

const NODE_STATUS_DRAFT: &str = "draft";
const NODE_STATUS_LIVE: &str = "live";

const TRIGGER_ON_RECOMMENDATION_GAIN: &str = "on_recommendation_gain";
const TRIGGER_ON_THRESHOLD_CROSS: &str = "on_threshold_cross";
const TRIGGER_ON_CONFIDENCE_GAIN: &str = "on_confidence_gain";

const EFFECT_SUPPORT: &str = "support";
const EFFECT_OPPOSE: &str = "oppose";
const EFFECT_NEUTRAL: &str = "neutral";

const RECOMMENDATION_INSUFFICIENT_SIGNAL: &str = "insufficient_signal";
const RECOMMENDATION_ACT_NOW: &str = "act_now";
const RECOMMENDATION_WAIT: &str = "wait";

const EVENT_RECOMMENDATION_CHANGED: &str = "recommendation_changed";
const EVENT_THRESHOLD_CROSSED: &str = "threshold_crossed";
const EVENT_CONFIDENCE_DROPPED: &str = "confidence_dropped";
const EVENT_NODE_ADDED: &str = "node_added";
const EVENT_NODE_UPDATED: &str = "node_updated";
const EVENT_MARKET_ATTACHED: &str = "market_attached";
const EVENT_AGENT_ATTACHED: &str = "agent_attached";
const EVENT_AUTOMATION_UPDATED: &str = "automation_updated";
const EVENT_ALERT_UPDATED: &str = "alert_updated";
const EVENT_AUTOMATION_TRIGGERED: &str = "automation_triggered";
const EVENT_AUTOMATION_SKIPPED: &str = "automation_skipped";

#[derive(Debug, Clone)]
struct DecisionCellRecord {
    id: String,
    owner: String,
    title: String,
    statement: String,
    decision_type: String,
    horizon_at: Option<DateTime<Utc>>,
    status: String,
    automation_enabled: bool,
    current_recommendation: String,
    confidence_bps: i32,
    summary: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct DecisionActionRecord {
    id: String,
    label: String,
    rank: i32,
}

#[derive(Debug, Clone)]
struct DecisionNodeRecord {
    id: String,
    label: String,
    description: String,
    weight_bps: i32,
    source_type: String,
    source_ref: Option<String>,
    status: String,
    last_probability_bps: Option<i32>,
    last_market_snapshot: Value,
    action_effects: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct DecisionAlertRecord {
    id: String,
    kind: String,
    threshold: Value,
    active: bool,
    last_triggered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct DecisionAutomationPolicyRecord {
    max_agent_notional_usdc: f64,
    max_triggers_per_day: i32,
    min_trigger_interval_seconds: i64,
    allowed_provider: Option<String>,
    require_confidence_bps: i32,
    active: bool,
}

#[derive(Debug, Clone)]
struct DecisionNodeAgentRecord {
    id: String,
    node_id: String,
    external_agent_id: String,
    trigger_mode: String,
    active: bool,
    agent_name: Option<String>,
    provider: Option<String>,
    agent_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DecisionActionScore {
    pub action_id: String,
    pub label: String,
    pub rank: i32,
    pub score_bps: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DecisionContributor {
    pub node_id: String,
    pub label: String,
    pub action_label: String,
    pub score_bps: i32,
    pub probability_bps: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_bps: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct DecisionSummary {
    #[serde(default)]
    action_scores: Vec<DecisionActionScore>,
    #[serde(default)]
    top_contributors: Vec<DecisionContributor>,
    #[serde(default)]
    why_changed: String,
    #[serde(default)]
    live_nodes: usize,
    #[serde(default)]
    total_nodes: usize,
    #[serde(default)]
    top_action_lead_bps: i32,
    #[serde(default)]
    last_recalculated_at: String,
    last_changed_node: Option<DecisionContributor>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionRecommendation {
    pub state: String,
    pub confidence_bps: i32,
    pub why_changed: String,
    pub live_nodes: usize,
    pub total_nodes: usize,
    pub top_action_lead_bps: i32,
    pub action_scores: Vec<DecisionActionScore>,
    pub top_contributors: Vec<DecisionContributor>,
    pub last_changed_node: Option<DecisionContributor>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionActionResponse {
    pub id: String,
    pub label: String,
    pub rank: i32,
    pub score_bps: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionNodeAgentResponse {
    pub id: String,
    pub external_agent_id: String,
    pub trigger_mode: String,
    pub active: bool,
    pub name: Option<String>,
    pub provider: Option<String>,
    pub agent_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionNodeResponse {
    pub id: String,
    pub label: String,
    pub description: String,
    pub weight_bps: i32,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub status: String,
    pub last_probability_bps: Option<i32>,
    pub last_market_snapshot: Value,
    pub action_effects: Value,
    pub created_at: String,
    pub updated_at: String,
    pub agents: Vec<DecisionNodeAgentResponse>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionAlertResponse {
    pub id: String,
    pub kind: String,
    pub threshold: Value,
    pub active: bool,
    pub last_triggered_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionAutomationPolicyResponse {
    pub automation_enabled: bool,
    pub max_agent_notional_usdc: f64,
    pub max_triggers_per_day: i32,
    pub min_trigger_interval_seconds: i64,
    pub allowed_provider: Option<String>,
    pub require_confidence_bps: i32,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionEventResponse {
    pub id: String,
    pub node_id: Option<String>,
    pub kind: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionCellListItem {
    pub id: String,
    pub title: String,
    pub statement: String,
    pub decision_type: String,
    pub horizon_at: Option<String>,
    pub status: String,
    pub automation_enabled: bool,
    pub linked_market_refs: Vec<String>,
    pub recommendation: DecisionRecommendation,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionCellResponse {
    pub id: String,
    pub owner: String,
    pub title: String,
    pub statement: String,
    pub decision_type: String,
    pub horizon_at: Option<String>,
    pub status: String,
    pub automation_enabled: bool,
    pub recommendation: DecisionRecommendation,
    pub actions: Vec<DecisionActionResponse>,
    pub nodes: Vec<DecisionNodeResponse>,
    pub alerts: Vec<DecisionAlertResponse>,
    pub automation_policy: DecisionAutomationPolicyResponse,
    pub events: Vec<DecisionEventResponse>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionCellsListResponse {
    pub data: Vec<DecisionCellListItem>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionRunnerTickResponse {
    pub scanned: u64,
    pub recalculated: u64,
    pub automations_triggered: u64,
    pub skipped: BTreeMap<String, u64>,
}

#[derive(Debug, Clone)]
struct CellGraph {
    cell: DecisionCellRecord,
    actions: Vec<DecisionActionRecord>,
    nodes: Vec<DecisionNodeRecord>,
    alerts: Vec<DecisionAlertRecord>,
    policy: DecisionAutomationPolicyRecord,
    node_agents: Vec<DecisionNodeAgentRecord>,
}

#[derive(Debug, Clone)]
struct ResolvedNodeMarket {
    probability_bps: i32,
    snapshot: Value,
}

#[derive(Debug, Clone, Default)]
struct RecalculationFlags {
    recommendation_changed: bool,
    threshold_crossed: bool,
    confidence_dropped: bool,
    confidence_gain: bool,
}

#[derive(Debug, Clone)]
struct RecalculateResult {
    graph: CellGraph,
    recommendation: DecisionRecommendation,
    automations_triggered: u64,
    skipped: BTreeMap<String, u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDecisionCellsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDecisionCellRequest {
    pub title: String,
    pub statement: String,
    pub decision_type: String,
    pub horizon_at: Option<String>,
    pub actions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDecisionCellRequest {
    pub title: Option<String>,
    pub statement: Option<String>,
    pub horizon_at: Option<String>,
    pub status: Option<String>,
    pub automation_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDecisionActionRequest {
    pub label: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDecisionNodeRequest {
    pub label: String,
    pub description: Option<String>,
    pub weight_bps: Option<i32>,
    pub source_type: Option<String>,
    pub source_ref: Option<String>,
    pub status: Option<String>,
    pub action_effects: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDecisionNodeRequest {
    pub label: Option<String>,
    pub description: Option<String>,
    pub weight_bps: Option<i32>,
    pub source_type: Option<String>,
    pub source_ref: Option<String>,
    pub status: Option<String>,
    pub action_effects: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachDecisionMarketRequest {
    pub source_type: String,
    pub source_ref: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachDecisionAgentRequest {
    pub external_agent_id: String,
    pub trigger_mode: String,
    pub active: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDecisionAutomationRequest {
    pub automation_enabled: Option<bool>,
    pub max_agent_notional_usdc: Option<f64>,
    pub max_triggers_per_day: Option<i32>,
    pub min_trigger_interval_seconds: Option<i64>,
    pub allowed_provider: Option<String>,
    pub require_confidence_bps: Option<i32>,
    pub active: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertDecisionAlertRequest {
    pub kind: String,
    pub threshold: Option<Value>,
    pub active: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionRunnerTickRequest {
    pub limit: Option<i64>,
}

#[derive(Debug, Clone)]
struct StarterNodeTemplate {
    label: &'static str,
    description: &'static str,
}

fn parse_datetime(value: Option<&str>) -> Result<Option<DateTime<Utc>>, ApiError> {
    let Some(raw) = value else {
        return Ok(None);
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let parsed = DateTime::parse_from_rfc3339(trimmed)
        .map_err(|_| ApiError::bad_request("INVALID_DATETIME", "timestamp must be RFC3339"))?;
    Ok(Some(parsed.with_timezone(&Utc)))
}

fn clean_required(value: &str, field: &str, max_len: usize) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request("INVALID_INPUT", &format!("{field} is required")));
    }
    if trimmed.len() > max_len {
        return Err(ApiError::bad_request(
            "INVALID_INPUT",
            &format!("{field} must be at most {max_len} characters"),
        ));
    }
    Ok(trimmed.to_string())
}

fn clean_optional(value: Option<&str>, max_len: usize) -> Result<Option<String>, ApiError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > max_len {
        return Err(ApiError::bad_request(
            "INVALID_INPUT",
            &format!("value must be at most {max_len} characters"),
        ));
    }
    Ok(Some(trimmed.to_string()))
}

fn normalize_decision_type(raw: &str) -> Result<String, ApiError> {
    let value = raw.trim().to_ascii_lowercase();
    match value.as_str() {
        DECISION_TYPE_TIMING | DECISION_TYPE_CHOICE | DECISION_TYPE_HEDGE | DECISION_TYPE_ALLOCATION => Ok(value),
        _ => Err(ApiError::bad_request(
            "INVALID_DECISION_TYPE",
            "decision type must be one of: timing, choice, hedge, allocation",
        )),
    }
}

fn normalize_source_type(raw: &str) -> Result<String, ApiError> {
    let value = raw.trim().to_ascii_lowercase();
    match value.as_str() {
        SOURCE_TYPE_INTERNAL_MARKET | SOURCE_TYPE_EXTERNAL_MARKET | SOURCE_TYPE_DRAFT_MARKET => {
            Ok(value)
        }
        _ => Err(ApiError::bad_request(
            "INVALID_SOURCE_TYPE",
            "source type must be one of: internal_market, external_market, draft_market",
        )),
    }
}

fn normalize_trigger_mode(raw: &str) -> Result<String, ApiError> {
    let value = raw.trim().to_ascii_lowercase();
    match value.as_str() {
        TRIGGER_ON_RECOMMENDATION_GAIN | TRIGGER_ON_THRESHOLD_CROSS | TRIGGER_ON_CONFIDENCE_GAIN => Ok(value),
        _ => Err(ApiError::bad_request(
            "INVALID_TRIGGER_MODE",
            "trigger mode must be one of: on_recommendation_gain, on_threshold_cross, on_confidence_gain",
        )),
    }
}

fn normalize_effect(raw: &str) -> Result<String, ApiError> {
    let value = raw.trim().to_ascii_lowercase();
    match value.as_str() {
        EFFECT_SUPPORT | EFFECT_OPPOSE | EFFECT_NEUTRAL => Ok(value),
        _ => Err(ApiError::bad_request(
            "INVALID_ACTION_EFFECT",
            "action effect must be support, oppose, or neutral",
        )),
    }
}

fn normalize_actions(decision_type: &str, actions: Option<Vec<String>>) -> Result<Vec<String>, ApiError> {
    let provided = actions.unwrap_or_default();
    if decision_type == DECISION_TYPE_TIMING {
        if provided.is_empty() {
            return Ok(vec!["act now".to_string(), "wait".to_string()]);
        }
    }

    let cleaned = provided
        .into_iter()
        .map(|entry| clean_required(entry.as_str(), "action label", 120))
        .collect::<Result<Vec<_>, _>>()?;

    let normalized_unique = cleaned
        .iter()
        .map(|entry| entry.to_ascii_lowercase())
        .collect::<std::collections::HashSet<_>>();

    if normalized_unique.len() != cleaned.len() {
        return Err(ApiError::bad_request(
            "INVALID_ACTIONS",
            "action labels must be unique",
        ));
    }

    if cleaned.len() < 2 || cleaned.len() > 3 {
        return Err(ApiError::bad_request(
            "INVALID_ACTIONS",
            "decision cells require 2 or 3 actions",
        ));
    }

    Ok(cleaned)
}

fn action_effect_map(action_labels: &[String], supported_index: Option<usize>, opposed_index: Option<usize>) -> Value {
    let mut object = serde_json::Map::new();
    for (index, label) in action_labels.iter().enumerate() {
        let effect = if Some(index) == supported_index {
            EFFECT_SUPPORT
        } else if Some(index) == opposed_index {
            EFFECT_OPPOSE
        } else {
            EFFECT_NEUTRAL
        };
        object.insert(label.clone(), Value::String(effect.to_string()));
    }
    Value::Object(object)
}

fn starter_templates(decision_type: &str) -> &'static [StarterNodeTemplate] {
    match decision_type {
        DECISION_TYPE_TIMING => &[
            StarterNodeTemplate {
                label: "Catalyst confirmed",
                description: "The positive trigger required to act is verified in the market.",
            },
            StarterNodeTemplate {
                label: "Negative blocker emerges",
                description: "A blocking event increases the probability that waiting is safer.",
            },
            StarterNodeTemplate {
                label: "Broader trend persists",
                description: "The surrounding market regime continues to support the same timing decision.",
            },
        ],
        DECISION_TYPE_CHOICE => &[
            StarterNodeTemplate {
                label: "Outcome driver A",
                description: "Primary driver that would validate the leading choice.",
            },
            StarterNodeTemplate {
                label: "Cost or risk driver",
                description: "A cost, downside, or fragility signal that can flip the choice.",
            },
            StarterNodeTemplate {
                label: "External validation",
                description: "Independent confirmation that the preferred path is gaining support.",
            },
        ],
        DECISION_TYPE_HEDGE => &[
            StarterNodeTemplate {
                label: "Downside event",
                description: "The key adverse scenario the hedge is intended to offset.",
            },
            StarterNodeTemplate {
                label: "Hedge cost pressure",
                description: "A signal that the hedge itself is becoming too expensive or inefficient.",
            },
            StarterNodeTemplate {
                label: "Correlation breakdown",
                description: "A signal that the intended hedge relationship is weakening.",
            },
        ],
        DECISION_TYPE_ALLOCATION => &[
            StarterNodeTemplate {
                label: "Upside catalyst",
                description: "The strongest driver in favor of increasing the allocation.",
            },
            StarterNodeTemplate {
                label: "Downside catalyst",
                description: "The strongest driver in favor of reducing or avoiding the allocation.",
            },
            StarterNodeTemplate {
                label: "Liquidity or exit condition",
                description: "The condition that determines whether the allocation can be changed safely.",
            },
        ],
        _ => &[],
    }
}

fn build_starter_nodes(decision_type: &str, actions: &[String]) -> Vec<(StarterNodeTemplate, Value)> {
    starter_templates(decision_type)
        .iter()
        .enumerate()
        .map(|(index, template)| {
            let effects = if decision_type == DECISION_TYPE_TIMING {
                match index {
                    0 => action_effect_map(actions, Some(0), Some(1)),
                    1 => action_effect_map(actions, Some(1), Some(0)),
                    _ => action_effect_map(actions, Some(0), None),
                }
            } else {
                action_effect_map(actions, Some(index % actions.len()), None)
            };
            (template.clone(), effects)
        })
        .collect()
}

fn parse_cell_record(row: sqlx::postgres::PgRow) -> Result<DecisionCellRecord, ApiError> {
    Ok(DecisionCellRecord {
        id: row.try_get("id").map_err(|err| ApiError::internal(&err.to_string()))?,
        owner: row.try_get("owner").map_err(|err| ApiError::internal(&err.to_string()))?,
        title: row.try_get("title").map_err(|err| ApiError::internal(&err.to_string()))?,
        statement: row
            .try_get("statement")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        decision_type: row
            .try_get("decision_type")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        horizon_at: row.try_get("horizon_at").ok(),
        status: row.try_get("status").map_err(|err| ApiError::internal(&err.to_string()))?,
        automation_enabled: row
            .try_get("automation_enabled")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        current_recommendation: row
            .try_get("current_recommendation")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        confidence_bps: row
            .try_get("confidence_bps")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        summary: row.try_get("summary").unwrap_or_else(|_| json!({})),
        created_at: row
            .try_get("created_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        updated_at: row
            .try_get("updated_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
    })
}

fn parse_action_record(row: sqlx::postgres::PgRow) -> Result<DecisionActionRecord, ApiError> {
    Ok(DecisionActionRecord {
        id: row.try_get("id").map_err(|err| ApiError::internal(&err.to_string()))?,
        label: row.try_get("label").map_err(|err| ApiError::internal(&err.to_string()))?,
        rank: row.try_get("rank").map_err(|err| ApiError::internal(&err.to_string()))?,
    })
}

fn parse_node_record(row: sqlx::postgres::PgRow) -> Result<DecisionNodeRecord, ApiError> {
    Ok(DecisionNodeRecord {
        id: row.try_get("id").map_err(|err| ApiError::internal(&err.to_string()))?,
        label: row.try_get("label").map_err(|err| ApiError::internal(&err.to_string()))?,
        description: row.try_get("description").unwrap_or_default(),
        weight_bps: row.try_get("weight_bps").map_err(|err| ApiError::internal(&err.to_string()))?,
        source_type: row.try_get("source_type").map_err(|err| ApiError::internal(&err.to_string()))?,
        source_ref: row.try_get("source_ref").ok(),
        status: row.try_get("status").map_err(|err| ApiError::internal(&err.to_string()))?,
        last_probability_bps: row.try_get("last_probability_bps").ok(),
        last_market_snapshot: row.try_get("last_market_snapshot").unwrap_or_else(|_| json!({})),
        action_effects: row.try_get("action_effects").unwrap_or_else(|_| json!({})),
        created_at: row.try_get("created_at").map_err(|err| ApiError::internal(&err.to_string()))?,
        updated_at: row.try_get("updated_at").map_err(|err| ApiError::internal(&err.to_string()))?,
    })
}

fn parse_alert_record(row: sqlx::postgres::PgRow) -> Result<DecisionAlertRecord, ApiError> {
    Ok(DecisionAlertRecord {
        id: row.try_get("id").map_err(|err| ApiError::internal(&err.to_string()))?,
        kind: row.try_get("kind").map_err(|err| ApiError::internal(&err.to_string()))?,
        threshold: row.try_get("threshold").unwrap_or_else(|_| json!({})),
        active: row.try_get("active").map_err(|err| ApiError::internal(&err.to_string()))?,
        last_triggered_at: row.try_get("last_triggered_at").ok(),
    })
}

fn parse_policy_record(row: Option<sqlx::postgres::PgRow>, _cell_id: &str) -> Result<DecisionAutomationPolicyRecord, ApiError> {
    if let Some(row) = row {
        return Ok(DecisionAutomationPolicyRecord {
            max_agent_notional_usdc: row
                .try_get("max_agent_notional_usdc")
                .map_err(|err| ApiError::internal(&err.to_string()))?,
            max_triggers_per_day: row
                .try_get("max_triggers_per_day")
                .map_err(|err| ApiError::internal(&err.to_string()))?,
            min_trigger_interval_seconds: row
                .try_get("min_trigger_interval_seconds")
                .map_err(|err| ApiError::internal(&err.to_string()))?,
            allowed_provider: row.try_get("allowed_provider").ok(),
            require_confidence_bps: row
                .try_get("require_confidence_bps")
                .map_err(|err| ApiError::internal(&err.to_string()))?,
            active: row.try_get("active").map_err(|err| ApiError::internal(&err.to_string()))?,
        });
    }

    Ok(DecisionAutomationPolicyRecord {
        max_agent_notional_usdc: 0.0,
        max_triggers_per_day: 0,
        min_trigger_interval_seconds: 0,
        allowed_provider: None,
        require_confidence_bps: 0,
        active: false,
    })
}

fn parse_node_agent_record(row: sqlx::postgres::PgRow) -> Result<DecisionNodeAgentRecord, ApiError> {
    Ok(DecisionNodeAgentRecord {
        id: row.try_get("id").map_err(|err| ApiError::internal(&err.to_string()))?,
        node_id: row.try_get("node_id").map_err(|err| ApiError::internal(&err.to_string()))?,
        external_agent_id: row
            .try_get("external_agent_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        trigger_mode: row
            .try_get("trigger_mode")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        active: row.try_get("active").map_err(|err| ApiError::internal(&err.to_string()))?,
        agent_name: row.try_get("agent_name").ok(),
        provider: row.try_get("provider").ok(),
        agent_active: row.try_get("agent_active").ok(),
    })
}

fn parse_event_response(row: sqlx::postgres::PgRow) -> Result<DecisionEventResponse, ApiError> {
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(|err| ApiError::internal(&err.to_string()))?;
    Ok(DecisionEventResponse {
        id: row.try_get("id").map_err(|err| ApiError::internal(&err.to_string()))?,
        node_id: row.try_get("node_id").ok(),
        kind: row.try_get("kind").map_err(|err| ApiError::internal(&err.to_string()))?,
        payload: row.try_get("payload").unwrap_or_else(|_| json!({})),
        created_at: created_at.to_rfc3339(),
    })
}

fn summary_from_value(value: &Value) -> DecisionSummary {
    serde_json::from_value(value.clone()).unwrap_or_default()
}

fn normalize_action_effects(value: Option<Value>, actions: &[DecisionActionRecord]) -> Result<Value, ApiError> {
    let Some(payload) = value else {
        let mut map = serde_json::Map::new();
        for action in actions {
            map.insert(action.label.clone(), Value::String(EFFECT_NEUTRAL.to_string()));
        }
        return Ok(Value::Object(map));
    };

    let object = payload.as_object().ok_or_else(|| {
        ApiError::bad_request(
            "INVALID_ACTION_EFFECTS",
            "actionEffects must be an object keyed by action label",
        )
    })?;

    let allowed = actions
        .iter()
        .map(|action| action.label.clone())
        .collect::<std::collections::HashSet<_>>();
    let mut normalized = serde_json::Map::new();

    for action in actions {
        let effect = object
            .get(action.label.as_str())
            .and_then(Value::as_str)
            .unwrap_or(EFFECT_NEUTRAL);
        normalized.insert(action.label.clone(), Value::String(normalize_effect(effect)?));
    }

    for key in object.keys() {
        if !allowed.contains(key) {
            return Err(ApiError::bad_request(
                "INVALID_ACTION_EFFECTS",
                "actionEffects contains an unknown action label",
            ));
        }
    }

    Ok(Value::Object(normalized))
}

fn action_effect_for_label<'a>(action_effects: &'a Value, label: &str) -> &'a str {
    action_effects
        .get(label)
        .and_then(Value::as_str)
        .unwrap_or(EFFECT_NEUTRAL)
}

fn recommendation_from_timing_label(label: &str) -> String {
    match label.trim().to_ascii_lowercase().as_str() {
        "act now" => RECOMMENDATION_ACT_NOW.to_string(),
        "wait" => RECOMMENDATION_WAIT.to_string(),
        _ => label.to_string(),
    }
}

async fn ensure_internal_market_exists(state: &AppState, market_id: &str) -> Result<(), ApiError> {
    let exists = sqlx::query("SELECT id FROM markets WHERE id = $1")
        .bind(market_id)
        .fetch_optional(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?
        .is_some();

    if !exists {
        return Err(ApiError::not_found("Market"));
    }

    Ok(())
}

async fn ensure_cell_exists_for_owner(
    state: &AppState,
    cell_id: &str,
    owner: &str,
) -> Result<DecisionCellRecord, ApiError> {
    let row = sqlx::query(
        "SELECT id, owner, title, statement, decision_type, horizon_at, status,
                automation_enabled, current_recommendation, confidence_bps, summary,
                created_at, updated_at
         FROM decision_cells
         WHERE id = $1 AND owner = $2",
    )
    .bind(cell_id)
    .bind(owner)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    row.map(parse_cell_record)
        .transpose()?
        .ok_or_else(|| ApiError::not_found("Decision cell"))
}

async fn ensure_policy_row(state: &AppState, cell_id: &str) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO decision_automation_policies (
            cell_id, max_agent_notional_usdc, max_triggers_per_day,
            min_trigger_interval_seconds, allowed_provider, require_confidence_bps, active
         ) VALUES ($1, 0, 0, 0, NULL, 0, FALSE)
         ON CONFLICT (cell_id) DO NOTHING",
    )
    .bind(cell_id)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;
    Ok(())
}

async fn load_cell_graph(
    state: &AppState,
    cell_id: &str,
    owner: &str,
) -> Result<CellGraph, ApiError> {
    let cell = ensure_cell_exists_for_owner(state, cell_id, owner).await?;
    ensure_policy_row(state, cell_id).await?;

    let actions = sqlx::query(
        "SELECT id, cell_id, label, rank
         FROM decision_cell_actions
         WHERE cell_id = $1
         ORDER BY rank ASC",
    )
    .bind(cell_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?
    .into_iter()
    .map(parse_action_record)
    .collect::<Result<Vec<_>, _>>()?;

    let nodes = sqlx::query(
        "SELECT id, cell_id, label, description, weight_bps, source_type, source_ref,
                status, last_probability_bps, last_market_snapshot, action_effects,
                created_at, updated_at
         FROM decision_nodes
         WHERE cell_id = $1
         ORDER BY created_at ASC",
    )
    .bind(cell_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?
    .into_iter()
    .map(parse_node_record)
    .collect::<Result<Vec<_>, _>>()?;

    let alerts = sqlx::query(
        "SELECT id, cell_id, kind, threshold, active, last_triggered_at
         FROM decision_alerts
         WHERE cell_id = $1
         ORDER BY created_at ASC",
    )
    .bind(cell_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?
    .into_iter()
    .map(parse_alert_record)
    .collect::<Result<Vec<_>, _>>()?;

    let policy = parse_policy_record(
        sqlx::query(
            "SELECT cell_id, max_agent_notional_usdc, max_triggers_per_day,
                    min_trigger_interval_seconds, allowed_provider,
                    require_confidence_bps, active
             FROM decision_automation_policies
             WHERE cell_id = $1",
        )
        .bind(cell_id)
        .fetch_optional(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?,
        cell_id,
    )?;

    let node_agents = sqlx::query(
        "SELECT dna.id, dna.cell_id, dna.node_id, dna.external_agent_id, dna.trigger_mode,
                dna.active, ea.name AS agent_name, ea.provider, ea.active AS agent_active
         FROM decision_node_agents dna
         LEFT JOIN external_agents ea ON ea.id = dna.external_agent_id
         WHERE dna.cell_id = $1
         ORDER BY dna.created_at ASC",
    )
    .bind(cell_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?
    .into_iter()
    .map(parse_node_agent_record)
    .collect::<Result<Vec<_>, _>>()?;

    Ok(CellGraph {
        cell,
        actions,
        nodes,
        alerts,
        policy,
        node_agents,
    })
}

async fn list_cell_events(
    state: &AppState,
    cell_id: &str,
    limit: i64,
) -> Result<Vec<DecisionEventResponse>, ApiError> {
    sqlx::query(
        "SELECT id, node_id, kind, payload, created_at
         FROM decision_events
         WHERE cell_id = $1
         ORDER BY created_at DESC
         LIMIT $2",
    )
    .bind(cell_id)
    .bind(limit)
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?
    .into_iter()
    .map(parse_event_response)
    .collect::<Result<Vec<_>, _>>()
}

async fn insert_decision_event(
    state: &AppState,
    cell_id: &str,
    node_id: Option<&str>,
    kind: &str,
    payload: Value,
) -> Result<DecisionEventResponse, ApiError> {
    let id = Uuid::new_v4().to_string();
    let row = sqlx::query(
        "INSERT INTO decision_events (id, cell_id, node_id, kind, payload)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, node_id, kind, payload, created_at",
    )
    .bind(id.as_str())
    .bind(cell_id)
    .bind(node_id)
    .bind(kind)
    .bind(payload)
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;
    parse_event_response(row)
}

async fn resolve_node_market(
    state: &AppState,
    node: &DecisionNodeRecord,
) -> Result<Option<ResolvedNodeMarket>, ApiError> {
    match node.source_type.as_str() {
        SOURCE_TYPE_DRAFT_MARKET => Ok(None),
        SOURCE_TYPE_INTERNAL_MARKET => {
            let Some(source_ref) = node.source_ref.as_deref() else {
                return Ok(None);
            };
            let row = sqlx::query(
                "SELECT id, question, status, yes_price, no_price, category, trading_end, resolution_deadline
                 FROM markets WHERE id = $1",
            )
            .bind(source_ref)
            .fetch_optional(state.db.pool())
            .await
            .map_err(|err| ApiError::internal(&err.to_string()))?;

            let Some(row) = row else {
                return Ok(Some(ResolvedNodeMarket {
                    probability_bps: 0,
                    snapshot: json!({
                        "status": "missing",
                        "error": "market_not_found"
                    }),
                }));
            };

            let yes_price: f64 = row.try_get("yes_price").unwrap_or(0.5);
            let probability_bps = (yes_price.clamp(0.0, 1.0) * 10_000.0).round() as i32;
            Ok(Some(ResolvedNodeMarket {
                probability_bps,
                snapshot: json!({
                    "id": row.try_get::<String, _>("id").unwrap_or_default(),
                    "question": row.try_get::<String, _>("question").unwrap_or_default(),
                    "status": row.try_get::<i16, _>("status").unwrap_or_default(),
                    "yesPrice": yes_price,
                    "noPrice": row.try_get::<f64, _>("no_price").unwrap_or(1.0 - yes_price),
                    "category": row.try_get::<Option<String>, _>("category").ok().flatten(),
                    "tradingEnd": row.try_get::<Option<DateTime<Utc>>, _>("trading_end").ok().flatten().map(|value| value.to_rfc3339()),
                    "resolutionDeadline": row.try_get::<DateTime<Utc>, _>("resolution_deadline").ok().map(|value| value.to_rfc3339()),
                }),
            }))
        }
        SOURCE_TYPE_EXTERNAL_MARKET => {
            let Some(source_ref) = node.source_ref.as_deref() else {
                return Ok(None);
            };
            let market_id = match ExternalMarketId::parse(source_ref) {
                Ok(value) => value,
                Err(err) => {
                    return Ok(Some(ResolvedNodeMarket {
                        probability_bps: 0,
                        snapshot: json!({
                            "status": "invalid",
                            "error": err.code,
                            "message": err.message,
                        }),
                    }));
                }
            };
            match external::fetch_market_by_id(&state.config, &market_id).await {
                Ok(snapshot) => {
                    let probability_bps = (snapshot.yes_price.clamp(0.0, 1.0) * 10_000.0).round() as i32;
                    Ok(Some(ResolvedNodeMarket {
                        probability_bps,
                        snapshot: serde_json::to_value(snapshot)
                            .unwrap_or_else(|_| json!({ "status": "serialization_error" })),
                    }))
                }
                Err(err) => Ok(Some(ResolvedNodeMarket {
                    probability_bps: 0,
                    snapshot: json!({
                        "status": "unavailable",
                        "error": err.code,
                        "message": err.message,
                    }),
                })),
            }
        }
        _ => Ok(None),
    }
}

fn build_recommendation(
    cell: &DecisionCellRecord,
    actions: &[DecisionActionRecord],
    nodes: &[DecisionNodeRecord],
    previous_summary: &DecisionSummary,
    resolved_nodes: &HashMap<String, Option<ResolvedNodeMarket>>,
    now: DateTime<Utc>,
) -> DecisionRecommendation {
    let mut raw_scores = vec![0.0_f64; actions.len()];
    let mut active_weight_bps = 0_i32;
    let mut live_nodes = 0_usize;
    let mut top_contributors = Vec::new();
    let mut last_changed_node: Option<DecisionContributor> = None;
    let mut max_delta = -1_i32;

    for node in nodes {
        let Some(Some(resolved)) = resolved_nodes.get(node.id.as_str()) else {
            continue;
        };

        if matches!(node.source_type.as_str(), SOURCE_TYPE_DRAFT_MARKET) {
            continue;
        }

        if resolved.snapshot.get("error").is_some() {
            continue;
        }

        live_nodes += 1;
        active_weight_bps += node.weight_bps.max(0);
        let centered_signal = (resolved.probability_bps as f64 - 5_000.0) / 5_000.0;
        let weighted_signal = centered_signal * node.weight_bps as f64;
        let delta = node
            .last_probability_bps
            .map(|previous| (resolved.probability_bps - previous).abs())
            .unwrap_or_default();

        for (index, action) in actions.iter().enumerate() {
            let contribution = match action_effect_for_label(&node.action_effects, action.label.as_str()) {
                EFFECT_SUPPORT => weighted_signal,
                EFFECT_OPPOSE => -weighted_signal,
                _ => 0.0,
            };
            raw_scores[index] += contribution;
        }

        if delta >= max_delta {
            max_delta = delta;
            let top_action = actions.first().map(|entry| entry.label.clone()).unwrap_or_default();
            last_changed_node = Some(DecisionContributor {
                node_id: node.id.clone(),
                label: node.label.clone(),
                action_label: top_action,
                score_bps: 0,
                probability_bps: resolved.probability_bps,
                delta_bps: Some(delta),
                source_ref: node.source_ref.clone(),
            });
        }
    }

    let mut action_scores = actions
        .iter()
        .enumerate()
        .map(|(index, action)| {
            let score_bps = if active_weight_bps > 0 {
                ((raw_scores[index] / active_weight_bps as f64) * 10_000.0)
                    .round()
                    .clamp(-10_000.0, 10_000.0) as i32
            } else {
                0
            };
            DecisionActionScore {
                action_id: action.id.clone(),
                label: action.label.clone(),
                rank: action.rank,
                score_bps,
            }
        })
        .collect::<Vec<_>>();

    action_scores.sort_by(|left, right| right.score_bps.cmp(&left.score_bps).then(left.rank.cmp(&right.rank)));
    let total_nodes = nodes.len();
    let live_ratio = if total_nodes == 0 {
        0.0
    } else {
        live_nodes as f64 / total_nodes as f64
    };
    let confidence_bps = ((active_weight_bps as f64) * live_ratio)
        .round()
        .clamp(0.0, 10_000.0) as i32;

    let top_lead = if action_scores.len() >= 2 {
        action_scores[0].score_bps - action_scores[1].score_bps
    } else {
        0
    };

    let state = if live_nodes < MIN_LIVE_NODES_FOR_SIGNAL || top_lead < RECOMMENDATION_LEAD_THRESHOLD_BPS {
        RECOMMENDATION_INSUFFICIENT_SIGNAL.to_string()
    } else if cell.decision_type == DECISION_TYPE_TIMING {
        recommendation_from_timing_label(action_scores[0].label.as_str())
    } else {
        action_scores[0].label.clone()
    };

    let winning_label = action_scores
        .first()
        .map(|entry| entry.label.clone())
        .unwrap_or_default();

    for node in nodes {
        let Some(Some(resolved)) = resolved_nodes.get(node.id.as_str()) else {
            continue;
        };
        if resolved.snapshot.get("error").is_some() {
            continue;
        }
        let centered_signal = (resolved.probability_bps as f64 - 5_000.0) / 5_000.0;
        let weighted_signal = centered_signal * node.weight_bps as f64;
        let contribution = match action_effect_for_label(&node.action_effects, winning_label.as_str()) {
            EFFECT_SUPPORT => weighted_signal,
            EFFECT_OPPOSE => -weighted_signal,
            _ => 0.0,
        };
        if contribution.abs() <= f64::EPSILON {
            continue;
        }
        top_contributors.push(DecisionContributor {
            node_id: node.id.clone(),
            label: node.label.clone(),
            action_label: winning_label.clone(),
            score_bps: ((contribution / active_weight_bps.max(1) as f64) * 10_000.0).round() as i32,
            probability_bps: resolved.probability_bps,
            delta_bps: node.last_probability_bps.map(|previous| resolved.probability_bps - previous),
            source_ref: node.source_ref.clone(),
        });
    }

    top_contributors.sort_by(|left, right| right.score_bps.abs().cmp(&left.score_bps.abs()));
    top_contributors.truncate(3);

    if let Some(last_changed) = last_changed_node.as_mut() {
        last_changed.action_label = winning_label.clone();
        last_changed.score_bps = top_contributors
            .iter()
            .find(|entry| entry.node_id == last_changed.node_id)
            .map(|entry| entry.score_bps)
            .unwrap_or_default();
    }

    let why_changed = if let Some(last_changed) = last_changed_node.as_ref() {
        if previous_summary.why_changed.is_empty() || cell.current_recommendation != state {
            format!(
                "{} moved to {:.1}% and pushed the cell toward {}.",
                last_changed.label,
                last_changed.probability_bps as f64 / 100.0,
                if state == RECOMMENDATION_INSUFFICIENT_SIGNAL {
                    "insufficient signal"
                } else {
                    state.as_str()
                }
            )
        } else {
            format!(
                "{} remains the most recent moving input at {:.1}%.",
                last_changed.label,
                last_changed.probability_bps as f64 / 100.0,
            )
        }
    } else if total_nodes == 0 {
        "Add nodes to start scoring this decision cell.".to_string()
    } else {
        "Link live markets to at least two nodes before the cell can issue a recommendation.".to_string()
    };

    let summary = DecisionSummary {
        action_scores: action_scores.clone(),
        top_contributors: top_contributors.clone(),
        why_changed: why_changed.clone(),
        live_nodes,
        total_nodes,
        top_action_lead_bps: top_lead.max(0),
        last_recalculated_at: now.to_rfc3339(),
        last_changed_node: last_changed_node.clone(),
    };

    DecisionRecommendation {
        state,
        confidence_bps,
        why_changed,
        live_nodes,
        total_nodes,
        top_action_lead_bps: summary.top_action_lead_bps,
        action_scores,
        top_contributors,
        last_changed_node,
    }
}

async fn persist_node_snapshots(
    state: &AppState,
    graph: &CellGraph,
    resolved_nodes: &HashMap<String, Option<ResolvedNodeMarket>>,
) -> Result<(), ApiError> {
    for node in &graph.nodes {
        let Some(value) = resolved_nodes.get(node.id.as_str()) else {
            continue;
        };
        let (probability, snapshot) = if let Some(resolved) = value {
            (
                Some(resolved.probability_bps),
                resolved.snapshot.clone(),
            )
        } else {
            (None, json!({ "status": "draft" }))
        };
        sqlx::query(
            "UPDATE decision_nodes
             SET last_probability_bps = $2,
                 last_market_snapshot = $3,
                 status = $4
             WHERE id = $1",
        )
        .bind(node.id.as_str())
        .bind(probability)
        .bind(snapshot)
        .bind(if matches!(node.source_type.as_str(), SOURCE_TYPE_DRAFT_MARKET) {
            NODE_STATUS_DRAFT
        } else {
            NODE_STATUS_LIVE
        })
        .execute(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    }
    Ok(())
}

async fn update_cell_summary(
    state: &AppState,
    cell_id: &str,
    recommendation: &DecisionRecommendation,
) -> Result<(), ApiError> {
    let summary = DecisionSummary {
        action_scores: recommendation.action_scores.clone(),
        top_contributors: recommendation.top_contributors.clone(),
        why_changed: recommendation.why_changed.clone(),
        live_nodes: recommendation.live_nodes,
        total_nodes: recommendation.total_nodes,
        top_action_lead_bps: recommendation.top_action_lead_bps,
        last_recalculated_at: Utc::now().to_rfc3339(),
        last_changed_node: recommendation.last_changed_node.clone(),
    };

    sqlx::query(
        "UPDATE decision_cells
         SET current_recommendation = $2,
             confidence_bps = $3,
             summary = $4
         WHERE id = $1",
    )
    .bind(cell_id)
    .bind(recommendation.state.as_str())
    .bind(recommendation.confidence_bps)
    .bind(serde_json::to_value(summary).unwrap_or_else(|_| json!({})))
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(())
}

async fn evaluate_and_record_alerts(
    state: &AppState,
    graph: &CellGraph,
    recommendation: &DecisionRecommendation,
    previous_summary: &DecisionSummary,
    previous_recommendation: &str,
    resolved_nodes: &HashMap<String, Option<ResolvedNodeMarket>>,
) -> Result<(Vec<DecisionEventResponse>, RecalculationFlags), ApiError> {
    let mut events = Vec::new();
    let mut flags = RecalculationFlags::default();

    if previous_recommendation != recommendation.state {
        flags.recommendation_changed = true;
        let event = insert_decision_event(
            state,
            graph.cell.id.as_str(),
            None,
            EVENT_RECOMMENDATION_CHANGED,
            json!({
                "previous": previous_recommendation,
                "current": recommendation.state,
                "confidenceBps": recommendation.confidence_bps,
            }),
        )
        .await?;
        create_notification(
            state,
            NewNotification {
                owner: graph.cell.owner.clone(),
                kind: NotificationType::DecisionRecommendationChanged,
                title: format!("{} recommendation changed", graph.cell.title),
                message: recommendation.why_changed.clone(),
                market_id: None,
                order_id: None,
                decision_cell_id: Some(graph.cell.id.clone()),
                metadata: json!({
                    "previous": previous_recommendation,
                    "current": recommendation.state,
                }),
            },
        )
        .await?;
        events.push(event);
    }

    if previous_summary.top_action_lead_bps < RECOMMENDATION_LEAD_THRESHOLD_BPS
        && recommendation.top_action_lead_bps >= RECOMMENDATION_LEAD_THRESHOLD_BPS
    {
        flags.threshold_crossed = true;
    }

    let previous_confidence_bps = graph.cell.confidence_bps;

    if previous_confidence_bps > 0 && recommendation.confidence_bps < previous_confidence_bps {
        flags.confidence_dropped = true;
    }

    if recommendation.confidence_bps > previous_confidence_bps {
        flags.confidence_gain = true;
    }

    for alert in &graph.alerts {
        if !alert.active {
            continue;
        }

        let mut fired = false;
        let mut event_kind = EVENT_THRESHOLD_CROSSED;
        let mut payload = json!({ "kind": alert.kind });
        match alert.kind.as_str() {
            "recommendation_changed" => {
                fired = previous_recommendation != recommendation.state;
                event_kind = EVENT_RECOMMENDATION_CHANGED;
                payload = json!({
                    "previous": previous_recommendation,
                    "current": recommendation.state,
                });
            }
            "confidence_below" => {
                let threshold = alert
                    .threshold
                    .get("bps")
                    .and_then(Value::as_i64)
                    .unwrap_or(0) as i32;
                let previous = graph.cell.confidence_bps;
                fired = previous >= threshold && recommendation.confidence_bps < threshold;
                if fired {
                    event_kind = EVENT_CONFIDENCE_DROPPED;
                    payload = json!({
                        "thresholdBps": threshold,
                        "currentConfidenceBps": recommendation.confidence_bps,
                    });
                    flags.confidence_dropped = true;
                }
            }
            "action_lead_above" => {
                let threshold = alert
                    .threshold
                    .get("bps")
                    .and_then(Value::as_i64)
                    .unwrap_or(RECOMMENDATION_LEAD_THRESHOLD_BPS as i64)
                    as i32;
                fired = previous_summary.top_action_lead_bps < threshold
                    && recommendation.top_action_lead_bps >= threshold;
                if fired {
                    payload = json!({
                        "thresholdBps": threshold,
                        "topActionLeadBps": recommendation.top_action_lead_bps,
                    });
                    flags.threshold_crossed = true;
                }
            }
            "node_probability_cross" => {
                let node_id = alert
                    .threshold
                    .get("nodeId")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let threshold = alert
                    .threshold
                    .get("bps")
                    .and_then(Value::as_i64)
                    .unwrap_or(0) as i32;
                let direction = alert
                    .threshold
                    .get("direction")
                    .and_then(Value::as_str)
                    .unwrap_or("above");
                if let Some(node) = graph.nodes.iter().find(|entry| entry.id == node_id) {
                    let previous = node.last_probability_bps.unwrap_or_default();
                    let current = resolved_nodes
                        .get(node.id.as_str())
                        .and_then(|entry| entry.as_ref())
                        .map(|entry| entry.probability_bps)
                        .unwrap_or_default();
                    fired = match direction {
                        "below" => previous >= threshold && current < threshold,
                        _ => previous < threshold && current >= threshold,
                    };
                    if fired {
                        payload = json!({
                            "nodeId": node.id,
                            "nodeLabel": node.label,
                            "thresholdBps": threshold,
                            "currentProbabilityBps": current,
                            "direction": direction,
                        });
                        flags.threshold_crossed = true;
                    }
                }
            }
            _ => {}
        }

        if !fired {
            continue;
        }

        sqlx::query(
            "UPDATE decision_alerts SET last_triggered_at = NOW() WHERE id = $1",
        )
        .bind(alert.id.as_str())
        .execute(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

        let event = insert_decision_event(state, graph.cell.id.as_str(), None, event_kind, payload.clone()).await?;
        let notification_kind = match event_kind {
            EVENT_RECOMMENDATION_CHANGED => NotificationType::DecisionRecommendationChanged,
            EVENT_CONFIDENCE_DROPPED => NotificationType::DecisionConfidenceDropped,
            _ => NotificationType::DecisionThresholdCrossed,
        };
        create_notification(
            state,
            NewNotification {
                owner: graph.cell.owner.clone(),
                kind: notification_kind,
                title: format!("{} alert fired", graph.cell.title),
                message: recommendation.why_changed.clone(),
                market_id: None,
                order_id: None,
                decision_cell_id: Some(graph.cell.id.clone()),
                metadata: payload,
            },
        )
        .await?;
        events.push(event);
    }

    Ok((events, flags))
}

async fn maybe_run_automation(
    state: &AppState,
    graph: &CellGraph,
    recommendation: &DecisionRecommendation,
    flags: &RecalculationFlags,
) -> Result<(Vec<DecisionEventResponse>, u64, BTreeMap<String, u64>), ApiError> {
    let mut events = Vec::new();
    let mut executed = 0_u64;
    let mut skipped = BTreeMap::new();

    if !graph.cell.automation_enabled || !graph.policy.active {
        return Ok((events, executed, skipped));
    }

    if recommendation.state == RECOMMENDATION_INSUFFICIENT_SIGNAL {
        skipped.insert("insufficient_signal".to_string(), 1);
        return Ok((events, executed, skipped));
    }

    if recommendation.confidence_bps < graph.policy.require_confidence_bps {
        skipped.insert("confidence_too_low".to_string(), 1);
        return Ok((events, executed, skipped));
    }

    let has_new_event = flags.recommendation_changed || flags.threshold_crossed || flags.confidence_gain;
    if !has_new_event {
        skipped.insert("steady_state".to_string(), 1);
        return Ok((events, executed, skipped));
    }

    let last_success = sqlx::query(
        "SELECT created_at FROM decision_events
         WHERE cell_id = $1 AND kind = $2
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(graph.cell.id.as_str())
    .bind(EVENT_AUTOMATION_TRIGGERED)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    if let Some(row) = last_success {
        let created_at: DateTime<Utc> = row.try_get("created_at").map_err(|err| ApiError::internal(&err.to_string()))?;
        if graph.policy.min_trigger_interval_seconds > 0
            && Utc::now() < created_at + Duration::seconds(graph.policy.min_trigger_interval_seconds)
        {
            skipped.insert("cooldown_active".to_string(), 1);
            return Ok((events, executed, skipped));
        }
    }

    if graph.policy.max_triggers_per_day > 0 {
        let today_count = sqlx::query(
            "SELECT COUNT(*)::BIGINT AS total
             FROM decision_events
             WHERE cell_id = $1 AND kind = $2 AND created_at >= date_trunc('day', NOW())",
        )
        .bind(graph.cell.id.as_str())
        .bind(EVENT_AUTOMATION_TRIGGERED)
        .fetch_one(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?
        .try_get::<i64, _>("total")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
        if today_count >= graph.policy.max_triggers_per_day as i64 {
            skipped.insert("daily_cap_reached".to_string(), 1);
            return Ok((events, executed, skipped));
        }
    }

    for attachment in &graph.node_agents {
        if !attachment.active {
            continue;
        }

        let trigger_matches = match attachment.trigger_mode.as_str() {
            TRIGGER_ON_RECOMMENDATION_GAIN => flags.recommendation_changed,
            TRIGGER_ON_CONFIDENCE_GAIN => flags.confidence_gain,
            _ => flags.threshold_crossed,
        };
        if !trigger_matches {
            continue;
        }

        let agent = match load_external_agent_for_owner(
            state,
            attachment.external_agent_id.as_str(),
            graph.cell.owner.as_str(),
        )
        .await
        {
            Ok(agent) => agent,
            Err(err) => {
                let reason = skip_reason_from_error(&err);
                *skipped.entry(reason.clone()).or_insert(0) += 1;
                events.push(
                    insert_decision_event(
                        state,
                        graph.cell.id.as_str(),
                        Some(attachment.node_id.as_str()),
                        EVENT_AUTOMATION_SKIPPED,
                        json!({
                            "externalAgentId": attachment.external_agent_id,
                            "reason": reason,
                            "message": err.message,
                        }),
                    )
                    .await?,
                );
                continue;
            }
        };

        if !agent.active {
            *skipped.entry("agent_inactive".to_string()).or_insert(0) += 1;
            events.push(
                insert_decision_event(
                    state,
                    graph.cell.id.as_str(),
                    Some(attachment.node_id.as_str()),
                    EVENT_AUTOMATION_SKIPPED,
                    json!({
                        "externalAgentId": agent.id,
                        "reason": "agent_inactive",
                    }),
                )
                .await?,
            );
            continue;
        }

        if let Some(allowed_provider) = graph.policy.allowed_provider.as_deref() {
            if agent.provider.as_str() != allowed_provider {
                *skipped.entry("provider_not_allowed".to_string()).or_insert(0) += 1;
                events.push(
                    insert_decision_event(
                        state,
                        graph.cell.id.as_str(),
                        Some(attachment.node_id.as_str()),
                        EVENT_AUTOMATION_SKIPPED,
                        json!({
                            "externalAgentId": agent.id,
                            "provider": agent.provider.as_str(),
                            "allowedProvider": allowed_provider,
                            "reason": "provider_not_allowed",
                        }),
                    )
                    .await?,
                );
                continue;
            }
        }

        let notional_usdc = (agent.price.max(0.0) * agent.quantity.max(0.0)).abs();
        if graph.policy.max_agent_notional_usdc > 0.0
            && notional_usdc > graph.policy.max_agent_notional_usdc
        {
            *skipped.entry("notional_too_large".to_string()).or_insert(0) += 1;
            events.push(
                insert_decision_event(
                    state,
                    graph.cell.id.as_str(),
                    Some(attachment.node_id.as_str()),
                    EVENT_AUTOMATION_SKIPPED,
                    json!({
                        "externalAgentId": agent.id,
                        "notionalUsdc": notional_usdc,
                        "maxAgentNotionalUsdc": graph.policy.max_agent_notional_usdc,
                        "reason": "notional_too_large",
                    }),
                )
