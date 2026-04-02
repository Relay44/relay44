use std::fmt;
use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use rust_decimal::Decimal;
use tabled::settings::style::Style;
use tabled::settings::{Alignment, Modify, Width};
use tabled::settings::object::Columns;
use tabled::{Table, Tabled};

static QUIET: AtomicBool = AtomicBool::new(false);
static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn set_quiet(q: bool) {
    QUIET.store(q, Ordering::Relaxed);
}

pub fn is_quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}

pub fn set_verbose(v: bool) {
    VERBOSE.store(v, Ordering::Relaxed);
}

pub fn is_verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

#[derive(Clone, Copy)]
pub enum Format {
    Table,
    Json,
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Format::Table => write!(f, "table"),
            Format::Json => write!(f, "json"),
        }
    }
}

impl std::str::FromStr for Format {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "table" => Ok(Format::Table),
            "json" => Ok(Format::Json),
            _ => Err(format!("unknown format '{s}' (expected: table, json)")),
        }
    }
}

fn is_tty() -> bool {
    io::stderr().is_terminal()
}

fn term_width() -> usize {
    if let Ok(cols) = std::env::var("COLUMNS") {
        if let Ok(w) = cols.parse::<usize>() {
            return w;
        }
    }
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80)
}

// --- Value extraction from JSON ---

pub fn str_val(val: &serde_json::Value, key: &str) -> String {
    match &val[key] {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "—".into(),
        other => other.to_string(),
    }
}

pub fn price_field(val: &serde_json::Value, key: &str) -> String {
    match &val[key] {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                format!("{:.1}¢", f * 100.0)
            } else {
                n.to_string()
            }
        }
        _ => "—".into(),
    }
}

// --- Number formatting (Polymarket-style) ---

pub fn usdc(lamports: f64) -> String {
    let d = Decimal::from_f64_retain(lamports / 1_000_000.0).unwrap_or_default();
    format!("${}", format_decimal(d))
}

#[allow(dead_code)]
pub fn usdc_raw(amount: f64) -> String {
    let d = Decimal::from_f64_retain(amount).unwrap_or_default();
    format!("${}", format_decimal(d))
}

pub fn format_decimal(d: Decimal) -> String {
    let million = Decimal::from(1_000_000);
    let thousand = Decimal::from(1_000);

    if d.abs() >= million {
        let v = d / million;
        format!("{:.1}M", v)
    } else if d.abs() >= thousand {
        let v = d / thousand;
        format!("{:.1}K", v)
    } else {
        format!("{:.2}", d)
    }
}

pub fn format_date(val: &serde_json::Value, key: &str) -> String {
    match val[key].as_str() {
        Some(s) => {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                dt.format("%Y-%m-%d %H:%M UTC").to_string()
            } else {
                s.to_string()
            }
        }
        None => "—".into(),
    }
}

pub fn active_status(val: &serde_json::Value) -> String {
    match val["active"].as_bool() {
        Some(true) => "active".into(),
        Some(false) => "inactive".into(),
        None => str_val(val, "status"),
    }
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max > 1 {
        format!("{}…", &s[..max - 1])
    } else {
        s[..max].to_string()
    }
}

// --- Slug / ID detection ---

#[allow(dead_code)]
pub fn is_numeric_id(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

// --- JSON output ---

pub fn print_json(value: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(value).unwrap());
}

#[allow(dead_code)]
pub fn print_error_json(msg: &str) {
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({ "error": msg })).unwrap()
    );
}

// --- Table output (tabled crate) ---

pub fn print_tabled<T: Tabled>(data: &[T]) {
    if data.is_empty() {
        dimmed("(no results)");
        return;
    }
    let width = term_width();
    let table = Table::new(data)
        .with(Style::rounded())
        .with(Width::wrap(width).keep_words(true))
        .to_string();
    println!("{table}");
}

pub fn print_tabled_with_cols<T: Tabled>(data: &[T], right_align_cols: &[usize]) {
    if data.is_empty() {
        dimmed("(no results)");
        return;
    }
    let width = term_width();
    let mut table = Table::new(data);
    table.with(Style::rounded());
    table.with(Width::wrap(width).keep_words(true));
    for &col in right_align_cols {
        table.with(Modify::new(Columns::single(col)).with(Alignment::right()));
    }
    println!("{table}");
}

// --- Detail table (2-column bordered view) ---

pub fn print_detail(pairs: &[(&str, String)]) {
    #[derive(Tabled)]
    struct Row {
        #[tabled(rename = "Field")]
        key: String,
        #[tabled(rename = "Value")]
        value: String,
    }

    let rows: Vec<Row> = pairs
        .iter()
        .map(|(k, v)| Row {
            key: k.to_string(),
            value: v.clone(),
        })
        .collect();

    let width = term_width();
    let table = Table::new(&rows)
        .with(Style::rounded())
        .with(Width::wrap(width).keep_words(true))
        .to_string();
    println!("{table}");
}

// --- Pagination ---

pub fn pagination_hint(offset: u32, limit: u32, total: Option<u64>) {
    if is_quiet() {
        return;
    }
    if let Some(t) = total {
        let end = (offset as u64 + limit as u64).min(t);
        if t > 0 {
            dimmed(&format!("showing {}-{} of {t}", offset + 1, end));
        }
        if end < t {
            dimmed(&format!(
                "next page: add --offset {}",
                offset + limit
            ));
        }
    }
}

// --- Messages ---

pub fn success(msg: &str) {
    if is_quiet() {
        return;
    }
    if is_tty() {
        eprintln!("{} {msg}", "✓".green().bold());
    } else {
        eprintln!("{msg}");
    }
}

pub fn warn(msg: &str) {
    if is_tty() {
        eprintln!("{} {msg}", "!".yellow().bold());
    } else {
        eprintln!("warning: {msg}");
    }
}

pub fn error(msg: &str) {
    if is_tty() {
        eprintln!("{} {msg}", "✗".red().bold());
    } else {
        eprintln!("error: {msg}");
    }
}

pub fn dimmed(msg: &str) {
    if is_quiet() {
        return;
    }
    if is_tty() {
        eprintln!("{}", msg.dimmed());
    } else {
        eprintln!("{msg}");
    }
}

pub fn debug(msg: &str) {
    if !is_verbose() {
        return;
    }
    if is_tty() {
        eprintln!("{} {msg}", "→".cyan());
    } else {
        eprintln!("debug: {msg}");
    }
}

pub fn spinner(msg: &str) -> ProgressBar {
    if !is_tty() || is_quiet() {
        return ProgressBar::hidden();
    }
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

pub fn confirm(prompt: &str) -> bool {
    if !is_tty() || is_quiet() {
        return true;
    }
    eprint!("{prompt} [y/N] ");
    let _ = io::stderr().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use serde_json::json;

    #[test]
    fn format_decimal_small() {
        let d = Decimal::new(42_15, 2); // 42.15
        assert_eq!(format_decimal(d), "42.15");
    }

    #[test]
    fn format_decimal_thousands() {
        let d = Decimal::from(5_500);
        assert_eq!(format_decimal(d), "5.5K");
    }

    #[test]
    fn format_decimal_millions() {
        let d = Decimal::from(2_300_000);
        assert_eq!(format_decimal(d), "2.3M");
    }

    #[test]
    fn format_decimal_negative() {
        let d = Decimal::from(-1_500);
        assert_eq!(format_decimal(d), "-1.5K");
    }

    #[test]
    fn usdc_converts_lamports() {
        assert_eq!(usdc(1_000_000.0), "$1.00");
        assert_eq!(usdc(50_000_000_000.0), "$50.0K");
    }

    #[test]
    fn str_val_extracts_types() {
        let v = json!({"s": "hello", "n": 42, "b": true, "nil": null});
        assert_eq!(str_val(&v, "s"), "hello");
        assert_eq!(str_val(&v, "n"), "42");
        assert_eq!(str_val(&v, "b"), "true");
        assert_eq!(str_val(&v, "nil"), "—");
        assert_eq!(str_val(&v, "missing"), "—");
    }

    #[test]
    fn price_field_formats_cents() {
        let v = json!({"p": 0.65, "nil": null});
        assert_eq!(price_field(&v, "p"), "65.0¢");
        assert_eq!(price_field(&v, "nil"), "—");
    }

    #[test]
    fn truncate_short_string() {
        assert_eq!(truncate("hi", 10), "hi");
    }

    #[test]
    fn truncate_long_string() {
        assert_eq!(truncate("hello world", 5), "hell…");
    }

    #[test]
    fn truncate_max_one() {
        assert_eq!(truncate("hello", 1), "h");
    }

    #[test]
    fn is_numeric_id_valid() {
        assert!(is_numeric_id("abc-123"));
        assert!(is_numeric_id("deadbeef"));
        assert!(!is_numeric_id(""));
        assert!(!is_numeric_id("hello world"));
    }

    #[test]
    fn format_parses_from_str() {
        assert!(matches!("table".parse::<Format>(), Ok(Format::Table)));
        assert!(matches!("json".parse::<Format>(), Ok(Format::Json)));
        assert!(matches!("JSON".parse::<Format>(), Ok(Format::Json)));
        assert!("xml".parse::<Format>().is_err());
    }

    #[test]
    fn format_display() {
        assert_eq!(Format::Table.to_string(), "table");
        assert_eq!(Format::Json.to_string(), "json");
    }

    #[test]
    fn format_date_rfc3339() {
        let v = json!({"ts": "2025-01-15T12:30:00Z"});
        assert_eq!(format_date(&v, "ts"), "2025-01-15 12:30 UTC");
    }

    #[test]
    fn format_date_missing() {
        let v = json!({});
        assert_eq!(format_date(&v, "ts"), "—");
    }

    #[test]
    fn active_status_variants() {
        assert_eq!(active_status(&json!({"active": true})), "active");
        assert_eq!(active_status(&json!({"active": false})), "inactive");
        assert_eq!(active_status(&json!({"status": "paused"})), "paused");
    }
}
