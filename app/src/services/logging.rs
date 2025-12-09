//! Structured JSON logging for production use with Loki/Promtail.
//!
//! When LOG_FORMAT=json, outputs structured JSON lines.
//! Otherwise, falls back to env_logger for development.

use chrono::Utc;
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::env;
use std::io::Write;

/// Initialize the logger based on LOG_FORMAT env var.
/// - "json" -> structured JSON output (for Loki/Promtail)
/// - anything else -> env_logger (for development)
pub fn init() {
    let format = env::var("LOG_FORMAT").unwrap_or_default();

    if format == "json" {
        let level = env::var("RUST_LOG")
            .ok()
            .and_then(|s| s.parse::<LevelFilter>().ok())
            .unwrap_or(LevelFilter::Info);

        log::set_boxed_logger(Box::new(JsonLogger { level }))
            .map(|()| log::set_max_level(level))
            .expect("Failed to initialize JSON logger");
    } else {
        env_logger::init();
    }
}

struct JsonLogger {
    level: LevelFilter,
}

impl Log for JsonLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let level = match record.level() {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        };

        let target = record.target();
        let message = format!("{}", record.args());

        // Escape JSON special characters in message
        let escaped_msg = escape_json(&message);
        let escaped_target = escape_json(target);

        let ts = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);

        // Write structured JSON line to stderr (where container logs go)
        let _ = writeln!(
            std::io::stderr(),
            r#"{{"ts":"{}","level":"{}","target":"{}","msg":"{}"}}"#,
            ts,
            level,
            escaped_target,
            escaped_msg
        );
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
}

fn escape_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_json_basic() {
        assert_eq!(escape_json("hello"), "hello");
        assert_eq!(escape_json(""), "");
    }

    #[test]
    fn test_escape_json_quotes() {
        assert_eq!(escape_json(r#"say "hi""#), r#"say \"hi\""#);
    }

    #[test]
    fn test_escape_json_backslash() {
        assert_eq!(escape_json(r"path\to\file"), r"path\\to\\file");
    }

    #[test]
    fn test_escape_json_newlines() {
        assert_eq!(escape_json("line1\nline2"), "line1\\nline2");
        assert_eq!(escape_json("cr\r"), "cr\\r");
        assert_eq!(escape_json("tab\there"), "tab\\there");
    }

    #[test]
    fn test_escape_json_control_chars() {
        assert_eq!(escape_json("\x00"), "\\u0000");
        assert_eq!(escape_json("\x1f"), "\\u001f");
    }

    #[test]
    fn test_escape_json_unicode_passthrough() {
        assert_eq!(escape_json("hello"), "hello");
    }

    #[test]
    fn test_escape_json_complex() {
        let input = "Error: \"file not found\"\npath: C:\\Users\\test";
        let expected = "Error: \\\"file not found\\\"\\npath: C:\\\\Users\\\\test";
        assert_eq!(escape_json(input), expected);
    }
}
