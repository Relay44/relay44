use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::api::auth::extract_jwt_user;
use crate::api::ApiError;
use crate::AppState;

// ── Follow endpoints ──────────────────────────────────────────────────

pub async fn follow_trader(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let follower = user.wallet_address.to_ascii_lowercase();
    let following = path.into_inner().to_ascii_lowercase();

    if follower == following {
        return Err(ApiError::bad_request(
            "CANNOT_FOLLOW_SELF",
            "You cannot follow yourself",
        ));
    }

    state
        .db
        .insert_follow(&follower, &following)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "ok": true })))
}

pub async fn unfollow_trader(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let follower = user.wallet_address.to_ascii_lowercase();
    let following = path.into_inner().to_ascii_lowercase();

    state
        .db
        .delete_follow(&follower, &following)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct PaginationParams {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

pub async fn get_following(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<PaginationParams>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let wallet = user.wallet_address.to_ascii_lowercase();
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0);

    let following = state
        .db
        .list_following(&wallet, limit, offset)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "following": following,
        "limit": limit,
        "offset": offset,
    })))
}

pub async fn get_followers(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<PaginationParams>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let wallet = user.wallet_address.to_ascii_lowercase();
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0);

    let followers = state
        .db
        .list_followers(&wallet, limit, offset)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "followers": followers,
        "limit": limit,
        "offset": offset,
    })))
}

pub async fn get_follower_counts(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let wallet = path.into_inner().to_ascii_lowercase();

    let counts = state
        .db
        .get_follower_counts(&wallet)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(counts))
}

pub async fn get_follow_status(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let follower = user.wallet_address.to_ascii_lowercase();
    let following = path.into_inner().to_ascii_lowercase();

    let is_following = state
        .db
        .check_follow(&follower, &following)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "following": is_following,
    })))
}

// ── Profile update ────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileRequest {
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub website_url: Option<String>,
    pub twitter_handle: Option<String>,
}

pub async fn update_profile(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<UpdateProfileRequest>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let wallet = user.wallet_address.to_ascii_lowercase();

    let bio = body.bio.as_deref().map(|s| &s[..s.len().min(500)]);
    let avatar_url = body.avatar_url.as_deref().map(|s| &s[..s.len().min(512)]);
    let website_url = body.website_url.as_deref().map(|s| &s[..s.len().min(256)]);
    let twitter_handle = body.twitter_handle.as_deref().map(|s| &s[..s.len().min(32)]);

    state
        .db
        .update_user_profile(&wallet, bio, avatar_url, website_url, twitter_handle)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "ok": true })))
}

// ── Market comments ───────────────────────────────────────────────────

pub async fn get_market_comments(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<PaginationParams>,
) -> Result<impl Responder, ApiError> {
    let market_id = path.into_inner();
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0);

    let comments = state
        .db
        .list_market_comments(&market_id, limit, offset)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "comments": comments,
        "limit": limit,
        "offset": offset,
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostCommentRequest {
    pub text: String,
    pub parent_id: Option<String>,
}

pub async fn post_market_comment(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<PostCommentRequest>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let wallet = user.wallet_address.to_ascii_lowercase();
    let market_id = path.into_inner();

    let text = body.text.trim();
    if text.is_empty() {
        return Err(ApiError::bad_request(
            "EMPTY_COMMENT",
            "Comment text must not be empty",
        ));
    }
    if text.len() > 2000 {
        return Err(ApiError::bad_request(
            "COMMENT_TOO_LONG",
            "Comment text must be 2000 characters or fewer",
        ));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let comment = state
        .db
        .insert_market_comment(&id, &market_id, &wallet, text, body.parent_id.as_deref())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Created().json(comment))
}
