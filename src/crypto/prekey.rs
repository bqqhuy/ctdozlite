//! Prekey server API client (WP16-1).
//!
//! Endpoints (evidence E2 — route index `re-notes/fixtures/v008-official-route-index.json`):
//! - `POST /api/e2ee/pc/prekey/update` — upload our prekey bundle
//! - `GET  /api/e2ee/pc/prekey/get` — fetch a peer's prekey bundle
//!
//! **Unverified (E3 inference):** the exact request/response schema of these
//! routes was not captured live.  The shapes below follow the Signal Protocol
//! bundle schema documented in
//! `re-notes/36-zalo-e2ee-signal-protocol-analysis.md` §4 and are used defensively:
//! every field is optional and a malformed response degrades to "no session"
//! rather than sending undecryptable ciphertext.

use anyhow::Result;
use serde_json::{json, Value};

use crate::session::Session;
use super::keystore::PreKeyBundle;

/// Upload our prekey bundle to the server.
pub async fn update_prekeys(s: &Session, bundle: &PreKeyBundle) -> Result<()> {
    let mut payload = json!({
        "identityKey": base64_encode(&bundle.identity_key),
        "registrationId": bundle.registration_id,
        "signedPreKey": {
            "keyId": bundle.signed_pre_key.key_id,
            "publicKey": base64_encode(&bundle.signed_pre_key.public_key),
            "signature": base64_encode(&bundle.signed_pre_key.signature),
        },
        "preKeys": bundle.pre_keys.iter().map(|k| json!({
            "keyId": k.key_id,
            "publicKey": base64_encode(&k.public_key),
        })).collect::<Vec<_>>(),
    });
    // The server expects the same envelope as other Zalo APIs.
    payload["clientId"] = json!(s.imei);

    let url = format!(
        "{}{}?{}",
        s.resolve_service_url("chat", "/api/e2ee/pc/prekey/update"),
        "",
        s.common_params()
    );
    let params = s.encoded_params(&payload)?;
    let full = format!("{url}&params={params}");
    let raw = s.post_form(&full, format!("params={params}")).await?;
    let dec = s.maybe_decrypt(raw, s.session_key_bytes()).await;
    let code = dec.get("error_code").and_then(|v| v.as_i64()).unwrap_or(0);
    anyhow::ensure!(code == 0, "Máy chủ từ chối đăng ký khóa E2EE (error_code={code})");
    Ok(())
}

/// Fetch a peer's prekey bundle.
///
/// Built manually by [`parse_bundle`] from a decrypted JSON `Value` — never
/// deserialized directly, so no `Deserialize` impl is needed (serde cannot
/// derive arrays longer than 32 elements anyway, audit A03's 64-byte signature).
#[derive(Debug)]
pub struct RemoteBundle {
    pub identity_key: [u8; 32],
    pub registration_id: u32,
    pub signed_pre_key_id: u32,
    pub signed_pre_key: [u8; 32],
    pub signature: [u8; 64],
    /// Ed25519 identity key of the peer, when the server exposes it separately
    /// from the Curve25519 DH identity key.  Used to verify the signed prekey.
    pub signing_identity_key: Option<[u8; 32]>,
    pub one_time_pre_key: Option<([u8; 32], u32)>,
}

pub async fn fetch_bundle(s: &Session, peer_id: &str) -> Result<RemoteBundle> {
    let payload = json!({ "toid": peer_id, "clientId": s.imei });
    let params = s.encoded_params(&payload)?;
    let url = format!(
        "{}{}?{}&params={params}",
        s.resolve_service_url("chat", "/api/e2ee/pc/prekey/get"),
        "",
        s.common_params()
    );
    let raw = s.post_form(&url, format!("params={params}")).await?;
    let dec = s.maybe_decrypt(raw, s.session_key_bytes()).await;
    parse_bundle(&dec)
}

/// Parse a bundle from a server response.  Accepts both the nested `data` shape
/// and a flat shape, and tolerates hex or base64 encoded keys.
pub fn parse_bundle(dec: &Value) -> Result<RemoteBundle> {
    let node = dec.get("data").unwrap_or(dec);
    let error = node.get("error_code").and_then(|v| v.as_i64()).unwrap_or(0);
    anyhow::ensure!(error == 0, "Máy chủ trả lỗi khi lấy bundle E2EE (error_code={error})");

    let identity_key = decode_key32(
        pick(node, &["identityKey", "identity_key"])
            .ok_or_else(|| anyhow::anyhow!("Thiếu identityKey trong bundle E2EE"))?,
    )?;
    let registration_id = node
        .get("registrationId")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let spk = node
        .get("signedPreKey")
        .ok_or_else(|| anyhow::anyhow!("Thiếu signedPreKey trong bundle E2EE"))?;
    let signed_pre_key_id = spk.get("keyId").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let signed_pre_key = decode_key32(
        spk.get("publicKey")
            .ok_or_else(|| anyhow::anyhow!("Thiếu publicKey của signedPreKey"))?,
    )?;
    let signature = decode_key64(
        spk.get("signature")
            .ok_or_else(|| anyhow::anyhow!("Thiếu signature của signedPreKey"))?,
    )?;

    let mut one_time_pre_key = None;
    if let Some(list) = node.get("preKeys").and_then(|v| v.as_array()) {
        if let Some(first) = list.first() {
            if let (Some(key), Some(id)) = (
                first.get("publicKey"),
                first.get("keyId").and_then(|v| v.as_u64()),
            ) {
                if let Ok(bytes) = decode_key32(key) {
                    one_time_pre_key = Some((bytes, id as u32));
                }
            }
        }
    }

    let signing_identity_key = node
        .get("signingIdentityKey")
        .and_then(|v| v.as_str())
        .and_then(|raw| decode_any(raw).ok())
        .filter(|b| b.len() == 32)
        .map(|b| {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b[..32]);
            arr
        });

    Ok(RemoteBundle {
        identity_key,
        signing_identity_key,
        registration_id,
        signed_pre_key_id,
        signed_pre_key,
        signature,
        one_time_pre_key,
    })
}

fn pick<'a>(node: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    for k in keys {
        if let Some(v) = node.get(*k) {
            return Some(v);
        }
    }
    None
}

/// Decode a key or signature that may be base64 or hex encoded, enforcing the
/// exact expected length.
///
/// Audit A03: the previous helper took a `len` parameter but always returned
/// `[u8; 32]`, silently truncating the 64-byte Ed25519 signature — every
/// production-parsed bundle then failed signature verification even when
/// correctly signed.  Length is now part of the return type, not a runtime
/// hint.
fn decode_key_bytes(value: &Value, len: usize) -> Result<Vec<u8>> {
    let raw = value
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Khóa E2EE không phải chuỗi"))?;
    let bytes = decode_any(raw)?;
    anyhow::ensure!(
        bytes.len() == len,
        "Khóa E2EE sai độ dài ({} != {len})",
        bytes.len()
    );
    Ok(bytes)
}

fn decode_key32(value: &Value) -> Result<[u8; 32]> {
    let bytes = decode_key_bytes(value, 32)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn decode_key64(value: &Value) -> Result<[u8; 64]> {
    let bytes = decode_key_bytes(value, 64)?;
    let mut out = [0u8; 64];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn decode_any(raw: &str) -> Result<Vec<u8>> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let trimmed = raw.trim();
    if let Ok(v) = B64.decode(trimmed) {
        return Ok(v);
    }
    // Hex fallback
    if trimmed.len().is_multiple_of(2) && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        let mut out = Vec::with_capacity(trimmed.len() / 2);
        let bytes = trimmed.as_bytes();
        for pair in bytes.chunks(2) {
            let s = std::str::from_utf8(pair)?;
            out.push(u8::from_str_radix(s, 16)?);
        }
        return Ok(out);
    }
    anyhow::bail!("Không giải mã được khóa E2EE")
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    B64.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_parsing_accepts_flat_shape() {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        let dec = json!({
            "error_code": 0,
            "identityKey": B64.encode([1u8; 32]),
            "registrationId": 12345,
            "signedPreKey": {
                "keyId": 1,
                "publicKey": B64.encode([2u8; 32]),
                "signature": B64.encode([3u8; 64]),
            },
            "preKeys": [{"keyId": 100, "publicKey": B64.encode([4u8; 32])}],
        });
        let b = parse_bundle(&dec).unwrap();
        assert_eq!(b.registration_id, 12345);
        assert_eq!(b.identity_key, [1u8; 32]);
        assert_eq!(b.signed_pre_key_id, 1);
        assert_eq!(b.signature, [3u8; 64]);
        assert_eq!(b.one_time_pre_key.unwrap().1, 100);
    }

    #[test]
    fn bundle_parsing_rejects_error_response() {
        let dec = json!({"error_code": -5, "data": {"error_code": -5}});
        assert!(parse_bundle(&dec).is_err());
    }

    /// Audit A03: the old `decode_key` helper took a length parameter but always
    /// returned `[u8; 32]`, truncating the 64-byte Ed25519 signature — every
    /// production bundle then failed `verify_signed_prekey` even when correctly
    /// signed.  A signature with distinct bytes must round-trip all 64 bytes
    /// through the production parser.
    #[test]
    fn bundle_signature_keeps_all_64_bytes() {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        let signature: Vec<u8> = (0..64u8).collect();
        let dec = json!({
            "error_code": 0,
            "identityKey": B64.encode([1u8; 32]),
            "signedPreKey": {
                "keyId": 1,
                "publicKey": B64.encode([2u8; 32]),
                "signature": B64.encode(&signature),
            },
        });
        let b = parse_bundle(&dec).unwrap();
        let expected: [u8; 64] = signature.clone().try_into().unwrap();
        assert_eq!(b.signature, expected);
    }

    /// The parser must enforce the exact 64-byte length for signatures —
    /// a malformed bundle fails loudly instead of reaching signature
    /// verification with truncated bytes.
    #[test]
    fn bundle_parsing_rejects_short_signature() {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        let dec = json!({
            "error_code": 0,
            "identityKey": B64.encode([1u8; 32]),
            "signedPreKey": {
                "keyId": 1,
                "publicKey": B64.encode([2u8; 32]),
                "signature": B64.encode([3u8; 63]),
            },
        });
        assert!(parse_bundle(&dec).is_err());
    }
}
