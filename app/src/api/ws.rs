//! WebSocket endpoint for real-time market updates.
//!
//! # Authentication
//!
//! Two authentication methods are supported:
//!
//! 1. **Query parameter** (recommended): `/ws?token=<jwt>`
//!    - Token is validated before WebSocket upgrade
//!    - Connection rejected immediately if token is invalid
//!
//! 2. **First message**: `{"type": "auth", "token": "<jwt>"}`
//!    - Connection is established, then first message must be auth
//!    - On success: `{"type": "authenticated", "message": "..."}`
//!    - On failure: connection closed with policy violation
//!
//! # Rate Limiting
//!
//! - Authenticated users: 10 connections per minute per user
//! - Unauthenticated: 5 connection attempts per minute per IP

use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use log::{error, info, warn};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::jwt::UserRole;
use super::rate_limit::{check_rate_limit_by_user, RateLimitTier};
use crate::services::websocket::SubscribeRequest;
use crate::AppState;

/// WebSocket connection timeout
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

/// Authentication state for WebSocket connections
enum WsAuthState {
    /// Fully authenticated with JWT claims
    Authenticated { wallet: String, _role: UserRole },
    /// Pending first-message authentication
    PendingAuth,
}

/// WebSocket session actor
pub struct WsSession {
    /// Unique session id
    id: u64,
    /// Client heartbeat
    hb: Instant,
    /// App state
    state: Arc<AppState>,
    /// Authentication state
    auth_state: WsAuthState,
    /// Subscribed markets
    subscribed_markets: Vec<String>,
}

impl WsSession {
    /// Create a new authenticated session
    pub fn authenticated(state: Arc<AppState>, wallet: String, role: UserRole) -> Self {
        Self {
            id: rand::random::<u64>(),
            hb: Instant::now(),
            state,
            auth_state: WsAuthState::Authenticated {
                wallet,
                _role: role,
            },
            subscribed_markets: Vec::new(),
        }
    }

    /// Create a session pending authentication via first message
    pub fn pending_auth(state: Arc<AppState>) -> Self {
        Self {
            id: rand::random::<u64>(),
            hb: Instant::now(),
            state,
            auth_state: WsAuthState::PendingAuth,
            subscribed_markets: Vec::new(),
        }
    }

    /// Get wallet address if authenticated
    fn wallet(&self) -> Option<&str> {
        match &self.auth_state {
            WsAuthState::Authenticated { wallet, .. } => Some(wallet),
            WsAuthState::PendingAuth => None,
        }
    }

    /// Check if session is authenticated
    fn is_authenticated(&self) -> bool {
        matches!(self.auth_state, WsAuthState::Authenticated { .. })
    }

    /// Heartbeat to keep connection alive
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                warn!("WebSocket client timeout, disconnecting");
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }

    /// Handle authentication message (first message when pending auth)
    fn handle_auth_message(&mut self, text: &str, ctx: &mut <Self as Actor>::Context) -> bool {
        // Try to parse as auth message: {"type": "auth", "token": "..."}
        #[derive(serde::Deserialize)]
        struct AuthMessagePayload {
            #[serde(rename = "type")]
            msg_type: String,
            token: String,
        }

        let auth_msg: AuthMessagePayload = match serde_json::from_str::<AuthMessagePayload>(text) {
            Ok(msg) if msg.msg_type == "auth" => msg,
            _ => {
                let response = serde_json::json!({
                    "type": "error",
                    "code": "AUTH_REQUIRED",
                    "message": "First message must be authentication: {\"type\": \"auth\", \"token\": \"<jwt>\"}"
                });
                ctx.text(response.to_string());
                ctx.close(Some(ws::CloseReason {
                    code: ws::CloseCode::Policy,
                    description: Some("Authentication required".to_string()),
                }));
                ctx.stop();
                return false;
            }
        };

        // Validate the token
        match self.state.jwt.validate_token(&auth_msg.token) {
            Ok(claims) => {
                info!(
                    "WebSocket authenticated via message for user: {}",
                    claims.sub
                );
                self.auth_state = WsAuthState::Authenticated {
                    wallet: claims.sub,
                    _role: claims.role,
                };

                let response = serde_json::json!({
                    "type": "authenticated",
                    "message": "Authentication successful"
                });
                ctx.text(response.to_string());
                true
            }
            Err(e) => {
                warn!("WebSocket auth message validation failed: {:?}", e);
                let response = serde_json::json!({
                    "type": "error",
                    "code": "AUTH_FAILED",
                    "message": "Invalid or expired token"
                });
                ctx.text(response.to_string());
                ctx.close(Some(ws::CloseReason {
                    code: ws::CloseCode::Policy,
                    description: Some("Authentication failed".to_string()),
                }));
                ctx.stop();
                false
            }
        }
    }

    /// Handle incoming text message
    fn handle_message(&mut self, text: &str, ctx: &mut <Self as Actor>::Context) {
        // Try to parse as subscription request
        if let Ok(req) = serde_json::from_str::<SubscribeRequest>(text) {
            match req.channel.as_str() {
                "orderbook" | "trades" | "market" => {
                    if let Some(market_id) = req.market_id {
                        if !self.subscribed_markets.contains(&market_id) {
                            self.subscribed_markets.push(market_id.clone());
                            let wallet = self.wallet().unwrap_or("unknown");
                            info!(
                                "Client {} ({}) subscribed to market: {}",
                                self.id, wallet, market_id
                            );

                            // Send confirmation
                            let response = serde_json::json!({
                                "type": "subscribed",
                                "channel": req.channel,
                                "market_id": market_id
                            });
                            ctx.text(response.to_string());
                        }
                    }
                }
                "unsubscribe" => {
                    if let Some(market_id) = req.market_id {
                        self.subscribed_markets.retain(|m| m != &market_id);
                        let wallet = self.wallet().unwrap_or("unknown");
                        info!(
                            "Client {} ({}) unsubscribed from market: {}",
                            self.id, wallet, market_id
                        );
                    }
                }
                _ => {
                    let response = serde_json::json!({
                        "type": "error",
                        "message": "Unknown channel"
                    });
                    ctx.text(response.to_string());
                }
            }
        }
    }
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let status = if self.is_authenticated() {
            format!("authenticated as {}", self.wallet().unwrap_or("unknown"))
        } else {
            "pending authentication".to_string()
        };
        info!("WebSocket session {} started: {}", self.id, status);
        self.hb(ctx);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        let wallet = self.wallet().unwrap_or("unauthenticated");
        info!("WebSocket session {} stopped ({})", self.id, wallet);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                // If not authenticated, first message must be auth
                if !self.is_authenticated() {
                    self.handle_auth_message(&text, ctx);
                    return;
                }
                self.handle_message(&text, ctx);
            }
            Ok(ws::Message::Binary(_)) => {
                warn!("Binary messages not supported");
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            Err(e) => {
                error!("WebSocket error: {:?}", e);
                ctx.stop();
            }
            _ => {}
        }
    }
}

/// WebSocket upgrade handler
///
/// Supports two authentication methods:
/// 1. Query parameter: /ws?token=<jwt> - validates before upgrade
/// 2. First message: {"type": "auth", "token": "<jwt>"} - validates after upgrade
///
/// Rate limited: 10 connections per minute per user (only for pre-authenticated connections)
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<Arc<AppState>>,
    query: web::Query<WsAuthQuery>,
) -> Result<HttpResponse, Error> {
    // Check if token provided via query parameter
    if let Some(token) = &query.token {
        // Validate JWT token from query parameter
        let claims = match state.jwt.validate_token(token) {
            Ok(claims) => claims,
            Err(e) => {
                warn!("WebSocket authentication failed: {:?}", e);
                return Ok(HttpResponse::Unauthorized().json(serde_json::json!({
                    "error": "UNAUTHORIZED",
                    "message": "Invalid or expired token"
                })));
            }
