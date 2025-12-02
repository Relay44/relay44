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
