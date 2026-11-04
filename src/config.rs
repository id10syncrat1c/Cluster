use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub provider: ProviderConfig,
    pub show_activity: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderConfig {
    pub name: String,
    pub endpoint: String,
    pub model: String,
    pub api_key_env: String,
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

impl Default for AppConfig {
    fn default() -> Self {
        Self { provider: ProviderConfig::default(), show_activity: true }
    }
}

impl AppConfig {
    pub fn path() -> Result<PathBuf> {
        let base = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg)
        } else {
            PathBuf::from(std::env::var("HOME").context("HOME is not set")?).join(".config")
        };
        Ok(base.join("cluster").join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() { return Ok(Self::default()); }
        let raw = fs::read_to_string(path).context("Could not read Cluster config")?;
        toml::from_str(&raw).context("Cluster config is invalid TOML")
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("Could not create config directory")?;
        }
        let raw = toml::to_string_pretty(self)?;
        let temporary = path.with_extension("toml.tmp");
        fs::write(&temporary, raw).context("Could not write config")?;
        fs::rename(temporary, path).context("Could not save config")?;
        Ok(())
    }
}
