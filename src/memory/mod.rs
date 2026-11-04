use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use std::hash::{Hash, Hasher};
use anyhow::{Context, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurableMemory {
    pub id: String,
    pub content: String,
    pub category: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversationTurn {
    pub id: String,
    pub user_message: String,
    pub assistant_response: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryData {
    pub durable: Vec<DurableMemory>,
    pub history: Vec<ConversationTurn>,
}

pub struct MemoryStore {
    path: PathBuf,
    pub data: MemoryData,
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

        let data = if path.exists() {
            let content = fs::read_to_string(&path)
                .with_context(|| "Could not read memory file")?;

            match serde_json::from_str::<MemoryData>(&content) {
                Ok(md) => md,
                Err(e) => {
                    eprintln!("Warning: memory file corrupted ({e}), attempting migration");

                    if let Ok(messages) =
                        serde_json::from_str::<Vec<serde_json::Value>>(&content)
                    {
                        let durable = Vec::new();
                        let mut history = Vec::new();
                        let mut current_user: Option<String> = None;

                        for msg in messages {
                            if let (Some(role), Some(content)) =
                                (msg["role"].as_str(), msg["content"].as_str())
                            {
                                match role {
                                    "user" => {
                                        current_user = Some(content.to_string());
                                    }
                                    "assistant" => {
                                        if let Some(user_msg) = current_user.take() {
                                            history.push(ConversationTurn {
                                                id: generate_id(),
                                                user_message: user_msg,
                                                assistant_response: content.to_string(),
                                                timestamp: current_timestamp(),
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }

                        if let Some(user_msg) = current_user {
                            history.push(ConversationTurn {
                                id: generate_id(),
                                user_message: user_msg,
                                assistant_response: String::new(),
                                timestamp: current_timestamp(),
                            });
                        }

                        MemoryData { durable, history }
                    } else {
                        eprintln!("Warning: memory file is corrupted, starting fresh");
                        MemoryData::default()
                    }
                }
            }
        } else {
            MemoryData::default()
        };

        Ok(Self { path, data })
    }

    pub fn save(&self) -> Result<()> {
        let data = serde_json::to_string_pretty(&self.data)?;
        let temporary = self.path.with_extension("tmp");

        fs::write(&temporary, data)
            .with_context(|| "Could not write temporary memory file")?;
        fs::rename(&temporary, &self.path)
            .with_context(|| "Could not replace memory file")?;

        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        self.data.history.clear();
        self.save()
    }

    pub fn reset(&mut self) -> Result<()> {
        self.data = MemoryData::default();
        self.save()
    }

    pub fn export(&self, path: &PathBuf) -> Result<()> {
        let data = serde_json::to_string_pretty(&self.data)?;
        fs::write(path, data)
            .with_context(|| "Could not export memory file")?;
        Ok(())
    }

    pub fn add_durable(
        &mut self,
        content: String,
        category: Option<String>,
    ) -> Result<()> {
        let memory = DurableMemory {
            id: generate_id(),
            content,
            category,
            created_at: current_timestamp(),
            updated_at: current_timestamp(),
        };
        self.data.durable.push(memory);
        self.save()
    }

    pub fn add_turn(
        &mut self,
        user_message: String,
        assistant_response: String,
    ) -> Result<()> {
        let turn = ConversationTurn {
            id: generate_id(),
            user_message,
            assistant_response,
            timestamp: current_timestamp(),
        };
        self.data.history.push(turn);
        self.save()
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn retrieve_relevant(
        &self,
        query: &str,
        limit: usize,
    ) -> Vec<&DurableMemory> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        let scored: Vec<(&DurableMemory, usize)> = self
            .data
            .durable
            .iter()
            .map(|m| {
                let content_lower = m.content.to_lowercase();
                let score = query_words
                    .iter()
                    .filter(|word| content_lower.contains(*word))
                    .count();
                (m, score)
            })
            .filter(|(_, score)| *score > 0)
            .collect();

        scored
            .into_iter()
            .take(limit)
            .map(|(m, _)| m)
            .collect()
    }

    pub fn get_relevant_context(&self, query: &str) -> Vec<Value> {
        let relevant = self.retrieve_relevant(query, 5);
        let mut msgs = Vec::new();
        for mem in relevant {
            msgs.push(json!({
                "role": "system",
                "content": mem.content.clone()
            }));
        }
        msgs
    }
}

fn generate_id() -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    current_timestamp().hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_memory_path() -> PathBuf {
        let tmp = std::env::temp_dir().join("cluster_test_memory.json");
        if tmp.exists() {
            let _ = std::fs::remove_file(&tmp);
        }
        tmp
    }

    #[test]
    fn test_memory_load_empty() {
        let path = test_memory_path();
        let original = std::env::var("XDG_DATA_HOME");
        let _ = std::env::remove_var("XDG_DATA_HOME");
        let _ = std::env::set_var("HOME", path.parent().unwrap());

        let result = MemoryStore::load();

        assert!(result.is_ok());
        let store = result.unwrap();
        assert!(store.data.durable.is_empty());
        assert!(store.data.history.is_empty());

        if let Ok(v) = original {
            let _ = std::env::set_var("XDG_DATA_HOME", v);
        } else {
            let _ = std::env::remove_var("XDG_DATA_HOME");
        }
    }

    #[test]
    fn test_memory_add_and_save() {
        let path = test_memory_path();
        let original_home = std::env::var("HOME").unwrap_or_default();
        let _ = std::env::set_var("HOME", path.parent().unwrap());

        let mut store = MemoryStore::load().unwrap();
        let _ = store.add_durable("hello world".to_string(), Some("test".to_string()));
        assert_eq!(store.data.durable.len(), 1);
        assert_eq!(store.data.durable[0].content, "hello world");

        let _ = store.add_turn("user".to_string(), "assistant".to_string());
        assert_eq!(store.data.history.len(), 1);
        assert_eq!(store.data.history[0].user_message, "user");

        let _ = store.clear();
        assert_eq!(store.data.history.len(), 0);

        let _ = store.reset();
        assert_eq!(store.data.durable.len(), 0);

        let _ = std::env::set_var("HOME", original_home);
    }

    #[test]
    fn test_memory_export() {
        let path = test_memory_path();
        let original_home = std::env::var("HOME").unwrap_or_default();
        let _ = std::env::set_var("HOME", path.parent().unwrap());

        let store = MemoryStore::load().unwrap();
        let export_path = path.with_extension("export.json");
        let _ = store.export(&export_path);
        assert!(export_path.exists());

        let _ = std::env::set_var("HOME", original_home);
    }

    #[test]
    fn test_retrieve_relevant() {
        let path = test_memory_path();
        let original_home = std::env::var("HOME").unwrap_or_default();
        let _ = std::env::set_var("HOME", path.parent().unwrap());

        let mut store = MemoryStore::load().unwrap();
        let _ = store.add_durable("rust programming".to_string(), None);
        let _ = store.add_durable("python scripting".to_string(), None);

        let relevant = store.retrieve_relevant("rust", 10);
        assert_eq!(relevant.len(), 1);
        assert_eq!(relevant[0].content, "rust programming");

        let _ = std::env::set_var("HOME", original_home);
    }
}