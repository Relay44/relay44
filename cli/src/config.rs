use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::output::Format;

pub const DEFAULT_PROFILE: &str = "default";
const DEFAULT_API_URL: &str = "https://relay44-api.onrender.com/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_active_profile")]
    pub active_profile: String,
    #[serde(default = "default_profiles")]
    pub profiles: BTreeMap<String, Profile>,
    #[serde(default)]
    pub workflows: BTreeMap<String, Workflow>,
    #[serde(default)]
    pub hooks: Vec<Hook>,
    #[serde(default)]
    pub session_log: SessionLogConfig,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default = "default_api_url")]
    pub api_url: String,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub wallet: Option<String>,
    #[serde(default)]
    pub output: Option<Format>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Workflow {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    pub command: String,
    pub run: String,
    pub stage: HookStage,
    #[serde(default)]
    pub required: bool,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HookStage {
    Pre,
    Post,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLogConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyConfig {
    #[serde(default = "default_api_url")]
    api_url: String,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    wallet: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            active_profile: default_active_profile(),
            profiles: default_profiles(),
            workflows: BTreeMap::new(),
            hooks: Vec::new(),
            session_log: SessionLogConfig::default(),
            aliases: BTreeMap::new(),
        }
    }
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            api_url: default_api_url(),
            access_token: None,
            refresh_token: None,
            wallet: None,
            output: None,
        }
    }
}

impl Default for SessionLogConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: None,
        }
    }
}

impl Config {
    pub fn dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
            .join("r44");
        ensure_private_dir(&dir)?;
        Ok(dir)
    }

    pub fn path() -> Result<PathBuf> {
        Ok(Self::dir()?.join("config.json"))
    }

    pub fn history_path() -> Result<PathBuf> {
        Ok(Self::dir()?.join("history"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let data = std::fs::read_to_string(&path)?;
        let mut config = parse_config(&data).unwrap_or_default();
        config.normalize();
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, data)?;
        set_private_file_permissions(&path);
        Ok(())
    }

    pub fn selected_profile_name(
        &self,
        requested: Option<&str>,
        fallback: Option<&str>,
    ) -> Result<String> {
        let name = requested
            .filter(|value| !value.trim().is_empty())
            .or(fallback.filter(|value| !value.trim().is_empty()))
            .unwrap_or(&self.active_profile);
        if self.profiles.contains_key(name) {
            return Ok(name.to_string());
        }
        Err(anyhow!("profile '{name}' not found"))
    }

    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    pub fn ensure_profile(&mut self, name: &str) -> &mut Profile {
        self.profiles
            .entry(name.to_string())
            .or_insert_with(Profile::default)
    }

    pub fn set_active_profile(&mut self, name: &str) -> Result<()> {
        if !self.profiles.contains_key(name) {
            return Err(anyhow!("profile '{name}' not found"));
        }
        self.active_profile = name.to_string();
        Ok(())
    }

    pub fn session_log_path(&self) -> Result<PathBuf> {
        if let Some(path) = &self.session_log.path {
            if let Some(parent) = path.parent() {
                ensure_private_dir(parent)?;
            }
            return Ok(path.clone());
        }
        Ok(Self::dir()?.join("sessions.jsonl"))
    }

    fn normalize(&mut self) {
        if self.profiles.is_empty() {
            self.profiles = default_profiles();
        }
        if !self.profiles.contains_key(&self.active_profile) {
            self.active_profile = self
                .profiles
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(default_active_profile);
        }
    }
}

fn parse_config(data: &str) -> Result<Config> {
    let value: serde_json::Value = serde_json::from_str(data)?;
    if value.get("profiles").is_some() {
        return Ok(serde_json::from_value(value)?);
    }

    let legacy: LegacyConfig = serde_json::from_value(value)?;
    let mut profiles = BTreeMap::new();
    profiles.insert(
        default_active_profile(),
        Profile {
            api_url: legacy.api_url,
            access_token: legacy.access_token,
            refresh_token: legacy.refresh_token,
            wallet: legacy.wallet,
            output: None,
        },
    );
    Ok(Config {
        active_profile: default_active_profile(),
        profiles,
        workflows: BTreeMap::new(),
        hooks: Vec::new(),
        session_log: SessionLogConfig::default(),
        aliases: BTreeMap::new(),
    })
}

fn ensure_private_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    set_private_dir_permissions(path);
    Ok(())
}

fn set_private_dir_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
    }
}

fn set_private_file_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
}

fn default_profiles() -> BTreeMap<String, Profile> {
    let mut profiles = BTreeMap::new();
    profiles.insert(default_active_profile(), Profile::default());
    profiles
}

fn default_active_profile() -> String {
    DEFAULT_PROFILE.to_string()
}

fn default_api_url() -> String {
    DEFAULT_API_URL.to_string()
}

fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_config() {
        let config = parse_config(
            r#"{
                "api_url": "https://example.com/v1",
                "access_token": "abc",
                "refresh_token": "def",
                "wallet": "wallet-1"
            }"#,
        )
        .unwrap();

        assert_eq!(config.active_profile, DEFAULT_PROFILE);
        let profile = config.profile(DEFAULT_PROFILE).unwrap();
        assert_eq!(profile.api_url, "https://example.com/v1");
        assert_eq!(profile.access_token.as_deref(), Some("abc"));
        assert_eq!(profile.refresh_token.as_deref(), Some("def"));
        assert_eq!(profile.wallet.as_deref(), Some("wallet-1"));
    }

    #[test]
    fn normalizes_missing_active_profile() {
        let mut config = Config {
            active_profile: "missing".into(),
            profiles: BTreeMap::from([("prod".into(), Profile::default())]),
            workflows: BTreeMap::new(),
            hooks: Vec::new(),
            session_log: SessionLogConfig::default(),
            aliases: BTreeMap::new(),
        };

        config.normalize();
        assert_eq!(config.active_profile, "prod");
    }

    #[test]
    fn resolves_requested_or_fallback_profile() {
        let mut config = Config::default();
        config.profiles.insert("prod".into(), Profile::default());
        config.active_profile = "prod".into();

        assert_eq!(
            config.selected_profile_name(Some("prod"), None).unwrap(),
            "prod"
        );
        assert_eq!(
            config.selected_profile_name(None, Some("default")).unwrap(),
            "default"
        );
    }
}
