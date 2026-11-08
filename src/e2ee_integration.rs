//! E2EE integration layer: bridges crypto/session.rs into chat.rs send/receive flow (WP16-3).
//!
//! When isE2EE flag is present in conversation metadata:
//! - Outgoing: encrypt plaintext → wire format → POST /api/e2ee/pc/message/sms
//! - Incoming: detect e2eeStatus ∈ [3000,4000) → decrypt → display plaintext
//! - Error codes: -29 (DECRYPT_FAILED), -30 (PENDING_DATA) → UI warning
//!
//! ## A08 fix (P1/S): atomic session operations
//!
//! Each (profile_id, peer_id) pair has a dedicated Mutex to serialize encrypt/decrypt operations.
//! This prevents concurrent operations from loading the same session state, both advancing the
//! chain key, then racing to save → last write wins but both used overlapping counters (nonce reuse).

use anyhow::{Context, Result};
use serde_json::Value;
use std::sync::Mutex;

use crate::crypto::session::E2eeManager;
use crate::crypto::prekey::{fetch_bundle, update_prekeys};
use crate::session::Session;

/// Global lock serializing all E2EE operations (A08 fix).
///
/// Simplified approach: instead of per-session locks (complex lifetime),
/// serialize ALL ratchet operations globally. This is acceptable because:
/// - E2EE operations are infrequent (only encrypted messages)
/// - Most messages are plaintext (groups, non-E2EE chats)
/// - Single-writer per database connection already exists
/// - Contention only occurs when user sends to multiple E2EE peers simultaneously
static E2EE_GLOBAL_LOCK: std::sync::LazyLock<Mutex<()>> =
    std::sync::LazyLock::new(|| Mutex::new(()));

/// Acquire global E2EE serialization lock.
///
/// Returns a guard that serializes all encrypt/decrypt operations across all sessions,
/// preventing concurrent chain key advancement (A08 fix).
///
/// This is a coarse-grained lock but sufficient for current usage patterns where
/// E2EE messages are rare compared to total message volume.
fn lock_e2ee() -> std::sync::MutexGuard<'static, ()> {
    E2EE_GLOBAL_LOCK.lock().unwrap()
}

/// Get or create E2EE manager for a profile.
///
/// NOTE: Each call opens a new SQLite connection. The manager itself is not cached;
/// only SESSION_LOCKS provides serialization across concurrent operations.
pub fn get_manager(profile_id: &str) -> Result<E2eeManager> {
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
///
/// A08 fix: serializes all E2EE operations globally to prevent concurrent
/// chain key advancement on any session.
pub async fn encrypt_message(
    session: &Session,
    profile_id: &str,
    peer_id: &str,
    plaintext: &str,
) -> Result<Vec<u8>> {
    let mgr = get_manager(profile_id)?;

    // Try existing session first under lock
    {
        let _lock = lock_e2ee();
        if mgr.load_session(peer_id)?.is_some() {
            return mgr.encrypt(peer_id, None, plaintext.as_bytes());
        }
    }

    // No session → fetch peer's prekey bundle (network I/O without holding lock)
    let bundle = fetch_bundle(session, peer_id).await?;

    // Upload our bundle if not yet done
    let our_bundle = mgr.local_bundle()?;
    if let Err(e) = update_prekeys(session, &our_bundle).await {
        eprintln!("Cảnh báo: không upload được prekey bundle: {e}");
    }

    // Start session with first message under lock (re-check if another task established it)
    let _lock = lock_e2ee();
    if mgr.load_session(peer_id)?.is_some() {
        return mgr.encrypt(peer_id, None, plaintext.as_bytes());
    }
    mgr.start_session(peer_id, &bundle, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("Không khởi tạo được phiên E2EE: {e}"))
}

/// Decrypt incoming E2EE payload.
///
/// A08 fix: serializes all E2EE operations globally.
pub fn decrypt_message(
    profile_id: &str,
    peer_id: &str,
    payload: &[u8],
) -> Result<crate::crypto::session::DecryptedMessage> {
    let _lock = lock_e2ee();
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
