use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub timestamp: String,
    pub profile: String,
    pub command: String,
    pub exit_status: i32,
    pub duration_ms: u128,
}

pub struct SessionLogger {
    path: std::path::PathBuf,
    enabled: bool,
    lock: Mutex<()>,
}

impl SessionLogger {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            path: config.session_log_path()?,
            enabled: config.session_log.enabled,
            lock: Mutex::new(()),
        })
    }

    pub fn append(
        &self,
        profile: &str,
        command: &str,
        exit_status: i32,
        duration_ms: u128,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let _guard = self.lock.lock().expect("session logger lock poisoned");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let entry = SessionEntry {
            timestamp: Utc::now().to_rfc3339(),
            profile: profile.to_string(),
            command: command.to_string(),
            exit_status,
            duration_ms,
        };
        serde_json::to_writer(&mut file, &entry)?;
        writeln!(file)?;
        Ok(())
    }
}

pub fn read_entries(path: &Path) -> Result<Vec<SessionEntry>> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read session file {}", path.display()))?;

    if data.trim_start().starts_with('[') {
        return Ok(serde_json::from_str(&data)?);
    }

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        entries.push(serde_json::from_str(&line)?);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_json_array_export() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sessions.json");
        std::fs::write(
            &path,
            serde_json::to_string(&vec![SessionEntry {
                timestamp: "2026-04-03T00:00:00Z".into(),
                profile: "prod".into(),
                command: "markets list".into(),
                exit_status: 0,
                duration_ms: 42,
            }])
            .unwrap(),
        )
        .unwrap();

        let entries = read_entries(&path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "markets list");
    }
}
