use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use std::sync::Arc;

use super::auth::{extract_jwt_user, AuthenticatedUserWithRole};
use super::jwt::UserRole;
use super::ApiError;
use crate::services::creator_economics::{self, TimeseriesWindow};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct CreatorScopeQuery {
    pub owner: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatorTimeseriesQuery {
    pub owner: Option<String>,
    pub window: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatorMaterializeRequest {
    pub owner: Option<String>,
    pub market_id: Option<u64>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorMaterializerRunRequest {
    pub owner: Option<String>,
    pub market_id: Option<u64>,
    pub window_days: Option<i64>,
    pub limit: Option<usize>,
}

fn normalize_wallet(value: &str) -> Result<String, ApiError> {
    let wallet = value.trim().to_ascii_lowercase();
    if wallet.len() != 42
        || !wallet.starts_with("0x")
        || !wallet[2..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(ApiError::bad_request(
            "INVALID_WALLET",
            "wallet must be a valid 0x EVM address",
        ));
    }
    Ok(wallet)
}

fn resolve_creator_scope(
    user: &AuthenticatedUserWithRole,
    requested_owner: Option<&str>,
) -> Result<String, ApiError> {
    let wallet = normalize_wallet(user.wallet_address.as_str())?;
    let requested = match requested_owner {
        Some(value) if !value.trim().is_empty() => Some(normalize_wallet(value)?),
        _ => None,
    };

    match requested {
        Some(requested) if matches!(user.role, UserRole::Admin) => Ok(requested),
        Some(requested) if requested == wallet => Ok(requested),
        Some(_) => Err(ApiError::forbidden(
            "creator economics are private to the authenticated wallet",
        )),
        None => Ok(wallet),
    }
}

fn ensure_materializer_admin(
    req: &HttpRequest,
    state: &web::Data<Arc<AppState>>,
) -> Result<(), ApiError> {
    let expected = state.config.admin_control_key.trim();
    if !expected.is_empty() {
        let provided = req
            .headers()
            .get("x-admin-key")
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .unwrap_or("");
        if provided == expected {
            return Ok(());
        }
    }

    let user = extract_jwt_user(req, state)?;
    if matches!(user.role, UserRole::Admin) {
        return Ok(());
    }

    Err(ApiError::forbidden(
        "creator economics materialization requires admin access",
    ))
}

fn ensure_creator_admin(
    req: &HttpRequest,
    state: &web::Data<Arc<AppState>>,
) -> Result<(), ApiError> {
    let expected = state.config.admin_control_key.trim();
    if !expected.is_empty() {
        let provided = req
            .headers()
            .get("x-admin-key")
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .unwrap_or("");
        if provided == expected {
            return Ok(());
        }
    }

    let user = extract_jwt_user(req, state)?;
    if matches!(user.role, UserRole::Admin) {
        Ok(())
    } else {
        Err(ApiError::forbidden("admin access required"))
    }
}

pub async fn get_creator_overview(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<CreatorScopeQuery>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let creator = resolve_creator_scope(&user, query.owner.as_deref())?;
    let overview = creator_economics::creator_overview(&state, creator.as_str()).await?;
    Ok(HttpResponse::Ok().json(overview))
}

pub async fn list_creator_markets(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<CreatorScopeQuery>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let creator = resolve_creator_scope(&user, query.owner.as_deref())?;
    let markets = creator_economics::creator_markets(&state, creator.as_str()).await?;
    Ok(HttpResponse::Ok().json(markets))
}

pub async fn get_creator_market_economics(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<u64>,
    query: web::Query<CreatorScopeQuery>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let creator = resolve_creator_scope(&user, query.owner.as_deref())?;
    let market =
        creator_economics::creator_market(&state, creator.as_str(), path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(market))
}

pub async fn get_creator_market_timeseries(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<u64>,
    query: web::Query<CreatorTimeseriesQuery>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let creator = resolve_creator_scope(&user, query.owner.as_deref())?;
    let window = TimeseriesWindow::parse(query.window.as_deref().unwrap_or("30d"))?;
    let response = creator_economics::creator_market_timeseries(
        &state,
        creator.as_str(),
        path.into_inner(),
        window,
    )
    .await?;
    Ok(HttpResponse::Ok().json(response))
}

pub async fn materialize_creator_economics_admin(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<CreatorMaterializeRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_materializer_admin(&req, &state)?;
    let response = creator_economics::materialize_creator_economics(
        &state,
        body.owner.as_deref(),
        body.market_id,
        body.limit,
    )
    .await?;
    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_creator_materializer_health(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_creator_admin(&req, &state)?;
    let health = creator_economics::creator_materializer_health(&state).await?;
    Ok(HttpResponse::Ok().json(health))
}

pub async fn run_creator_materializer(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: Option<web::Json<CreatorMaterializerRunRequest>>,
) -> Result<impl Responder, ApiError> {
    ensure_creator_admin(&req, &state)?;
    let body = body
        .map(web::Json::into_inner)
        .unwrap_or(CreatorMaterializerRunRequest {
            owner: None,
            market_id: None,
            window_days: None,
            limit: None,
        });
    let response = creator_economics::materialize_creator_market_rows(
        &state,
        body.owner.as_deref(),
        body.market_id,
        body.window_days.unwrap_or(30),
        body.limit,
    )
    .await?;
    Ok(HttpResponse::Ok().json(response))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/creator")
            .route(
                "/materializer/health",
                web::get().to(get_creator_materializer_health),
            )
            .route(
                "/materializer/run",
                web::post().to(run_creator_materializer),
            )
            .route("/overview", web::get().to(get_creator_overview))
            .route("/markets", web::get().to(list_creator_markets))
            .route(
                "/admin/materialize",
                web::post().to(materialize_creator_economics_admin),
            )
            .route(
                "/markets/{market_id}/economics",
                web::get().to(get_creator_market_economics),
            )
            .route(
                "/markets/{market_id}/timeseries",
                web::get().to(get_creator_market_timeseries),
            ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(wallet: &str, role: UserRole) -> AuthenticatedUserWithRole {
        AuthenticatedUserWithRole {
            wallet_address: wallet.to_string(),
            role,
        }
    }

    #[test]
    fn creator_scope_defaults_to_authenticated_wallet() {
        let user = user("0xabc0000000000000000000000000000000000000", UserRole::User);
        let resolved = resolve_creator_scope(&user, None).expect("scope");
        assert_eq!(resolved, "0xabc0000000000000000000000000000000000000");
    }

    #[test]
    fn creator_scope_rejects_other_wallet_for_non_admin() {
        let user = user("0xabc0000000000000000000000000000000000000", UserRole::User);
        let err = resolve_creator_scope(&user, Some("0xdef0000000000000000000000000000000000000"))
            .expect_err("forbidden");
        assert_eq!(err.code.as_str(), "FORBIDDEN");
    }

    #[test]
    fn creator_scope_allows_admin_override() {
        let user = user(
            "0xabc0000000000000000000000000000000000000",
            UserRole::Admin,
        );
        let resolved =
            resolve_creator_scope(&user, Some("0xdef0000000000000000000000000000000000000"))
                .expect("admin scope");
        assert_eq!(resolved, "0xdef0000000000000000000000000000000000000");
    }
}
