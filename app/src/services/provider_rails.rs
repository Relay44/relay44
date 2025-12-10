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
