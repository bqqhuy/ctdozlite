//! Fail-closed boundary for E2EE server operations whose official contracts are unverified.
//! Local synthetic ratchet primitives remain available independently for tests.

use anyhow::{Context, Result};
use serde_json::Value;
use std::sync::Mutex;

use crate::crypto::session::E2eeManager;

thread_local! {
    // SQLite connections stay on their owning worker thread; only its last account is retained.
    static MANAGER: std::cell::RefCell<Option<E2eeManager>> = const { std::cell::RefCell::new(None) };
}

/// Serialize crypto operations without repeatedly opening the database for every message.
pub(crate) fn with_manager<T>(
    profile: &str,
    operation: impl FnOnce(&E2eeManager) -> Result<T>,
) -> Result<T> {
    let _lock = lock_e2ee()?;
    MANAGER.with(|slot| {
        let mut slot = slot
            .try_borrow_mut()
            .map_err(|_| anyhow::anyhow!("Tác vụ E2EE đang được xử lý"))?;
        if slot
            .as_ref()
            .is_none_or(|manager| manager.profile_id() != profile)
        {
            *slot = Some(get_manager(profile)?);
        }
        operation(
            slot.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Không mở được E2EE"))?,
        )
    })
}

// Session policy checks and encrypted-message markers share one short critical section.
static E2EE_GLOBAL_LOCK: std::sync::LazyLock<Mutex<()>> =
    std::sync::LazyLock::new(|| Mutex::new(()));

pub(crate) fn lock_e2ee() -> Result<std::sync::MutexGuard<'static, ()>> {
    E2EE_GLOBAL_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("Không đọc được khóa trạng thái E2EE"))
}

/// Enforce the default refusal, with an explicit exception for the opt-in experiment.
/// # Errors
/// Reports the missing contract unless the explicit experimental launch is enabled.
pub fn require_verified_contract() -> Result<()> {
    if crate::e2ee_trial::enabled() {
        Ok(())
    } else {
        Err(unverified_contract_error())
    }
}

/// Explain the unavailable production contract without exposing keys or message content.
pub(crate) fn unverified_contract_error() -> anyhow::Error {
    anyhow::anyhow!(
        "E2EE chưa khả dụng: chưa xác minh định dạng gói tin và API khóa của Zalo PC/Web"
    )
}

/// Get or create E2EE manager for a profile.
///
/// NOTE: Each call opens a new SQLite connection. The manager itself is not cached;
/// the global lock serializes local session checks.
pub fn get_manager(profile_id: &str) -> Result<E2eeManager> {
    E2eeManager::open(profile_id)
}

/// Check for an existing encrypted session without treating storage failures as plaintext policy.
/// Fail-closed: database read/open errors must NOT be silently swallowed as false.
pub fn check_thread_e2ee(profile_id: &str, peer_id: &str) -> Result<bool> {
    if profile_id.is_empty() || peer_id.is_empty() {
        return Ok(false);
    }
    with_manager(profile_id, |manager| manager.requires_e2ee(peer_id))
        .context("Lỗi đọc trạng thái E2EE từ cơ sở dữ liệu khóa")
}

/// Retain encrypted-message policy even when no local ratchet can decode the message.
/// # Errors
/// Returns storage failures; callers must not interpret them as plaintext permission.
pub fn remember_e2ee_requirement(profile_id: &str, peer_id: &str) -> Result<()> {
    anyhow::ensure!(
        !profile_id.is_empty() && !peer_id.is_empty(),
        "Thiếu tài khoản hoặc người nhận E2EE"
    );
    with_manager(profile_id, |manager| {
        manager.conn().execute(
            "INSERT OR IGNORE INTO e2ee_required(profile_id,peer_id) VALUES(?1,?2)",
            rusqlite::params![profile_id, peer_id],
        )?;
        Ok(())
    })
}

/// Convenience wrapper for is_thread_e2ee that treats errors safely.
pub fn is_thread_e2ee(profile_id: &str, peer_id: &str) -> bool {
    check_thread_e2ee(profile_id, peer_id).unwrap_or(true)
}

/// Decrypt incoming E2EE payload.
///
/// The custom local envelope is not an official Zalo container.
pub fn decrypt_message(
    profile_id: &str,
    peer_id: &str,
    payload: &[u8],
) -> Result<crate::crypto::session::DecryptedMessage> {
    decrypt_received(profile_id, peer_id, None, payload)
}

/// Decrypt a stable network delivery while preserving crash-safe authenticated receipts.
pub(crate) fn decrypt_received(
    profile_id: &str,
    peer_id: &str,
    message_id: Option<&str>,
    payload: &[u8],
) -> Result<crate::crypto::session::DecryptedMessage> {
    use crate::e2ee_trial::{Stage, Trace};
    let mut trace = Trace::new(profile_id, peer_id, Stage::Contract, payload.len());
    let result = (|| {
        require_verified_contract()?;
        trace.stage(Stage::Decode);
        let decoded = crate::crypto::wire::deserialize(payload)?;
        trace.envelope(&decoded, payload.len());
        trace.stage(Stage::Load);
        trace.stage(Stage::DecryptPersist);
        with_manager(profile_id, |manager| match message_id {
            Some(id) => manager.decrypt_received(peer_id, id, payload),
            None => manager.decrypt(peer_id, payload),
        })
    })();
    trace.finish_result(&result);
    result
}

/// Normalize the account history path for private receive-receipt deletion.
pub(crate) fn history_key(path: &std::path::Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    {
        path.trim_start_matches("//?/").to_lowercase()
    }
    #[cfg(not(windows))]
    {
        path
    }
}

/// Remove protected recovery copies when the owning history message is deleted or recalled.
pub(crate) fn purge_received(
    history: Option<&str>,
    peer: &str,
    message: Option<&str>,
) -> Result<()> {
    let Some(history) = history.filter(|path| *path != ":memory:") else {
        return Ok(());
    };
    if !crate::storage::root().join("e2ee.db").exists() {
        return Ok(());
    }
    let _guard = lock_e2ee()?;
    let conn = crate::crypto::store::open_crypto_db()?;
    let history = history_key(std::path::Path::new(history));
    match message {
        Some(id) => conn.execute(
            "DELETE FROM e2ee_received WHERE history_path=?1 AND peer_id=?2 AND message_id=?3",
            rusqlite::params![history, peer, id],
        )?,
        None => conn.execute(
            "DELETE FROM e2ee_received WHERE history_path=?1 AND peer_id=?2",
            rusqlite::params![history, peer],
        )?,
    };
    Ok(())
}

/// Detect E2EE status codes.
pub fn detect_e2ee_status(raw: &Value) -> Option<i32> {
    raw.get("e2eeStatus")
        .and_then(|v| v.as_i64())
        .and_then(|n| i32::try_from(n).ok())
        .filter(|&code| (3000..4000).contains(&code))
}

/// Read explicit encrypted-message metadata; a base64-looking body alone is not policy.
pub fn has_e2ee_metadata(raw: &Value) -> bool {
    detect_e2ee_status(raw).is_some()
        || raw
            .get("isE2EE")
            .is_some_and(|flag| flag.as_bool() == Some(true) || flag.as_i64() == Some(1))
}

/// Check if a byte slice matches an E2EE wire envelope (version 1, type 1 or 3).
pub fn is_e2ee_wire_payload(payload: &[u8]) -> bool {
    crate::crypto::wire::deserialize(payload).is_ok()
}

/// Extract ciphertext bytes from raw message value if present (base64 encoded).
pub fn extract_ciphertext_bytes(raw: &Value) -> Option<Vec<u8>> {
    use base64::Engine;
    let b64_str = raw
        .get("content")
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
    base64::engine::general_purpose::STANDARD
        .decode(trimmed)
        .ok()
}

/// Check if a raw message value represents an E2EE message based on
/// metadata (e2eeStatus ∈ [3000, 4000), isE2EE == 1) or payload header (version 1, type 1 or 3).
pub fn is_e2ee_message(raw: &Value) -> bool {
    if let Some(status) = detect_e2ee_status(raw) {
        if (3000..4000).contains(&status) {
            return true;
        }
    }
    if raw
        .get("isE2EE")
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_bool().map(|b| if b { 1 } else { 0 }))
        })
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

/// Generated encrypted-message labels carry no authenticated plaintext.
pub(crate) fn is_placeholder(text: &str) -> bool {
    matches!(
        text,
        "[Tin nhắn mã hoá]"
            | "[Tin nhắn mã hoá - không giải mã được]"
            | "[Tin nhắn mã hoá - thiếu dữ liệu]"
            | "[Không đọc được trạng thái bảo mật E2EE]"
    )
}

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
        assert!(
            is_e2ee_message(&msg_wire),
            "Must detect E2EE message via wire header"
        );

        // 5. Normal plaintext message
        let msg_plain = json!({"content": "Xin chao buoi sang!"});
        assert!(
            !is_e2ee_message(&msg_plain),
            "Plaintext must not be detected as E2EE"
        );
    }
    #[tokio::test]
    async fn production_send_refuses_unverified_contract_before_key_or_network_work() {
        let session = crate::session::Session::new(false, false, false).unwrap();
        let error =
            crate::chat::send_e2ee_message(&session, "synthetic-owner", "peer", "private text", 1)
                .await
                .unwrap_err();
        assert!(error.to_string().contains("chưa xác minh"));
        assert!(!error.to_string().contains("private text"));
    }
}
