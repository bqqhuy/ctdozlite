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

/// Check if an active E2EE ratchet session exists for a peer (audit A01).
pub fn is_thread_e2ee(profile_id: &str, peer_id: &str) -> bool {
    if profile_id.is_empty() || peer_id.is_empty() {
        return false;
    }
    let _lock = lock_e2ee();
    get_manager(profile_id)
        .and_then(|mgr| mgr.load_session(peer_id))
        .map(|opt| opt.is_some())
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

/// Check if a byte slice matches an E2EE wire envelope (version 1, type 1 or 3).
pub fn is_e2ee_wire_payload(payload: &[u8]) -> bool {
    crate::crypto::wire::deserialize(payload).is_ok()
}

/// Extract ciphertext bytes from raw message value if present (base64 encoded).
pub fn extract_ciphertext_bytes(raw: &Value) -> Option<Vec<u8>> {
    use base64::Engine;
    let b64_str = raw.get("content")
        .or_else(|| raw.get("message"))
        .or_else(|| raw.get("msg"))
        .and_then(|v| {
            if let Some(s) = v.as_str() {
                Some(s)
            } else if let Some(obj) = v.as_object() {
                obj.get("ciphertext")
                    .or_else(|| obj.get("content"))
                    .or_else(|| obj.get("msg"))
                    .or_else(|| obj.get("message"))
                    .and_then(Value::as_str)
            } else {
                None
            }
        })?;
    let trimmed = b64_str.trim();
    if trimmed.is_empty() {
        return None;
    }
    base64::engine::general_purpose::STANDARD.decode(trimmed).ok()
}

/// Check if a raw message value represents an E2EE message based on
/// metadata (e2eeStatus ∈ [3000, 4000), isE2EE == 1) or payload header (version 1, type 1 or 3).
pub fn is_e2ee_message(raw: &Value) -> bool {
    if let Some(status) = detect_e2ee_status(raw) {
        if (3000..4000).contains(&status) {
            return true;
        }
    }
    if raw.get("isE2EE")
        .and_then(|v| v.as_i64().or_else(|| v.as_bool().map(|b| if b { 1 } else { 0 })))
        .map(|n| n > 0)
        .unwrap_or(false)
    {
        return true;
    }
    if let Some(bytes) = extract_ciphertext_bytes(raw) {
        if is_e2ee_wire_payload(&bytes) {
            return true;
        }
    }
    false
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
    #[test]
    fn e2ee_detection_from_metadata_and_wire_header_a10() {
        use base64::Engine;

        // 1. Metadata: isE2EE == 1
        let msg_flag = json!({"isE2EE": 1, "content": "not_base64"});
        assert!(is_e2ee_message(&msg_flag));

        // 2. Metadata: isE2EE == true
        let msg_bool = json!({"isE2EE": true, "content": "xyz"});
        assert!(is_e2ee_message(&msg_bool));

        // 3. Metadata: e2eeStatus in 3000..4000
        let msg_status = json!({"e2eeStatus": 3005, "content": "hello"});
        assert!(is_e2ee_message(&msg_status));

        // 4. Payload header (version 1, type 1) without metadata
        let header = crate::crypto::ratchet::MessageHeader {
            msg_type: crate::crypto::wire::TYPE_WHISPER,
            dh_public: [5u8; 32],
            pn: 0,
            n: 1,
        };
        let wire = crate::crypto::wire::serialize_whisper(&header, &[99u8; 32]);
        let wire_b64 = base64::engine::general_purpose::STANDARD.encode(&wire);

        let msg_wire = json!({"content": wire_b64});
        assert!(is_e2ee_message(&msg_wire), "Must detect E2EE message via wire header");

        // 5. Normal plaintext message
        let msg_plain = json!({"content": "Xin chao buoi sang!"});
        assert!(!is_e2ee_message(&msg_plain), "Plaintext must not be detected as E2EE");
    }

}
