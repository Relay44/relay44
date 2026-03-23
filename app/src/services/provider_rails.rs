use actix_web::HttpRequest;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

const DEFAULT_COUNTRY_HEADERS: [&str; 3] =
    ["cf-ipcountry", "x-vercel-ip-country", "x-country-code"];
const DEFAULT_LIMITLESS_RESTRICTED_COUNTRIES: [&str; 1] = ["US"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionClass {
    Us,
    NonUs,
    Unknown,
}

impl RegionClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Us => "us",
            Self::NonUs => "non_us",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionRoutingMode {
    Disabled,
    Observe,
    Enforce,
}

impl RegionRoutingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Observe => "observe",
            Self::Enforce => "enforce",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionUnknownPolicy {
    SafeFallback,
    HardBlock,
    AllowAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRailAction {
    Feed,
    MarketData,
    TradeOpen,
    TradeClose,
}

impl ProviderRailAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Feed => "feed",
            Self::MarketData => "market_data",
            Self::TradeOpen => "trade_open",
            Self::TradeClose => "trade_close",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RailProvider {
    Limitless,
    Polymarket,
}

impl RailProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Limitless => "limitless",
            Self::Polymarket => "polymarket",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub feed: bool,
    pub market_data: bool,
    pub trade_open: bool,
    pub trade_close: bool,
    pub legacy_close_only: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplianceRailsProfile {
    pub country: Option<String>,
    pub region_class: String,
    pub mode: String,
    pub rails: BTreeMap<String, ProviderCapabilities>,
    pub legacy_close_only: bool,
}

#[derive(Debug, Clone)]
pub struct RegionPolicyContext {
    pub country: Option<String>,
    pub region_class: RegionClass,
    pub mode: RegionRoutingMode,
    pub unknown_policy: RegionUnknownPolicy,
    pub safe_fallback_restriction: bool,
    pub limitless_restricted: bool,
}

#[derive(Debug, Clone)]
pub struct ProviderAccessDecision {
    pub allowed: bool,
    pub would_block: bool,
    pub reason: Option<String>,
    pub legacy_close_only: bool,
    pub country: Option<String>,
    pub region_class: RegionClass,
    pub mode: RegionRoutingMode,
    pub safe_fallback_restriction: bool,
}

fn parse_boolean(raw: Option<String>, fallback: bool) -> bool {
    let Some(value) = raw else {
        return fallback;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => fallback,
    }
}

fn parse_csv(raw: Option<String>, fallback: &[&str]) -> Vec<String> {
    match raw {
        Some(value) if !value.trim().is_empty() => value
            .split(',')
            .map(|entry| entry.trim())
            .filter(|entry| !entry.is_empty())
            .map(|entry| entry.to_string())
            .collect(),
        _ => fallback.iter().map(|entry| (*entry).to_string()).collect(),
    }
}

fn parse_unknown_policy(raw: Option<String>) -> RegionUnknownPolicy {
    match raw
        .unwrap_or_else(|| "safe_fallback".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "allow_all" => RegionUnknownPolicy::AllowAll,
        "hard_block" => RegionUnknownPolicy::HardBlock,
        _ => RegionUnknownPolicy::SafeFallback,
    }
}

fn parse_routing_mode() -> RegionRoutingMode {
    if !parse_boolean(std::env::var("REGION_ROUTING_ENABLED").ok(), false) {
        return RegionRoutingMode::Disabled;
    }
    match std::env::var("REGION_ROUTING_MODE")
        .unwrap_or_else(|_| "enforce".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "observe" => RegionRoutingMode::Observe,
        _ => RegionRoutingMode::Enforce,
    }
}

fn normalize_country(raw: &str) -> Option<String> {
    let first = raw.split(',').next()?.trim();
    if first.is_empty() {
        return None;
    }
    let normalized = first
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_uppercase();
    if normalized.len() == 2 {
        Some(normalized)
    } else {
        None
    }
}

fn read_country(req: &HttpRequest) -> Option<String> {
    let headers = parse_csv(
        std::env::var("REGION_COUNTRY_HEADER_PRIORITY").ok(),
        &DEFAULT_COUNTRY_HEADERS,
    );
    for key in headers {
        if let Some(raw) = req.headers().get(key.as_str()) {
            if let Ok(value) = raw.to_str() {
                if let Some(country) = normalize_country(value) {
                    return Some(country);
                }
            }
        }
    }
    None
}

fn to_region_class(country: Option<&str>) -> RegionClass {
    match country {
        Some("US") => RegionClass::Us,
        Some(_) => RegionClass::NonUs,
        None => RegionClass::Unknown,
    }
}

fn build_limitless_restricted_set() -> HashSet<String> {
    parse_csv(
        std::env::var("LIMITLESS_RESTRICTED_COUNTRIES").ok(),
        &DEFAULT_LIMITLESS_RESTRICTED_COUNTRIES,
    )
    .into_iter()
    .map(|entry| entry.to_ascii_uppercase())
    .collect()
}

fn open_capabilities() -> ProviderCapabilities {
    ProviderCapabilities {
        feed: true,
        market_data: true,
        trade_open: true,
        trade_close: true,
        legacy_close_only: false,
    }
}

fn hard_block_capabilities() -> ProviderCapabilities {
    ProviderCapabilities {
        feed: false,
        market_data: false,
        trade_open: false,
        trade_close: false,
        legacy_close_only: false,
    }
}

fn close_only_capabilities() -> ProviderCapabilities {
    ProviderCapabilities {
        feed: true,
        market_data: true,
        trade_open: false,
        trade_close: true,
        legacy_close_only: true,
    }
}

fn restrictions_apply(context: &RegionPolicyContext, provider: RailProvider) -> bool {
    match provider {
        RailProvider::Limitless => context.limitless_restricted,
        RailProvider::Polymarket => false,
    }
}

fn capabilities_for_provider(
    context: &RegionPolicyContext,
    provider: RailProvider,
) -> ProviderCapabilities {
    if context.mode == RegionRoutingMode::Disabled {
        return open_capabilities();
    }

    if context.region_class == RegionClass::Unknown
        && context.unknown_policy == RegionUnknownPolicy::HardBlock
    {
        return hard_block_capabilities();
    }

    if restrictions_apply(context, provider) {
        return close_only_capabilities();
    }

    open_capabilities()
}

fn action_allowed(capabilities: &ProviderCapabilities, action: ProviderRailAction) -> bool {
    match action {
        ProviderRailAction::Feed => capabilities.feed,
        ProviderRailAction::MarketData => capabilities.market_data,
        ProviderRailAction::TradeOpen => capabilities.trade_open,
        ProviderRailAction::TradeClose => capabilities.trade_close,
    }
}

pub fn resolve_region_policy_context(req: &HttpRequest) -> RegionPolicyContext {
    let mode = parse_routing_mode();
    let unknown_policy = parse_unknown_policy(std::env::var("REGION_UNKNOWN_POLICY").ok());
    let country = read_country(req);
    let region_class = to_region_class(country.as_deref());
    let unknown_restricted =
        country.is_none() && unknown_policy == RegionUnknownPolicy::SafeFallback;
    let restricted = build_limitless_restricted_set();
    let limitless_restricted = match country.as_deref() {
        Some(value) => restricted.contains(value),
        None => unknown_restricted,
    };

    RegionPolicyContext {
        country,
        region_class,
        mode,
        unknown_policy,
        safe_fallback_restriction: unknown_restricted,
        limitless_restricted: mode != RegionRoutingMode::Disabled && limitless_restricted,
    }
}

pub fn build_compliance_profile(req: &HttpRequest) -> ComplianceRailsProfile {
    let context = resolve_region_policy_context(req);
    let mut rails = BTreeMap::new();
    rails.insert(
        RailProvider::Limitless.as_str().to_string(),
        capabilities_for_provider(&context, RailProvider::Limitless),
    );
    rails.insert(
        RailProvider::Polymarket.as_str().to_string(),
        capabilities_for_provider(&context, RailProvider::Polymarket),
    );
    let legacy_close_only = rails.values().any(|entry| entry.legacy_close_only);

    ComplianceRailsProfile {
        country: context.country,
        region_class: context.region_class.as_str().to_string(),
        mode: context.mode.as_str().to_string(),
        rails,
        legacy_close_only,
    }
}

pub fn evaluate_provider_access(
    req: &HttpRequest,
    provider: RailProvider,
    action: ProviderRailAction,
) -> ProviderAccessDecision {
    let context = resolve_region_policy_context(req);
    let capabilities = capabilities_for_provider(&context, provider);
    let would_block = !action_allowed(&capabilities, action);
    let allowed = if context.mode == RegionRoutingMode::Enforce {
        !would_block
    } else {
        true
    };
    let reason = if would_block {
        Some(format!(
            "{} is unavailable for {} in this region",
            provider.as_str(),
            action.as_str()
        ))
    } else {
        None
    };

    ProviderAccessDecision {
        allowed,
        would_block,
        reason,
        legacy_close_only: capabilities.legacy_close_only,
        country: context.country,
        region_class: context.region_class,
        mode: context.mode,
        safe_fallback_restriction: context.safe_fallback_restriction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;
    use std::sync::Mutex;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn with_env<F: FnOnce()>(f: F) {
        let _guard = TEST_MUTEX.lock().expect("lock");
        let keys = [
            "REGION_ROUTING_ENABLED",
            "REGION_ROUTING_MODE",
            "REGION_UNKNOWN_POLICY",
            "REGION_COUNTRY_HEADER_PRIORITY",
            "LIMITLESS_RESTRICTED_COUNTRIES",
        ];
        let saved: Vec<(String, Option<String>)> = keys
            .iter()
            .map(|key| (key.to_string(), std::env::var(key).ok()))
            .collect();

        for key in keys {
            std::env::remove_var(key);
        }

        f();

        for (key, value) in saved {
            if let Some(raw) = value {
                std::env::set_var(key, raw);
            } else {
                std::env::remove_var(key);
            }
        }
    }

    #[test]
    fn enforce_mode_blocks_limitless_open_trades_for_restricted_country() {
        with_env(|| {
            std::env::set_var("REGION_ROUTING_ENABLED", "true");
            std::env::set_var("REGION_ROUTING_MODE", "enforce");
            std::env::set_var("LIMITLESS_RESTRICTED_COUNTRIES", "US");

            let req = TestRequest::default()
                .insert_header(("cf-ipcountry", "US"))
                .to_http_request();

            let feed =
                evaluate_provider_access(&req, RailProvider::Limitless, ProviderRailAction::Feed);
            let open = evaluate_provider_access(
                &req,
                RailProvider::Limitless,
                ProviderRailAction::TradeOpen,
            );
            let close = evaluate_provider_access(
                &req,
                RailProvider::Limitless,
                ProviderRailAction::TradeClose,
            );

            assert!(feed.allowed);
            assert!(!open.allowed);
            assert!(close.allowed);
        });
    }

    #[test]
    fn observe_mode_never_blocks_requests() {
        with_env(|| {
            std::env::set_var("REGION_ROUTING_ENABLED", "1");
            std::env::set_var("REGION_ROUTING_MODE", "observe");
            std::env::set_var("LIMITLESS_RESTRICTED_COUNTRIES", "US");

            let req = TestRequest::default()
                .insert_header(("cf-ipcountry", "US"))
                .to_http_request();
            let decision = evaluate_provider_access(
                &req,
                RailProvider::Limitless,
                ProviderRailAction::TradeOpen,
            );

            assert!(decision.allowed);
            assert!(decision.would_block);
        });
    }

    #[test]
    fn unknown_policy_allow_all_keeps_limitless_open() {
        with_env(|| {
            std::env::set_var("REGION_ROUTING_ENABLED", "true");
            std::env::set_var("REGION_ROUTING_MODE", "enforce");
            std::env::set_var("REGION_UNKNOWN_POLICY", "allow_all");

            let req = TestRequest::default().to_http_request();
            let decision = evaluate_provider_access(
                &req,
                RailProvider::Limitless,
                ProviderRailAction::TradeOpen,
            );
            assert!(decision.allowed);
            assert!(!decision.would_block);
        });
    }

    #[test]
    fn unknown_policy_hard_block_blocks_all_actions() {
        with_env(|| {
            std::env::set_var("REGION_ROUTING_ENABLED", "true");
            std::env::set_var("REGION_ROUTING_MODE", "enforce");
            std::env::set_var("REGION_UNKNOWN_POLICY", "hard_block");

            let req = TestRequest::default().to_http_request();

            for provider in [RailProvider::Limitless, RailProvider::Polymarket] {
                for action in [
                    ProviderRailAction::Feed,
                    ProviderRailAction::MarketData,
                    ProviderRailAction::TradeOpen,
                    ProviderRailAction::TradeClose,
                ] {
                    let decision = evaluate_provider_access(&req, provider, action);
                    assert!(!decision.allowed);
                }
            }
        });
    }
}
