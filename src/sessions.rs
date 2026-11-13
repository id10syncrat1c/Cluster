use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

use crate::config::ProviderConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTurn {
    pub user_message: String,
    pub assistant_response: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub provider: ProviderConfig,
    pub history: Vec<SessionTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SessionData {
    active_id: String,
    sessions: Vec<Session>,
}

pub struct SessionStore {
    path: PathBuf,
    data: SessionData,
}

impl SessionStore {
    pub fn load(default_provider: ProviderConfig) -> Result<Self> {
        let base = if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(xdg)
        } else {
            PathBuf::from(std::env::var("HOME").context("HOME is not set")?).join(".local/share")
        };
        let path = base.join("cluster").join("sessions.json");
        let mut data = if path.exists() {
            serde_json::from_str(&fs::read_to_string(&path).context("Could not read sessions")?)
                .context("Session file is invalid JSON")?
        } else {
            SessionData::default()
        };
        if data.sessions.is_empty() {
            let session = Session {
                id: session_id(),
                name: "Session 1".into(),
                provider: default_provider,
                history: Vec::new(),
            };
            data.active_id = session.id.clone();
            data.sessions.push(session);
        }
        if !data.sessions.iter().any(|session| session.id == data.active_id) {
            data.active_id = data.sessions[0].id.clone();
        }
        let store = Self { path, data };
        store.save()?;
        Ok(store)
    }

    pub fn sessions(&self) -> &[Session] {
        &self.data.sessions
    }

    pub fn active(&self) -> &Session {
        self.data.sessions.iter().find(|session| session.id == self.data.active_id).unwrap()
    }

    pub fn create(&mut self, provider: ProviderConfig) -> Result<()> {
        let number = self.data.sessions.len() + 1;
        let session = Session {
            id: session_id(),
            name: format!("Session {number}"),
            provider,
            history: Vec::new(),
        };
        self.data.active_id = session.id.clone();
        self.data.sessions.push(session);
        self.save()
    }

    pub fn switch(&mut self, index: usize) -> Result<()> {
        let session = self.data.sessions.get(index).ok_or_else(|| anyhow!("Unknown session"))?;
        self.data.active_id = session.id.clone();
        self.save()
    }

    pub fn set_provider(&mut self, provider: ProviderConfig) -> Result<()> {
        let session = self.data.sessions.iter_mut().find(|session| session.id == self.data.active_id).unwrap();
        session.provider = provider;
        self.save()
    }

    pub fn add_turn(&mut self, user_message: String, assistant_response: String) -> Result<()> {
        let session = self.data.sessions.iter_mut().find(|session| session.id == self.data.active_id).unwrap();
        session.history.push(SessionTurn {
            user_message,
            assistant_response,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        });
        self.save()
    }

    pub fn clear_active(&mut self) -> Result<()> {
        let session = self.data.sessions.iter_mut().find(|session| session.id == self.data.active_id).unwrap();
        session.history.clear();
        self.save()
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).context("Could not create session directory")?;
        }
        let raw = serde_json::to_string_pretty(&self.data)?;
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, raw).context("Could not write sessions")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
        }
        fs::rename(temporary, &self.path).context("Could not save sessions")?;
        Ok(())
    }
}

fn session_id() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    format!("{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_keep_their_provider_configuration() {
        let provider = ProviderConfig::default();
        let path = std::env::temp_dir().join(format!("cluster-sessions-{}.json", std::process::id()));
        let mut store = SessionStore {
            path: path.clone(),
            data: SessionData::default(),
        };
        store.create(provider.clone()).unwrap();
        store.set_provider(ProviderConfig { model: "new-model".into(), ..provider.clone() }).unwrap();
        store.create(provider.clone()).unwrap();
        assert_eq!(store.sessions()[0].provider.model, "new-model");
        assert_eq!(store.active().provider.model, provider.model);
        fs::remove_file(path).unwrap();
    }
}
