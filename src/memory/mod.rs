use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

pub struct MemoryStore {
    path: PathBuf,
    pub messages: Vec<Value>,
}

impl MemoryStore {
    pub fn load() -> Result<Self> {
        let base = if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(xdg)
        } else {
            PathBuf::from(
                std::env::var("HOME").context("HOME is not set")?
            )
            .join(".local/share")
        };

        let directory = base.join("cluster");
        fs::create_dir_all(&directory)?;

        let path = directory.join("memory.json");

        let messages = if path.exists() {
            let data = fs::read_to_string(&path)
                .context("Could not read memory file")?;

            let loaded: Vec<Value> = serde_json::from_str(&data)
                .context("Memory file contains invalid JSON")?;

            for message in &loaded {
                let role = message["role"].as_str()
                    .ok_or_else(|| anyhow!("Invalid message role in memory"))?;

                if role != "user" && role != "assistant" {
                    return Err(anyhow!("Unexpected message role in memory"));
                }

                if !message["content"].is_string() {
                    return Err(anyhow!("Invalid message content in memory"));
                }
            }

            loaded
        } else {
            Vec::new()
        };

        Ok(Self { path, messages })
    }

    pub fn save(&self) -> Result<()> {
        let data = serde_json::to_string_pretty(&self.messages)?;
        let temporary = self.path.with_extension("tmp");

        fs::write(&temporary, data)
            .context("Could not write temporary memory file")?;

        fs::rename(&temporary, &self.path)
            .context("Could not replace memory file")?;

        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        self.messages.clear();
        self.save()
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}
