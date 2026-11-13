use anyhow::{Context, Result, anyhow};
use ring::{
    aead::{self, Aad, LessSafeKey, Nonce, UnboundKey},
    pbkdf2,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use crossterm::{event::{self, Event, KeyCode, KeyEventKind}, terminal};
use std::io::{self, Write};
use std::{collections::HashMap, fs, num::NonZeroU32, path::PathBuf};

const MAGIC: &[u8; 4] = b"CLV1";
const ITERATIONS: u32 = 600_000;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SecretStore {
    values: HashMap<String, String>,
}

impl SecretStore {
    pub fn path() -> Result<PathBuf> {
        let base = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg)
        } else {
            PathBuf::from(std::env::var("HOME").context("HOME is not set")?).join(".config")
        };
        Ok(base.join("cluster").join("secrets.enc"))
    }

    pub fn prompt_password(prompt: &str) -> Result<String> {
        print!("{prompt}");
        io::stdout().flush()?;
        terminal::enable_raw_mode()?;
        let result = (|| -> Result<String> {
            let mut password = String::new();
            loop {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                        KeyCode::Enter => break,
                        KeyCode::Esc => {
                            password.clear();
                            break;
                        }
                        KeyCode::Backspace => {
                            password.pop();
                            print!("\x08 \x08");
                            io::stdout().flush()?;
                        }
                        KeyCode::Char(_) if !key.modifiers.is_empty() => {}
                        KeyCode::Char(c) => {
                            password.push(c);
                            print!("*");
                            io::stdout().flush()?;
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            Ok(password)
        })();
        terminal::disable_raw_mode()?;
        println!();
        result
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.values.insert(key.into(), value.into());
    }

    pub fn remove(&mut self, key: &str) {
        self.values.remove(key);
    }

    pub fn keys(&self) -> Vec<String> {
        self.values.keys().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn load(path: &PathBuf, password: &str) -> Result<Self> {
        let bytes = fs::read(path).context("Could not read encrypted credential vault")?;
        if bytes.len() < 4 + 16 + 12 + 16 || &bytes[..4] != MAGIC {
            return Err(anyhow!("Credential vault has an unsupported or damaged format"));
        }
        let salt = &bytes[4..20];
        let nonce_bytes: [u8; 12] = bytes[20..32].try_into()?;
        let mut key_bytes = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            NonZeroU32::new(ITERATIONS).unwrap(),
            salt,
            password.as_bytes(),
            &mut key_bytes,
        );
        let key = LessSafeKey::new(UnboundKey::new(&aead::AES_256_GCM, &key_bytes).map_err(|_| anyhow!("Could not initialize vault encryption"))?);
        key_bytes.fill(0);
        let mut ciphertext = bytes[32..].to_vec();
        let plaintext = key
            .open_in_place(
                Nonce::assume_unique_for_key(nonce_bytes),
                Aad::from(MAGIC.as_slice()),
                &mut ciphertext,
            )
            .map_err(|_| anyhow!("Incorrect password or corrupted credential vault"))?;
        let mut decoded = plaintext.to_vec();
        let store = serde_json::from_slice(&decoded).context("Credential vault contents are invalid")?;
        decoded.fill(0);
        ciphertext.fill(0);
        Ok(store)
    }

    pub fn save(&self, path: &PathBuf, password: &str) -> Result<()> {
        let parent = path.parent().ok_or_else(|| anyhow!("Vault path has no parent directory"))?;
        fs::create_dir_all(parent).context("Could not create credential directory")?;
        let random = SystemRandom::new();
        let mut salt = [0u8; 16];
        let mut nonce_bytes = [0u8; 12];
        random.fill(&mut salt).map_err(|_| anyhow!("Could not generate vault salt"))?;
        random.fill(&mut nonce_bytes).map_err(|_| anyhow!("Could not generate vault nonce"))?;
        let mut key_bytes = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            NonZeroU32::new(ITERATIONS).unwrap(),
            &salt,
            password.as_bytes(),
            &mut key_bytes,
        );
        let key = LessSafeKey::new(UnboundKey::new(&aead::AES_256_GCM, &key_bytes)?);
        key_bytes.fill(0);
        let mut plaintext = serde_json::to_vec(self)?;
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::from(MAGIC),
            &mut plaintext,
        )
        .map_err(|_| anyhow!("Could not encrypt credential vault"))?;
        let mut output = Vec::with_capacity(32 + plaintext.len());
        output.extend_from_slice(MAGIC);
        output.extend_from_slice(&salt);
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&plaintext);
        plaintext.fill(0);
        let temporary = path.with_extension("enc.tmp");
        fs::write(&temporary, output).context("Could not write encrypted credential vault")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
        }
        fs::rename(&temporary, path).context("Could not save encrypted credential vault")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_round_trip_and_rejects_wrong_password() {
        let path = std::env::temp_dir().join(format!("cluster-vault-{}.enc", std::process::id()));
        let mut store = SecretStore::default();
        store.set("OPENAI_API_KEY", "example-secret");
        store.save(&path, "correct horse battery staple").unwrap();
        let loaded = SecretStore::load(&path, "correct horse battery staple").unwrap();
        assert_eq!(loaded.get("OPENAI_API_KEY"), Some("example-secret"));
        assert!(SecretStore::load(&path, "wrong password").is_err());
        fs::remove_file(path).unwrap();
    }
}
