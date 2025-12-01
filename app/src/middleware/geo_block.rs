use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header,
    web, HttpResponse,
};
use futures::future::{ok, LocalBoxFuture, Ready};
use log::{info, warn};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::OnceLock;

/// Blocked countries by ISO 3166-1 alpha-2 code
static BLOCKED_COUNTRIES: OnceLock<HashSet<&'static str>> = OnceLock::new();

fn get_blocked_countries() -> &'static HashSet<&'static str> {
    BLOCKED_COUNTRIES.get_or_init(|| {
        let mut set = HashSet::new();
        // US and territories
        set.insert("US");
        set.insert("UM"); // US Minor Outlying Islands
        set.insert("PR"); // Puerto Rico
        set.insert("VI"); // US Virgin Islands
        set.insert("GU"); // Guam
        set.insert("AS"); // American Samoa

        // UK
        set.insert("GB");

        // Sanctioned countries
        set.insert("CU"); // Cuba
        set.insert("IR"); // Iran
        set.insert("KP"); // North Korea
        set.insert("SY"); // Syria
        set.insert("RU"); // Russia

        // Other restricted jurisdictions
        set.insert("AU"); // Australia (depending on license)
        set.insert("CA"); // Canada (Ontario specifically, but blocking all for safety)
        set
    })
}

pub fn blocked_country_codes() -> Vec<&'static str> {
    let mut countries: Vec<_> = get_blocked_countries().iter().copied().collect();
    countries.sort_unstable();
    countries
}

/// Geo-blocking middleware
///
/// Blocks requests from prohibited jurisdictions based on:
/// 1. CF-IPCountry header (Cloudflare)
/// 2. X-Vercel-IP-Country header (Vercel)
/// 3. GeoIP headers from other CDNs
pub struct GeoBlock {
    enabled: bool,
}

impl GeoBlock {
    pub fn new(enabled: bool) -> Self {
        if enabled {
            info!(
                "Geo-blocking enabled for {} countries",
                get_blocked_countries().len()
            );
        } else {
            warn!("Geo-blocking DISABLED - not recommended for production");
        }
        Self { enabled }
    }
}

impl<S, B> Transform<S, ServiceRequest> for GeoBlock
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Transform = GeoBlockMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(GeoBlockMiddleware {
            service,
            enabled: self.enabled,
        })
    }
}

pub struct GeoBlockMiddleware<S> {
    service: S,
    enabled: bool,
}

impl<S, B> Service<ServiceRequest> for GeoBlockMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Skip geo-blocking if disabled
        if !self.enabled {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_left_body())
            });
        }

        // Skip geo-blocking for health checks
        let path = req.path();
        if path.starts_with("/health") || path == "/metrics" || path == "/metrics/prometheus" {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_left_body())
            });
        }

        // Restrict writes only. Read endpoints stay publicly accessible.
        let is_write = matches!(
            *req.method(),
            actix_web::http::Method::POST
                | actix_web::http::Method::PUT
                | actix_web::http::Method::PATCH
                | actix_web::http::Method::DELETE
        );
        if !is_write {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_left_body())
            });
        }

        let country = get_country_override(&req).or_else(|| get_country_from_headers(&req));

        if let Some(country_code) = &country {
            if get_blocked_countries().contains(country_code.as_str()) {
                warn!(
                    "Blocked request from {} (IP: {:?})",
                    country_code,
                    req.connection_info().realip_remote_addr()
                );
