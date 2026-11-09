//! Parse synthetic prekey bundles for the local crypto model.
//! Official server schemas remain unverified (re-notes/36 section 8); no network adapter is exposed.

use anyhow::Result;
use serde_json::Value;

/// A synthetic peer prekey bundle; not a verified server response schema.
///
/// Local fixtures are parsed explicitly so key and signature lengths are validated.
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

/// Parse a bundle from a server response.  Accepts both the nested `data` shape
/// and a flat shape, and tolerates hex or base64 encoded keys.
pub fn parse_bundle(dec: &Value) -> Result<RemoteBundle> {
    if dec.get("error_code").is_some() {
        let code = dec
            .get("error_code")
            .and_then(crate::api_response::integer)
            .ok_or_else(|| anyhow::anyhow!("Mã lỗi bundle không hợp lệ"))?;
        anyhow::ensure!(code == 0, "Máy chủ từ chối bundle E2EE ({code})");
    }
    let node = dec.get("data").unwrap_or(dec);
    let error = node.get("error_code").and_then(|v| v.as_i64()).unwrap_or(0);
    anyhow::ensure!(
        error == 0,
        "Máy chủ trả lỗi khi lấy bundle E2EE (error_code={error})"
    );

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
    let signature = decode_signature_64(
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
        .and_then(|raw| decode_any_with_len(raw, 32).ok())
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
pub fn decode_any_with_len(raw: &str, len: usize) -> Result<Vec<u8>> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let trimmed = raw.trim();
    // 1. If length matches hex representation (len * 2) and all chars are hex digits, decode as hex
    if trimmed.len() == len * 2 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(v) = hex::decode(trimmed) {
            if v.len() == len {
                return Ok(v);
            }
        }
    }
    // 2. Try Base64 decode
    if let Ok(v) = B64.decode(trimmed) {
        if v.len() == len {
            return Ok(v);
        }
    }
    // 3. Fallback hex decode
    if trimmed.len().is_multiple_of(2) && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(v) = hex::decode(trimmed) {
            if v.len() == len {
                return Ok(v);
            }
        }
    }
    anyhow::bail!("Không giải mã được khóa E2EE (độ dài kỳ vọng: {len})")
}

fn decode_key_bytes(value: &Value, len: usize) -> Result<Vec<u8>> {
    let raw = value
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Khóa E2EE không phải chuỗi"))?;
    decode_any_with_len(raw, len)
}

pub fn decode_key32(value: &Value) -> Result<[u8; 32]> {
    let bytes = decode_key_bytes(value, 32)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

/// Decode a 64-byte Ed25519 signature from base64 or hex JSON value (audit A03).
pub fn decode_signature_64(value: &Value) -> Result<[u8; 64]> {
    let bytes = decode_key_bytes(value, 64)?;
    let mut out = [0u8; 64];
    out.copy_from_slice(&bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

    /// Audit A03: test decode_signature_64 directly with 64-byte base64 and hex,
    /// and ensure it rejects 32-byte or truncated inputs.
    #[test]
    fn test_decode_signature_64_exact_length_a03() {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        let raw64 = [42u8; 64];
        let val_b64 = json!(B64.encode(raw64));
        let decoded = decode_signature_64(&val_b64).expect("valid 64-byte base64");
        assert_eq!(decoded, raw64);

        let hex_str = hex::encode(raw64);
        let val_hex = json!(hex_str);
        let decoded_hex = decode_signature_64(&val_hex).expect("valid 64-byte hex");
        assert_eq!(decoded_hex, raw64);

        // Reject 32-byte key (old truncate bug)
        let val_32 = json!(B64.encode([1u8; 32]));
        assert!(
            decode_signature_64(&val_32).is_err(),
            "Must reject 32-byte input"
        );

        // Reject 63-byte input
        let val_63 = json!(B64.encode([1u8; 63]));
        assert!(
            decode_signature_64(&val_63).is_err(),
            "Must reject 63-byte input"
        );
    }
}
