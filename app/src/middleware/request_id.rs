use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderName, HeaderValue},
    Error, HttpMessage,
};
use futures::future::{ok, LocalBoxFuture, Ready};
use std::rc::Rc;
use uuid::Uuid;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Request ID stored in request extensions for use in handlers
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct RequestId(pub String);

impl RequestId {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Middleware factory for request ID tracking
pub struct RequestIdMiddleware;

impl<S, B> Transform<S, ServiceRequest> for RequestIdMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = RequestIdMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RequestIdMiddlewareService {
            service: Rc::new(service),
        })
    }
}

pub struct RequestIdMiddlewareService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for RequestIdMiddlewareService<S>
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

        Box::pin(async move {
            // Extract existing request ID or generate new one
            let request_id = req
                .headers()
                .get(REQUEST_ID_HEADER)
                .and_then(|v| v.to_str().ok())
                .map(String::from)
                .unwrap_or_else(|| Uuid::new_v4().to_string());

            // Store in request extensions for later use
            req.extensions_mut().insert(RequestId(request_id.clone()));

            // Call the service
            let mut res = service.call(req).await?;

            // Add request ID to response headers
            if let Ok(header_value) = HeaderValue::from_str(&request_id) {
                res.headers_mut()
                    .insert(HeaderName::from_static(REQUEST_ID_HEADER), header_value);
            }

            Ok(res)
        })
    }
}

/// Helper to extract request ID from request in handlers
#[allow(dead_code)]
pub fn get_request_id(req: &actix_web::HttpRequest) -> Option<String> {
    req.extensions().get::<RequestId>().map(|r| r.0.clone())
}
