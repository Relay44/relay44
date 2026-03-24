use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::api::ApiError;
use crate::middleware;
use crate::services::database::ComplianceDecisionEntry;
use crate::services::provider_rails::{build_compliance_profile, ProviderCapabilities};
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompliancePolicyResponse {
    pub mode: String,
    pub blocked_countries: Vec<String>,
    pub writes_restricted: bool,
    pub country: Option<String>,
    pub region_class: String,
    pub routing_mode: String,
    pub rails: std::collections::BTreeMap<String, ProviderCapabilities>,
    pub legacy_close_only: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplianceDecisionRequest {
    pub request_id: Option<String>,
    pub wallet: Option<String>,
    pub country_code: Option<String>,
    pub action: String,
    pub route: String,
    pub method: String,
    pub decision: String,
    pub reason_code: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

fn ensure_admin(req: &HttpRequest, state: &AppState) -> Result<(), ApiError> {
    let expected = state.config.admin_control_key.trim();
    if expected.is_empty() {
        return Err(ApiError::forbidden(
            "admin control key is not configured for this environment",
        ));
    }

    let provided = req
        .headers()
        .get("x-admin-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or("");

    if provided != expected {
        return Err(ApiError::unauthorized("invalid admin key"));
    }

    Ok(())
}

pub async fn get_compliance_policy(
    req: HttpRequest,
    _state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let rails_profile = build_compliance_profile(&req);
    let blocked_countries = middleware::blocked_country_codes()
        .into_iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();

    Ok(HttpResponse::Ok().json(CompliancePolicyResponse {
        mode: "us_restricted_geofence".to_string(),
        blocked_countries,
        writes_restricted: true,
        country: rails_profile.country,
        region_class: rails_profile.region_class,
        routing_mode: rails_profile.mode,
        rails: rails_profile.rails,
        legacy_close_only: rails_profile.legacy_close_only,
    }))
}

pub async fn create_compliance_decision(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<ComplianceDecisionRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_admin(&req, &state)?;

    let action = body.action.trim().to_ascii_lowercase();
    let decision = body.decision.trim().to_ascii_lowercase();
    let reason_code = body.reason_code.trim().to_ascii_uppercase();
    if action.is_empty() || decision.is_empty() || reason_code.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_COMPLIANCE_DECISION",
            "action, decision, and reasonCode are required",
        ));
    }

    let entry = ComplianceDecisionEntry {
        request_id: body.request_id.as_deref(),
        wallet: body.wallet.as_deref(),
        country_code: body.country_code.as_deref(),
        action: action.as_str(),
        route: body.route.as_str(),
        method: body.method.as_str(),
        decision: decision.as_str(),
        reason_code: reason_code.as_str(),
        metadata: body.metadata.clone(),
    };

    state
        .db
        .record_compliance_decision(&entry)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "ok": true
    })))
}
