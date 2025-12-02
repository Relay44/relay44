use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;
use std::fmt;

#[derive(Debug)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub status: u16,
    pub details: Option<serde_json::Value>,
    pub headers: Vec<(String, String)>,
}

impl ApiError {
    pub fn bad_request(code: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            status: 400,
            details: None,
            headers: Vec::new(),
        }
    }

    pub fn unauthorized(message: &str) -> Self {
        Self {
            code: "UNAUTHORIZED".to_string(),
            message: message.to_string(),
            status: 401,
            details: None,
            headers: Vec::new(),
        }
    }

    pub fn forbidden(message: &str) -> Self {
        Self {
            code: "FORBIDDEN".to_string(),
            message: message.to_string(),
            status: 403,
            details: None,
            headers: Vec::new(),
        }
    }

    pub fn not_found(resource: &str) -> Self {
        Self {
            code: "NOT_FOUND".to_string(),
            message: format!("{} not found", resource),
            status: 404,
            details: None,
            headers: Vec::new(),
        }
    }

    pub fn internal(message: &str) -> Self {
        Self {
            code: "INTERNAL_ERROR".to_string(),
            message: message.to_string(),
            status: 500,
            details: None,
            headers: Vec::new(),
        }
    }

    pub fn rate_limited(retry_after: u64) -> Self {
        Self {
            code: "RATE_LIMITED".to_string(),
            message: format!("Too many requests. Retry after {} seconds.", retry_after),
            status: 429,
            details: None,
            headers: Vec::new(),
        }
    }

    pub fn conflict(code: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            status: 409,
            details: None,
            headers: Vec::new(),
        }
    }

    pub fn payment_required(message: &str, quote: Option<impl serde::Serialize>) -> Self {
        Self::payment_required_with_headers(message, quote, Vec::new())
    }

    pub fn payment_required_with_headers(
        message: &str,
        quote: Option<impl serde::Serialize>,
        headers: Vec<(String, String)>,
    ) -> Self {
        let details = quote
            .and_then(|value| serde_json::to_value(value).ok())
            .map(|value| serde_json::json!({ "quote": value }));

        Self {
            code: "PAYMENT_REQUIRED".to_string(),
            message: message.to_string(),
            status: 402,
            details,
            headers,
        }
    }

    pub fn legal_restricted(code: &str, message: &str, details: Option<serde_json::Value>) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            status: 451,
            details,
            headers: Vec::new(),
        }
    }

    pub fn with_details(mut self, details: Option<serde_json::Value>) -> Self {
        self.details = details;
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let body = ErrorResponse {
            error: ErrorDetail {
                code: self.code.clone(),
                message: self.message.clone(),
                details: self.details.clone(),
            },
        };

        let mut response = match self.status {
            400 => HttpResponse::BadRequest(),
            401 => HttpResponse::Unauthorized(),
            402 => HttpResponse::PaymentRequired(),
            403 => HttpResponse::Forbidden(),
            404 => HttpResponse::NotFound(),
            409 => HttpResponse::Conflict(),
            451 => HttpResponse::build(actix_web::http::StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS),
            429 => HttpResponse::TooManyRequests(),
            _ => HttpResponse::InternalServerError(),
        };

        for (key, value) in &self.headers {
            response.insert_header((key.as_str(), value.as_str()));
        }

        response.json(body)
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        // SECURITY: Log the full error internally but return generic message
        log::error!("Internal error: {}", err);
        ApiError::internal("An internal error occurred. Please try again later.")
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        // SECURITY: Log the full error internally but return generic message
        // Never expose database details to clients
        log::error!("Database error: {}", err);
        ApiError::internal("A database error occurred. Please try again later.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bad_request_error() {
        let err = ApiError::bad_request("INVALID_INPUT", "Input is invalid");
        assert_eq!(err.code, "INVALID_INPUT");
        assert_eq!(err.message, "Input is invalid");
        assert_eq!(err.status, 400);
    }

    #[test]
    fn test_unauthorized_error() {
        let err = ApiError::unauthorized("Not authenticated");
        assert_eq!(err.code, "UNAUTHORIZED");
        assert_eq!(err.message, "Not authenticated");
        assert_eq!(err.status, 401);
    }

    #[test]
    fn test_forbidden_error() {
        let err = ApiError::forbidden("Access denied");
        assert_eq!(err.code, "FORBIDDEN");
        assert_eq!(err.message, "Access denied");
        assert_eq!(err.status, 403);
    }

    #[test]
    fn test_not_found_error() {
        let err = ApiError::not_found("Market");
        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(err.message, "Market not found");
        assert_eq!(err.status, 404);
    }

    #[test]
    fn test_internal_error() {
        let err = ApiError::internal("Something went wrong");
        assert_eq!(err.code, "INTERNAL_ERROR");
        assert_eq!(err.message, "Something went wrong");
        assert_eq!(err.status, 500);
    }

    #[test]
    fn test_rate_limited_error() {
        let err = ApiError::rate_limited(60);
        assert_eq!(err.code, "RATE_LIMITED");
        assert_eq!(err.message, "Too many requests. Retry after 60 seconds.");
        assert_eq!(err.status, 429);
    }

    #[test]
    fn test_conflict_error() {
        let err = ApiError::conflict("DUPLICATE", "Resource already exists");
        assert_eq!(err.code, "DUPLICATE");
        assert_eq!(err.message, "Resource already exists");
        assert_eq!(err.status, 409);
    }

    #[test]
    fn test_legal_restricted_error() {
        let err = ApiError::legal_restricted("REGION_PROVIDER_RESTRICTED", "blocked", None);
        assert_eq!(err.code, "REGION_PROVIDER_RESTRICTED");
        assert_eq!(err.message, "blocked");
        assert_eq!(err.status, 451);
    }

    #[test]
    fn test_display_trait() {
        let err = ApiError::bad_request("TEST_CODE", "Test message");
        let display = format!("{}", err);
        assert_eq!(display, "TEST_CODE: Test message");
    }

    #[test]
    fn test_debug_trait() {
        let err = ApiError::bad_request("TEST", "msg");
        let debug = format!("{:?}", err);
        assert!(debug.contains("TEST"));
        assert!(debug.contains("msg"));
    }

    #[test]
    fn test_from_anyhow_error() {
        let anyhow_err = anyhow::anyhow!("Some internal error");
        let api_err: ApiError = anyhow_err.into();
        assert_eq!(api_err.code, "INTERNAL_ERROR");
        assert_eq!(api_err.status, 500);
        // Message should be generic, not exposing internal details
        assert!(!api_err.message.contains("Some internal error"));
    }

    #[test]
    fn test_all_status_codes_unique() {
        let errors = [
            ApiError::bad_request("", ""),
            ApiError::unauthorized(""),
            ApiError::forbidden(""),
            ApiError::not_found(""),
            ApiError::internal(""),
            ApiError::rate_limited(0),
            ApiError::conflict("", ""),
            ApiError::legal_restricted("REGION_PROVIDER_RESTRICTED", "", None),
        ];
        let statuses: Vec<u16> = errors.iter().map(|e| e.status).collect();
        assert_eq!(statuses, [400, 401, 403, 404, 500, 429, 409, 451]);
    }
}

