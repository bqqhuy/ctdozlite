//! Multi-profile account registry (WP16 Sprint 5-6).
//!
//! Each profile has isolated:
//! - Account-keyed history and crypto records
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
        crate::storage::atomic_write(&path, content.as_bytes())?;
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
        anyhow::ensure!(self.profiles.contains_key(name), "Không tìm thấy tài khoản");
        self.active_profile = Some(name.to_string());
        Ok(())
    }

    pub fn active(&self) -> Option<&Profile> {
        self.active_profile
            .as_ref()
            .and_then(|name| self.profiles.get(name))
    }
}

pub fn registry_path() -> PathBuf {
    crate::storage::root().join("profiles.json")
}

static REGISTRY_IO: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Publish the target and revoke stale profile writers under the same registry lock.
pub fn activate_for_switch(current: &crate::session::Session, name: &str) -> Result<()> {
    let _guard = REGISTRY_IO
        .lock()
        .map_err(|_| anyhow::anyhow!("Không khóa được danh sách tài khoản"))?;
    let mut registry = ProfileRegistry::load()?;
    registry.set_active(name)?;
    registry.save()?;
    current.revoke_runtime();
    Ok(())
}

/// Register a real remembered login and its DPAPI-protected account-local session.
/// # Errors
/// Returns credential/registry persistence failures without publishing a broken profile.
pub fn remember_session(session: &crate::session::Session) -> Result<()> {
    if !crate::storage::remember_login()? {
        return Ok(());
    }
    anyhow::ensure!(
        session.is_active() && !session.account_key().is_empty(),
        "Chưa có tài khoản để ghi nhớ"
    );
    let _guard = REGISTRY_IO
        .lock()
        .map_err(|_| anyhow::anyhow!("Không khóa được danh sách tài khoản"))?;
    anyhow::ensure!(session.is_active(), "Tài khoản đã thay đổi");
    let uid = session.account_key();
    let path = crate::storage::account_file(uid, "session.dat");
    session.save_to_file(path.to_str().context("Đường dẫn phiên không hợp lệ")?)?;
    let mut registry = ProfileRegistry::load()?;
    let name = registry
        .profiles
        .values()
        .find(|profile| profile.uid == uid)
        .map(|profile| profile.name.clone())
        .unwrap_or_else(|| {
            use sha2::{Digest, Sha256};
            format!("account-{:x}", Sha256::digest(uid.as_bytes()))
        });
    let login = session
        .login
        .as_ref()
        .context("Thiếu thông tin tài khoản")?;
    registry.add_profile(Profile {
        name: name.clone(),
        uid: uid.into(),
        phone: None,
        display_name: login.display_name.clone(),
        avatar_url: (!login.avatar.is_empty()).then(|| login.avatar.clone()),
        last_active: crate::login::unix_now(),
        session_file: path,
    });
    registry.set_active(&name)?;
    registry.save()
}

/// Forget only the selected account's cached credential; history and other accounts remain.
pub fn forget_session(owner: &str) -> Result<()> {
    let _guard = REGISTRY_IO
        .lock()
        .map_err(|_| anyhow::anyhow!("Không khóa được danh sách tài khoản"))?;
    let path = crate::storage::account_file(owner, "session.dat");
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut registry = ProfileRegistry::load()?;
    registry.profiles.retain(|_, profile| profile.uid != owner);
    if registry
        .active_profile
        .as_ref()
        .is_some_and(|name| !registry.profiles.contains_key(name))
    {
        registry.active_profile = None;
    }
    registry.save()
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
