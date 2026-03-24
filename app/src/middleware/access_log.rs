//! Structured access log middleware for production use with Loki/Promtail.
//!
//! Emits one log line per request with method, path, status, and latency.
//! Pairs with request_id middleware for correlation.

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use futures::future::{ok, LocalBoxFuture, Ready};
use std::rc::Rc;
use std::time::Instant;

use super::request_id::RequestId;

/// Access log middleware factory
pub struct AccessLog;

impl<S, B> Transform<S, ServiceRequest> for AccessLog
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = AccessLogService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(AccessLogService {
            service: Rc::new(service),
        })
    }
}

pub struct AccessLogService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for AccessLogService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();

        let method = req.method().to_string();
        let path = req.path().to_string();
        let request_id = req
            .extensions()
            .get::<RequestId>()
            .map(|r| r.0.clone())
            .unwrap_or_default();

        let start = Instant::now();

        Box::pin(async move {
            let res = service.call(req).await?;
            let elapsed = start.elapsed();
            let status = res.status().as_u16();
            let latency_ms = elapsed.as_secs_f64() * 1000.0;

            // Skip health check noise
            if (path == "/health" || path == "/health/detailed") && status == 200 {
                return Ok(res);
            }

            let level = if status >= 500 {
                "ERROR"
            } else if status >= 400 {
                "WARN"
            } else {
                "INFO"
            };

            log::log!(
                match level {
                    "ERROR" => log::Level::Error,
                    "WARN" => log::Level::Warn,
                    _ => log::Level::Info,
                },
                "request method={} path={} status={} latency_ms={:.2} request_id={}",
                method,
                path,
                status,
                latency_ms,
                request_id
            );

            Ok(res)
        })
    }
}
