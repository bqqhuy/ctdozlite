//! Wire encoding for E2EE payloads (WP16-2).
//!
//! # Fork deviation (E3 - unverified against live Zalo PC capture)
//! The exact ciphertext container used by Zalo's official client was **not**
//! captured live (see `re-notes/36-zalo-e2ee-signal-protocol-analysis.md` §8,
//! item 1), so this module defines a self-describing binary envelope that
//! carries the Signal Protocol structures verbatim:
//!
//! ```text
//! byte 0      : format version (1)
//! byte 1      : message type (1 = WhisperMessage, 3 = PreKeyWhisperMessage)
//! bytes 2..34 : sender ratchet DH public key
//! bytes 34..38: pn (u32 LE) — messages in the previous chain
//! bytes 38..42: n  (u32 LE) — index in the current chain
//! -- PreKeyWhisperMessage only --
//! bytes 42..46: registration id (u32 LE)
//! bytes 46..50: signed prekey id (u32 LE)
//! bytes 50..54: one-time prekey id (u32 LE, 0 = None)
//! bytes 54..86: base key (X3DH ephemeral public key)
//! bytes 86..118: identity key (sender's Curve25519 identity public key)
//! remainder    : ciphertext || 32-byte HMAC-SHA256
//! ```
//!
//! The layout is stable and versioned so it can be swapped for the official
//! container once a live capture confirms the server format.

use anyhow::{Context, Result};

use super::ratchet::MessageHeader;

/// Current envelope version.
pub const WIRE_VERSION: u8 = 1;

/// Message type: regular ratchet message.
pub const TYPE_WHISPER: u8 = 1;
/// Message type: first message of a session (carries the X3DH base key).
pub const TYPE_PREKEY_WHISPER: u8 = 3;

/// Header information carried by a `PreKeyWhisperMessage`.
/// Header information carried by a `PreKeyWhisperMessage`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PreKeyBundleWire {
    /// Sender's registration id.
    pub registration_id: u32,
    /// Id of the sender's signed prekey.
    pub signed_pre_key_id: u32,
    /// Optional id of the receiver's one-time prekey used in X3DH.
    pub one_time_pre_key_id: Option<u32>,
    /// X3DH ephemeral (base) key.
    pub base_key: [u8; 32],
    /// Sender's identity public key.
    pub identity_key: [u8; 32],
}

/// A decoded `PreKeyWhisperMessage`.
#[derive(Debug, Clone)]
pub struct PreKeyMessage {
    /// X3DH bundle information.
    pub bundle: PreKeyBundleWire,
    /// Ratchet message header.
    pub header: MessageHeader,
    /// Ciphertext with the trailing 32-byte MAC.
    pub ciphertext: Vec<u8>,
}

/// A decoded `WhisperMessage`.
#[derive(Debug, Clone)]
pub struct WhisperMessage {
    /// Ratchet message header.
    pub header: MessageHeader,
    /// Ciphertext with the trailing 32-byte MAC.
    pub ciphertext: Vec<u8>,
}

/// Either kind of decoded E2EE payload.
#[derive(Debug, Clone)]
pub enum DecodedMessage {
    /// First message of a session.
    PreKey(PreKeyMessage),
    /// Continuation of an established session.
    Whisper(WhisperMessage),
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn read_u32(data: &[u8], at: usize) -> Result<u32> {
    let slice = data.get(at..at + 4).context("Gói tin E2EE bị cắt cụt")?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

/// Serialise a regular `WhisperMessage`.
pub fn serialize_whisper(header: &MessageHeader, ciphertext: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(42 + ciphertext.len());
    out.push(WIRE_VERSION);
    out.push(TYPE_WHISPER);
    out.extend_from_slice(&header.dh_public);
    put_u32(&mut out, header.pn);
    put_u32(&mut out, header.n);
    out.extend_from_slice(ciphertext);
    out
}

/// Serialise a `PreKeyWhisperMessage`.
pub fn serialize_prekey(
    bundle: &PreKeyBundleWire,
    header: &MessageHeader,
    ciphertext: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(118 + ciphertext.len());
    out.push(WIRE_VERSION);
    out.push(TYPE_PREKEY_WHISPER);
    out.extend_from_slice(&header.dh_public);
    put_u32(&mut out, header.pn);
    put_u32(&mut out, header.n);
    put_u32(&mut out, bundle.registration_id);
    put_u32(&mut out, bundle.signed_pre_key_id);
    put_u32(&mut out, bundle.one_time_pre_key_id.unwrap_or(0));
    out.extend_from_slice(&bundle.base_key);
    out.extend_from_slice(&bundle.identity_key);
    out.extend_from_slice(ciphertext);
    out
}

/// Parse an E2EE payload into its structure.
pub fn deserialize(payload: &[u8]) -> Result<DecodedMessage> {
    anyhow::ensure!(
        payload.len() >= 42,
        "Gói tin E2EE quá ngắn ({} byte)",
        payload.len()
    );
    let version = payload[0];
    anyhow::ensure!(
        version == WIRE_VERSION,
        "Phiên bản gói tin E2EE không được hỗ trợ: {version}"
    );
    let msg_type = payload[1];
    let mut dh_public = [0u8; 32];
    dh_public.copy_from_slice(&payload[2..34]);
    let pn = read_u32(payload, 34)?;
    let n = read_u32(payload, 38)?;
    let header = MessageHeader {
        msg_type,
        dh_public,
        pn,
        n,
    };
    match msg_type {
        TYPE_WHISPER => Ok(DecodedMessage::Whisper(WhisperMessage {
            header,
            ciphertext: payload[42..].to_vec(),
        })),
        TYPE_PREKEY_WHISPER => {
            anyhow::ensure!(payload.len() >= 118, "Gói tin E2EE PreKey bị cắt cụt");
            let registration_id = read_u32(payload, 42)?;
            let signed_pre_key_id = read_u32(payload, 46)?;
            let otpk_raw = read_u32(payload, 50)?;
            let one_time_pre_key_id = if otpk_raw == 0 { None } else { Some(otpk_raw) };
            let mut base_key = [0u8; 32];
            base_key.copy_from_slice(&payload[54..86]);
            let mut identity_key = [0u8; 32];
            identity_key.copy_from_slice(&payload[86..118]);
            Ok(DecodedMessage::PreKey(PreKeyMessage {
                bundle: PreKeyBundleWire {
                    registration_id,
                    signed_pre_key_id,
                    one_time_pre_key_id,
                    base_key,
                    identity_key,
                },
                header,
                ciphertext: payload[118..].to_vec(),
            }))
        }
        other => anyhow::bail!("Loại tin nhắn E2EE không hỗ trợ: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> MessageHeader {
        MessageHeader {
            msg_type: TYPE_WHISPER,
            dh_public: [7u8; 32],
            pn: 2,
            n: 5,
        }
    }

    #[test]
    fn whisper_roundtrip() {
        let payload = serialize_whisper(&header(), &[1, 2, 3, 4]);
        match deserialize(&payload).unwrap() {
            DecodedMessage::Whisper(m) => {
                assert_eq!(m.header.dh_public, [7u8; 32]);
                assert_eq!(m.header.pn, 2);
                assert_eq!(m.header.n, 5);
                assert_eq!(m.ciphertext, vec![1, 2, 3, 4]);
            }
            _ => panic!("expected whisper"),
        }
    }

    #[test]
    fn prekey_roundtrip() {
        let bundle = PreKeyBundleWire {
            registration_id: 42,
            signed_pre_key_id: 1,
            one_time_pre_key_id: Some(777),
            base_key: [3u8; 32],
            identity_key: [4u8; 32],
        };
        let mut h = header();
        h.msg_type = TYPE_PREKEY_WHISPER;
        let payload = serialize_prekey(&bundle, &h, &[9, 9]);
        match deserialize(&payload).unwrap() {
            DecodedMessage::PreKey(m) => {
                assert_eq!(m.bundle.registration_id, 42);
                assert_eq!(m.bundle.signed_pre_key_id, 1);
                assert_eq!(m.bundle.one_time_pre_key_id, Some(777));
                assert_eq!(m.bundle.base_key, [3u8; 32]);
                assert_eq!(m.bundle.identity_key, [4u8; 32]);
                assert_eq!(m.ciphertext, vec![9, 9]);
            }
            _ => panic!("expected prekey"),
        }

        // Test with None one_time_pre_key_id
        let mut bundle_none = bundle;
        bundle_none.one_time_pre_key_id = None;
        let payload_none = serialize_prekey(&bundle_none, &h, &[9, 9]);
        match deserialize(&payload_none).unwrap() {
            DecodedMessage::PreKey(m) => {
                assert_eq!(m.bundle.one_time_pre_key_id, None);
            }
            _ => panic!("expected prekey"),
        }
    }

    #[test]
    fn rejects_unknown_version_and_type() {
        let mut payload = serialize_whisper(&header(), &[1]);
        payload[0] = 99;
        assert!(deserialize(&payload).is_err());

        let mut payload = serialize_whisper(&header(), &[1]);
        payload[1] = 77;
        assert!(deserialize(&payload).is_err());
    }

    #[test]
    fn rejects_truncated_payload() {
        assert!(deserialize(&[1, 2, 3]).is_err());
    }
}
