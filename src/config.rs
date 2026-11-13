use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub provider: ProviderConfig,
    pub providers: Vec<ProviderConfig>,
    pub show_activity: bool,
    pub show_thought_process: bool,
    pub code_depth_preview: bool,
    pub password_lock_enabled: bool,
    pub skills: Vec<SkillConfig>,
    pub mcp_connectors: Vec<McpConnectorConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderConfig {
    pub name: String,
    pub endpoint: String,
    pub model: String,
    pub api_key_env: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillConfig {
    pub name: String,
    pub path: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct McpConnectorConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            name: "OpenRouter".into(),
            endpoint: "https://openrouter.ai/api/v1/chat/completions".into(),
            model: "openrouter/free".into(),
            api_key_env: "OPENROUTER_API_KEY".into(),
        }
    }
}

impl Default for SkillConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            path: String::new(),
            enabled: true,
        }
    }
}

impl Default for McpConnectorConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: String::new(),
            args: Vec::new(),
            enabled: true,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider: ProviderConfig::default(),
            providers: Vec::new(),
            show_activity: true,
            show_thought_process: true,
            code_depth_preview: false,
            password_lock_enabled: false,
            skills: Vec::new(),
            mcp_connectors: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn path() -> Result<PathBuf> {
        let base = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg)
        } else {
            PathBuf::from(std::env::var("HOME").context("HOME is not set")?).join(".config")
        };
        Ok(base.join("cluster").join("config.json"))
    }

    fn legacy_path() -> Result<PathBuf> {
        Ok(Self::path()?.with_extension("toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if path.exists() {
            let raw = fs::read_to_string(path).context("Could not read Cluster config")?;
            return serde_json::from_str(&raw).context("Cluster config is invalid JSON");
        }
        let legacy = Self::legacy_path()?;
        if legacy.exists() {
            let raw = fs::read_to_string(&legacy).context("Could not read legacy Cluster config")?;
            let config: Self = toml::from_str(&raw).context("Legacy Cluster config is invalid TOML")?;
            config.save()?;
            return Ok(config);
        }
        Ok(Self::default())
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("Could not create config directory")?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, raw).context("Could not write config")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
        }
        fs::rename(temporary, path).context("Could not save config")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_enable_thought_process_and_disable_code_preview() {
        let config = AppConfig::default();
        assert!(config.show_thought_process);
        assert!(!config.code_depth_preview);
        assert!(!config.password_lock_enabled);
    }

    #[test]
    fn config_serializes_as_readable_json() {
        let config = AppConfig::default();
        let value = serde_json::to_value(config).unwrap();
        assert_eq!(value["provider"]["name"], "OpenRouter");
        assert_eq!(value["show_thought_process"], true);
    }
}
