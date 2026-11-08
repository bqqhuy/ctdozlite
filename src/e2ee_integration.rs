//! E2EE integration layer: bridges crypto/session.rs into chat.rs send/receive flow (WP16-3).
//!
//! When isE2EE flag is present in conversation metadata:
//! - Outgoing: encrypt plaintext → wire format → POST /api/e2ee/pc/message/sms
//! - Incoming: detect e2eeStatus ∈ [3000,4000) → decrypt → display plaintext
//! - Error codes: -29 (DECRYPT_FAILED), -30 (PENDING_DATA) → UI warning

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::crypto::session::E2eeManager;
use crate::crypto::prekey::{fetch_bundle, update_prekeys};
use crate::session::Session;

/// Global E2EE managers, keyed by profile_id.
static E2EE_MANAGERS: std::sync::LazyLock<Mutex<HashMap<String, E2eeManager>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Get or create E2EE manager for a profile.
pub fn get_manager(profile_id: &str) -> Result<E2eeManager> {
    let mut map = E2EE_MANAGERS.lock().unwrap();
    if map.contains_key(profile_id) {
        return E2eeManager::open(profile_id);
    }
    let mgr = E2eeManager::open(profile_id)?;
    map.insert(profile_id.to_string(), mgr);
    E2eeManager::open(profile_id)
}

/// Check if conversation has E2EE enabled.
pub fn is_e2ee_enabled(convo: &Value) -> bool {
    convo.get("isE2EE")
        .and_then(|v| v.as_u64())
        .map(|n| n > 0)
        .unwrap_or(false)
}

/// Encrypt plaintext for E2EE conversation.
pub async fn encrypt_message(
    session: &Session,
    profile_id: &str,
    peer_id: &str,
    plaintext: &str,
) -> Result<Vec<u8>> {
    let mgr = get_manager(profile_id)?;
    
    // Try existing session first
    if mgr.load_session(peer_id)?.is_some() {
        return mgr.encrypt(peer_id, None, plaintext.as_bytes());
    }
    
    // No session → fetch peer's prekey bundle
    let bundle = fetch_bundle(session, peer_id).await?;
    
    // Upload our bundle if not yet done
    let our_bundle = mgr.local_bundle()?;
    if let Err(e) = update_prekeys(session, &our_bundle).await {
        eprintln!("Cảnh báo: không upload được prekey bundle: {e}");
    }
    
    // Start session with first message
    mgr.start_session(peer_id, &bundle, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("Không khởi tạo được phiên E2EE: {e}"))
}

/// Decrypt incoming E2EE payload.
pub fn decrypt_message(
    profile_id: &str,
    peer_id: &str,
    payload: &[u8],
) -> Result<crate::crypto::session::DecryptedMessage> {
    let mgr = get_manager(profile_id)?;
    mgr.decrypt(peer_id, payload)
        .context("Giải mã E2EE thất bại")
}

/// Detect E2EE status codes.
pub fn detect_e2ee_status(raw: &Value) -> Option<i32> {
    raw.get("e2eeStatus")
        .and_then(|v| v.as_i64())
        .map(|n| n as i32)
        .filter(|&code| (3000..4000).contains(&code))
}

/// Error code constants
pub const E2EE_DECRYPT_FAILED: i64 = -29;
pub const E2EE_PENDING_DATA: i64 = -30;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn e2ee_status_detection() {
        let msg = json!({"e2eeStatus": 3001});
        assert_eq!(detect_e2ee_status(&msg), Some(3001));
        
        let normal = json!({"status": 1});
        assert_eq!(detect_e2ee_status(&normal), None);
    }
}
