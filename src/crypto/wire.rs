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
use base64::{engine::general_purpose::STANDARD as B64, Engine};

use super::ratchet::MessageHeader;

/// Legacy flat envelope version.
pub const WIRE_VERSION: u8 = 1;

/// Standard Signal Protocol wire version byte: (3 << 4) | 3 = 0x33 (Signal v3).
pub const SIGNAL_PROTO_VERSION: u8 = 0x33;

/// Message type: regular ratchet message.
pub const TYPE_WHISPER: u8 = 1;
/// Message type: first message of a session (carries the X3DH base key).
pub const TYPE_PREKEY_WHISPER: u8 = 3;

/// Truncated MAC length per standard Signal Protocol wire format (8 bytes).
pub const TRUNCATED_MAC_LEN: usize = 8;
/// Legacy MAC length (32 bytes HMAC-SHA256).
pub const LEGACY_MAC_LEN: usize = 32;

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
    /// Ciphertext with the trailing MAC.
    pub ciphertext: Vec<u8>,
}

/// A decoded `WhisperMessage`.
#[derive(Debug, Clone)]
pub struct WhisperMessage {
    /// Ratchet message header.
    pub header: MessageHeader,
    /// Ciphertext with the trailing MAC.
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

// --- Minimal zero-dependency protobuf varint & length-delimited encoding/decoding ---

fn encode_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push(((value & 0x7f) as u8) | 0x80);
        value >>= 7;
    }
    out.push((value & 0x7f) as u8);
}

fn decode_varint(data: &[u8], offset: &mut usize) -> Result<u64> {
    let mut result: u64 = 0;
    let mut shift = 0;
    while *offset < data.len() {
        let b = data[*offset];
        *offset += 1;
        result |= ((b & 0x7f) as u64) << shift;
        if (b & 0x80) == 0 {
            return Ok(result);
        }
        shift += 7;
        anyhow::ensure!(shift <= 64, "Varint overflow");
    }
    anyhow::bail!("Cắt cụt khi đọc varint protobuf");
}

fn encode_bytes_field(out: &mut Vec<u8>, field_num: u32, data: &[u8]) {
    let tag = (field_num << 3) | 2; // wire type 2 = length-delimited
    encode_varint(out, tag as u64);
    encode_varint(out, data.len() as u64);
    out.extend_from_slice(data);
}

fn encode_uint32_field(out: &mut Vec<u8>, field_num: u32, value: u32) {
    if value == 0 {
        return; // default in proto3 or omit zero
    }
    let tag = field_num << 3; // wire type 0 = varint
    encode_varint(out, tag as u64);
    encode_varint(out, value as u64);
}

/// Serialise a regular `WhisperMessage` in standard Signal protobuf format.
/// Wire layout: [version: 0x33] || protobuf(SignalMessage) || 8-byte MAC.
pub fn serialize_whisper_signal(
    header: &MessageHeader,
    ciphertext: &[u8],
    mac_key: &[u8; 32],
) -> Vec<u8> {
    let mut proto_body = Vec::new();
    // Field 1: ratchet_key (32 bytes Curve25519 or 33 bytes DJB)
    encode_bytes_field(&mut proto_body, 1, &header.dh_public);
    // Field 2: counter (n)
    encode_uint32_field(&mut proto_body, 2, header.n);
    // Field 3: previous_counter (pn)
    encode_uint32_field(&mut proto_body, 3, header.pn);
    // Field 4: ciphertext
    encode_bytes_field(&mut proto_body, 4, ciphertext);

    let mut out = Vec::with_capacity(1 + proto_body.len() + TRUNCATED_MAC_LEN);
    out.push(SIGNAL_PROTO_VERSION);
    out.extend_from_slice(&proto_body);

    // Compute truncated 8-byte HMAC-SHA256 over message fields
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(mac_key).expect("HMAC accepts 32-byte key");
    mac.update(&header.dh_public);
    mac.update(&header.n.to_be_bytes());
    mac.update(&header.pn.to_be_bytes());
    mac.update(ciphertext);
    let full_mac = mac.finalize().into_bytes();
    out.extend_from_slice(&full_mac[..TRUNCATED_MAC_LEN]);
    out
}

/// Serialise a `PreKeyWhisperMessage` in standard Signal protobuf format.
/// Wire layout: [version: 0x33] || protobuf(PreKeySignalMessage)
/// where field 4 (`message`) contains the complete serialized WhisperMessage:
/// `[version: 0x33] || protobuf(SignalMessage) || [8-byte MAC]`.
///
/// Official MessageProtocol.proto PreKeySignalMessage tags:
/// - preKeyId = 1
/// - baseKey = 2
/// - identityKey = 3
/// - message = 4
/// - registrationId = 5
/// - signedPreKeyId = 6
pub fn serialize_prekey_signal(
    bundle: &PreKeyBundleWire,
    header: &MessageHeader,
    inner_ciphertext: &[u8],
    mac_key: &[u8; 32],
) -> Vec<u8> {
    // 1. Build nested WhisperMessage (SignalMessage with 0x33 version byte and 8-byte MAC)
    let nested_whisper = serialize_whisper_signal(header, inner_ciphertext, mac_key);

    // 2. Build PreKeySignalMessage with official MessageProtocol.proto tags
    let mut proto_body = Vec::new();
    if let Some(otpk_id) = bundle.one_time_pre_key_id {
        encode_uint32_field(&mut proto_body, 1, otpk_id); // field 1: preKeyId
    }
    encode_bytes_field(&mut proto_body, 2, &bundle.base_key); // field 2: baseKey
    encode_bytes_field(&mut proto_body, 3, &bundle.identity_key); // field 3: identityKey
    encode_bytes_field(&mut proto_body, 4, &nested_whisper); // field 4: message (SignalMessage)
    encode_uint32_field(&mut proto_body, 5, bundle.registration_id); // field 5: registrationId
    encode_uint32_field(&mut proto_body, 6, bundle.signed_pre_key_id); // field 6: signedPreKeyId

    let mut out = Vec::with_capacity(1 + proto_body.len());
    out.push(SIGNAL_PROTO_VERSION);
    out.extend_from_slice(&proto_body);
    out
}

/// Serialise a regular `WhisperMessage` (legacy flat envelope format for existing tests and backward compatibility).
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

/// Serialise a `PreKeyWhisperMessage` (legacy flat envelope format for existing tests and backward compatibility).
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

/// Encode binary E2EE message as Base64 string for Zalo JSON transport.
pub fn encode_wire_b64(msg_type: u8, wire_bytes: &[u8]) -> String {
    let mut payload = Vec::with_capacity(1 + wire_bytes.len());
    payload.push(msg_type);
    payload.extend_from_slice(wire_bytes);
    B64.encode(&payload)
}

/// Decode Base64 string from Zalo JSON transport into (msg_type, wire_bytes).
pub fn decode_wire_b64(b64: &str) -> Result<(u8, Vec<u8>)> {
    let bytes = B64
        .decode(b64.trim())
        .context("Giải mã Base64 tin nhắn E2EE thất bại")?;
    anyhow::ensure!(!bytes.is_empty(), "Gói tin E2EE Base64 rỗng");
    let msg_type = bytes[0];
    Ok((msg_type, bytes[1..].to_vec()))
}

/// Parse an E2EE payload into its structure.
///
/// Supports both:
/// 1. Standard Signal Protocol protobuf wire format (version byte `0x33`, 8-byte truncated MAC).
/// 2. Legacy 118-byte flat container (version byte `1`, 32-byte MAC).
pub fn deserialize(payload: &[u8]) -> Result<DecodedMessage> {
    anyhow::ensure!(!payload.is_empty(), "Gói tin E2EE rỗng");

    let version = payload[0];
    if version == SIGNAL_PROTO_VERSION {
        return deserialize_signal_proto(payload);
    }

    if version == WIRE_VERSION {
        return deserialize_legacy(payload);
    }

    anyhow::bail!("Phiên bản gói tin E2EE không được hỗ trợ: {version:#x}");
}

fn deserialize_legacy(payload: &[u8]) -> Result<DecodedMessage> {
    anyhow::ensure!(
        payload.len() >= 42,
        "Gói tin E2EE quá ngắn ({} byte)",
        payload.len()
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

fn deserialize_signal_proto(payload: &[u8]) -> Result<DecodedMessage> {
    anyhow::ensure!(
        payload.len() > 1,
        "Gói tin Signal protobuf quá ngắn ({} bytes)",
        payload.len()
    );

    // First, inspect if this is a PreKeySignalMessage or regular SignalMessage.
    // In PreKeySignalMessage, payload starts with 0x33, then fields:
    // 1 (preKeyId, varint) or 2 (baseKey, bytes, tag 0x12) or 3 (identityKey, bytes, tag 0x1a)
    // In SignalMessage, field 1 is ratchet_key (tag 0x0a)
    let body = &payload[1..];

    // Decode top-level protobuf fields
    let mut reg_id = 0u32;
    let mut prekey_id = None;
    let mut signed_prekey_id = 0u32;
    let mut base_key = None;
    let mut identity_key = None;
    let mut nested_msg_bytes = None;

    // Whisper message fields (if top-level is SignalMessage)
    let mut whisper_dh_public = None;
    let mut whisper_counter = 0u32;
    let mut whisper_prev_counter = 0u32;
    let mut whisper_ciphertext = None;

    // Try parsing as PreKeySignalMessage (over the full body)
    let mut is_prekey = false;
    let mut test_offset = 0;
    while test_offset < body.len() {
        if let Ok(tag) = decode_varint(body, &mut test_offset) {
            let field_num = (tag >> 3) as u32;
            let wire_type = (tag & 0x7) as u8;
            if (field_num == 2 || field_num == 3 || field_num == 4) && wire_type == 2 {
                if let Ok(len) = decode_varint(body, &mut test_offset) {
                    if test_offset + (len as usize) <= body.len() {
                        test_offset += len as usize;
                        if field_num == 4 {
                            is_prekey = true;
                        }
                        continue;
                    }
                }
            } else if (field_num == 1 || field_num == 5 || field_num == 6)
                && wire_type == 0
                && decode_varint(body, &mut test_offset).is_ok()
            {
                continue;
            }
        }
        break;
    }

    if is_prekey {
        // PreKeySignalMessage: entire body is protobuf (NO outer MAC)
        let mut offset = 0;
        while offset < body.len() {
            let tag = decode_varint(body, &mut offset)?;
            let field_num = (tag >> 3) as u32;
            let wire_type = (tag & 0x7) as u8;
            match wire_type {
                0 => {
                    let val = decode_varint(body, &mut offset)?;
                    match field_num {
                        1 => prekey_id = Some(val as u32),
                        5 => reg_id = val as u32,
                        6 => signed_prekey_id = val as u32,
                        _ => {}
                    }
                }
                2 => {
                    let len = decode_varint(body, &mut offset)? as usize;
                    anyhow::ensure!(offset + len <= body.len(), "Trường protobuf bị cắt cụt");
                    let field_slice = &body[offset..offset + len];
                    offset += len;
                    match field_num {
                        2 => base_key = Some(field_slice),
                        3 => identity_key = Some(field_slice),
                        4 => nested_msg_bytes = Some(field_slice),
                        _ => {}
                    }
                }
                _ => anyhow::bail!(
                    "Không hỗ trợ wire type protobuf trong PreKeySignalMessage: {wire_type}"
                ),
            }
        }

        let base_slice = base_key.context("Thiếu baseKey trong PreKeySignalMessage")?;
        let id_slice = identity_key.context("Thiếu identityKey trong PreKeySignalMessage")?;
        let base_32 = crate::crypto::keystore::from_djb_bytes(base_slice)?;
        let id_32 = crate::crypto::keystore::from_djb_bytes(id_slice)?;
        let nested_bytes = nested_msg_bytes.context("Thiếu message trong PreKeySignalMessage")?;

        // The nested message is a complete WhisperMessage: [0x33] || protobuf(SignalMessage) || 8-byte MAC
        let inner_decoded = deserialize_signal_proto(nested_bytes)?;
        let inner_whisper = match inner_decoded {
            DecodedMessage::Whisper(w) => w,
            DecodedMessage::PreKey(_) => anyhow::bail!("Nested message không thể là PreKey"),
        };

        return Ok(DecodedMessage::PreKey(PreKeyMessage {
            bundle: PreKeyBundleWire {
                registration_id: reg_id,
                signed_pre_key_id: signed_prekey_id,
                one_time_pre_key_id: prekey_id,
                base_key: base_32,
                identity_key: id_32,
            },
            header: inner_whisper.header,
            ciphertext: inner_whisper.ciphertext,
        }));
    }

    // Otherwise, it's a regular WhisperMessage: [version: 0x33] || protobuf(SignalMessage) || [mac: 8 bytes]
    anyhow::ensure!(
        payload.len() > 1 + TRUNCATED_MAC_LEN,
        "Gói tin Whisper Signal protobuf quá ngắn ({} bytes)",
        payload.len()
    );
    let body_end = payload.len() - TRUNCATED_MAC_LEN;
    let proto_bytes = &payload[1..body_end];
    let mac = &payload[body_end..];

    let mut offset = 0;
    while offset < proto_bytes.len() {
        let tag = decode_varint(proto_bytes, &mut offset)?;
        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 0x7) as u8;
        match wire_type {
            0 => {
                let val = decode_varint(proto_bytes, &mut offset)?;
                match field_num {
                    2 => whisper_counter = val as u32,
                    3 => whisper_prev_counter = val as u32,
                    _ => {}
                }
            }
            2 => {
                let len = decode_varint(proto_bytes, &mut offset)? as usize;
                anyhow::ensure!(
                    offset + len <= proto_bytes.len(),
                    "Trường protobuf bị cắt cụt"
                );
                let field_slice = &proto_bytes[offset..offset + len];
                offset += len;
                match field_num {
                    1 => whisper_dh_public = Some(field_slice),
                    4 => whisper_ciphertext = Some(field_slice),
                    _ => {}
                }
            }
            _ => anyhow::bail!("Không hỗ trợ wire type protobuf: {wire_type}"),
        }
    }

    let dh_slice = whisper_dh_public.context("Thiếu dh_public trong SignalMessage")?;
    let dh_32 = crate::crypto::keystore::from_djb_bytes(dh_slice)?;
    let ct_slice = whisper_ciphertext.context("Thiếu ciphertext trong SignalMessage")?;

    let mut full_ct = ct_slice.to_vec();
    full_ct.extend_from_slice(mac);

    Ok(DecodedMessage::Whisper(WhisperMessage {
        header: MessageHeader {
            msg_type: TYPE_WHISPER,
            dh_public: dh_32,
            pn: whisper_prev_counter,
            n: whisper_counter,
        },
        ciphertext: full_ct,
    }))
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

    #[test]
    fn signal_proto_whisper_roundtrip_with_truncated_mac() {
        let h = header();
        let ct = b"signal encrypted payload with 8 byte mac";
        let mac_key = [42u8; 32];
        let wire = serialize_whisper_signal(&h, ct, &mac_key);

        assert_eq!(wire[0], SIGNAL_PROTO_VERSION);
        let decoded = deserialize(&wire).expect("deserialize signal proto whisper");
        match decoded {
            DecodedMessage::Whisper(w) => {
                assert_eq!(w.header.dh_public, h.dh_public);
                assert_eq!(w.header.n, h.n);
                assert_eq!(w.header.pn, h.pn);
                assert_eq!(w.ciphertext.len(), ct.len() + TRUNCATED_MAC_LEN);
                assert_eq!(&w.ciphertext[..ct.len()], ct);
            }
            _ => panic!("Expected Whisper message"),
        }
    }

    #[test]
    fn signal_proto_prekey_roundtrip_with_truncated_mac() {
        let bundle = PreKeyBundleWire {
            registration_id: 1337,
            signed_pre_key_id: 7,
            one_time_pre_key_id: Some(99),
            base_key: [11u8; 32],
            identity_key: [22u8; 32],
        };
        let mut h = header();
        h.msg_type = TYPE_PREKEY_WHISPER;
        let ct = b"prekey initial ciphertext";
        let mac_key = [55u8; 32];
        let wire = serialize_prekey_signal(&bundle, &h, ct, &mac_key);

        assert_eq!(wire[0], SIGNAL_PROTO_VERSION);
        let decoded = deserialize(&wire).expect("deserialize signal proto prekey");
        match decoded {
            DecodedMessage::PreKey(pk) => {
                assert_eq!(pk.bundle.registration_id, 1337);
                assert_eq!(pk.bundle.signed_pre_key_id, 7);
                assert_eq!(pk.bundle.one_time_pre_key_id, Some(99));
                assert_eq!(pk.bundle.base_key, [11u8; 32]);
                assert_eq!(pk.bundle.identity_key, [22u8; 32]);
                assert_eq!(pk.header.dh_public, h.dh_public);
                assert_eq!(pk.header.n, h.n);
                assert_eq!(pk.ciphertext.len(), ct.len() + TRUNCATED_MAC_LEN);
                assert_eq!(&pk.ciphertext[..ct.len()], ct);
            }
            _ => panic!("Expected PreKey message"),
        }
    }

    #[test]
    fn base64_wire_wrapping_roundtrip() {
        let h = header();
        let ct = b"hello";
        let wire = serialize_whisper(&h, ct);
        let b64 = encode_wire_b64(TYPE_WHISPER, &wire);
        let (msg_type, decoded_bytes) = decode_wire_b64(&b64).expect("decode wire b64");
        assert_eq!(msg_type, TYPE_WHISPER);
        assert_eq!(decoded_bytes, wire);
    }
}
