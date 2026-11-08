//! Multi-profile account registry (WP16 Sprint 5-6).
//!
//! Each profile has isolated:
//! - SQLite databases (messages.db, e2ee.db)
//! - Session credentials
//! - Conversation state
//!
//! Profile switching does not require re-login (session cache per-profile).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub uid: String,
    pub phone: Option<String>,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub last_active: i64,
    pub session_file: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ProfileRegistry {
    pub profiles: HashMap<String, Profile>,
    pub active_profile: Option<String>,
}

impl ProfileRegistry {
    pub fn load() -> Result<Self> {
        let path = registry_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content).context("Invalid profile registry")
    }

    pub fn save(&self) -> Result<()> {
        let path = registry_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn add_profile(&mut self, profile: Profile) {
        self.profiles.insert(profile.name.clone(), profile);
    }

    pub fn remove_profile(&mut self, name: &str) -> Option<Profile> {
        self.profiles.remove(name)
    }

    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    pub fn set_active(&mut self, name: &str) -> Result<()> {
        anyhow::ensure!(self.profiles.contains_key(name), "Profile not found");
        self.active_profile = Some(name.to_string());
        Ok(())
    }

    pub fn active(&self) -> Option<&Profile> {
        self.active_profile.as_ref().and_then(|name| self.profiles.get(name))
    }
}

pub fn registry_path() -> PathBuf {
    crate::storage::root().join("profiles.json")
}

pub fn profile_root(profile_name: &str) -> PathBuf {
    crate::storage::root().join("profiles").join(profile_name)
}

pub fn profile_messages_db(profile_name: &str) -> PathBuf {
    profile_root(profile_name).join("messages.db")
}

pub fn profile_e2ee_db(profile_name: &str) -> PathBuf {
    profile_root(profile_name).join("e2ee.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_roundtrip() {
        let mut reg = ProfileRegistry::default();
        reg.add_profile(Profile {
            name: "alice".into(),
            uid: "123".into(),
            phone: Some("+1234567890".into()),
            display_name: "Alice".into(),
            avatar_url: None,
            last_active: 1234567890,
            session_file: PathBuf::from("alice.json"),
        });
        reg.set_active("alice").unwrap();
        
        let json = serde_json::to_string(&reg).unwrap();
        let parsed: ProfileRegistry = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.active_profile, Some("alice".to_string()));
        assert_eq!(parsed.profiles.len(), 1);
    }
}
