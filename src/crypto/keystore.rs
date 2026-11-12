//! Identity key, signed prekey and one-time prekey lifecycle (WP16-1).
//!
//! Evidence: `re-notes/36-zalo-e2ee-signal-protocol-analysis.md` §4
//! - Identity key: Curve25519, one per profile, long-lived
//! - Signed prekey: Curve25519, signed with Ed25519 identity key, rotated ~7 days
//! - One-time prekeys: Curve25519, consumed per session, batch of 5

use anyhow::Result;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use x25519_dalek::PublicKey;

use super::ratchet::KeyPair;

/// `PREKEY_BATCH` — matches Zalo zalo-e2ee.js constant (evidence E2).
pub const PREKEY_BATCH: usize = 5;
/// `DEVICE_ID` — Zalo PC always uses device_id = 1 (evidence E2).
pub const DEVICE_ID: u32 = 1;

/// Identity key pair.  Private key is stored DPAPI-encrypted at rest.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct IdentityKeyPair {
    /// Curve25519 key pair for DH operations.
    pub dh: super::ratchet::KeyPairBytes,
    /// Ed25519 signing key for signing our own prekeys.
    pub signing_private: [u8; 32],
    pub signing_public: [u8; 32],
    /// Random registration ID (u32).  Must match Zalo server registration.
    pub registration_id: u32,
}

impl IdentityKeyPair {
    /// Generate a fresh identity (new device registration).
    pub fn generate() -> Result<Self> {
        let dh = KeyPair::generate();
        let signing_private_bytes = random_32()?;
        let sk = SigningKey::from_bytes(&signing_private_bytes);
        let vk: VerifyingKey = (&sk).into();
        let reg_id = random_u32()? & 0x3FFF; // 14-bit, matching Signal spec
        Ok(Self {
            dh: (&dh).into(),
            signing_private: signing_private_bytes,
            signing_public: vk.to_bytes(),
            registration_id: reg_id,
        })
    }

    /// Sign arbitrary bytes with the Ed25519 signing key.
    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        let sk = SigningKey::from_bytes(&self.signing_private);
        sk.sign(data).to_bytes().to_vec()
    }
}

/// Signal Protocol Curve25519 DJB key type byte (0x05).
pub const DJB_TYPE: u8 = 0x05;

/// Convert a 32-byte Curve25519 public key to 33-byte DJB format (prefixed with 0x05).
pub fn to_djb_bytes(key: &[u8; 32]) -> [u8; 33] {
    let mut out = [0u8; 33];
    out[0] = DJB_TYPE;
    out[1..].copy_from_slice(key);
    out
}

/// Parse a 32-byte Curve25519 public key from either 33-byte DJB format (0x05 prefix) or raw 32-byte format.
pub fn from_djb_bytes(slice: &[u8]) -> Result<[u8; 32]> {
    if slice.len() == 33 && slice[0] == DJB_TYPE {
        let mut out = [0u8; 32];
        out.copy_from_slice(&slice[1..33]);
        Ok(out)
    } else if slice.len() == 32 {
        let mut out = [0u8; 32];
        out.copy_from_slice(slice);
        Ok(out)
    } else {
        anyhow::bail!(
            "Độ dài khóa Curve25519 không hợp lệ: {} byte (kỳ vọng 32 bytes hoặc 33 bytes có tiền tố 0x05)",
            slice.len()
        );
    }
}

/// Mask off the high bit for Curve25519 Montgomery public keys (RFC 7748 / XEdDSA).
pub fn clamp_curve25519_scalar(k: &[u8; 32]) -> [u8; 32] {
    let mut s = *k;
    s[0] &= 248;
    s[31] &= 127;
    s[31] |= 64;
    s
}

/// Sign a message using the Curve25519 identity private key with XEdDSA (XEd25519).
///
/// Implements XEd25519 signing per Signal Protocol specification (https://signal.org/docs/specifications/xeddsa/).
/// `k`: 32-byte Curve25519 private key.
/// `msg`: message bytes to sign (typically the 33-byte DJB-encoded signedPreKey public key).
pub fn xeddsa_sign(k_bytes: &[u8; 32], msg: &[u8]) -> [u8; 64] {
    use curve25519_dalek::constants::ED25519_BASEPOINT_POINT;
    use curve25519_dalek::scalar::Scalar;
    use sha2::{Digest, Sha512};

    // 1. Clamping the Montgomery private key k
    let k_clamped = clamp_curve25519_scalar(k_bytes);
    let k_scalar = Scalar::from_bytes_mod_order(k_clamped);

    // 2. Compute E = k * B on Edwards curve
    let e_point = ED25519_BASEPOINT_POINT * k_scalar;
    let enc_e = e_point.compress();

    // In XEdDSA: If sign bit of E is 1, a = -k mod q, else a = k mod q
    let sign_bit = (enc_e.as_bytes()[31] >> 7) & 1;
    let a_scalar = if sign_bit == 1 { -k_scalar } else { k_scalar };

    // Compute A = a * B (which has sign bit 0)
    let a_point = ED25519_BASEPOINT_POINT * a_scalar;
    let enc_a = a_point.compress();

    // 3. Generate nonce r = hash1(a || M || Z) mod q
    // Z: 64 random bytes for side-channel / glitch resistance
    let mut z = [0u8; 64];
    use aws_lc_rs::rand::{SecureRandom, SystemRandom};
    let _ = SystemRandom::new().fill(&mut z);

    let mut h1 = Sha512::new();
    // hash1 prefix: 32 bytes of 0xFF with first byte 0xFE
    let mut prefix = [0xFFu8; 32];
    prefix[0] = 0xFE;
    h1.update(prefix);
    h1.update(a_scalar.to_bytes());
    h1.update(msg);
    h1.update(z);
    let r_hash: [u8; 64] = h1.finalize().into();
    let r_scalar = Scalar::from_bytes_mod_order_wide(&r_hash);

    // 4. R = r * B
    let r_point = ED25519_BASEPOINT_POINT * r_scalar;
    let enc_r = r_point.compress();

    // 5. h = hash(R || A || M) mod q
    let mut h_standard = Sha512::new();
    h_standard.update(enc_r.as_bytes());
    h_standard.update(enc_a.as_bytes());
    h_standard.update(msg);
    let h_hash: [u8; 64] = h_standard.finalize().into();
    let h_scalar = Scalar::from_bytes_mod_order_wide(&h_hash);

    // 6. s = r + h * a mod q
    let s_scalar = r_scalar + h_scalar * a_scalar;

    let mut sig = [0u8; 64];
    sig[..32].copy_from_slice(enc_r.as_bytes());
    sig[32..].copy_from_slice(&s_scalar.to_bytes());
    sig
}

/// Verify an XEdDSA (XEd25519) signature over Curve25519 public key u.
///
/// Implements XEd25519 verification per Signal Protocol specification.
/// `u_bytes`: 32-byte Curve25519 public key (or 33-byte DJB public key).
/// `msg`: message bytes to verify.
/// `sig`: 64-byte signature (R || s).
pub fn xeddsa_verify(u_bytes: &[u8], msg: &[u8], sig: &[u8; 64]) -> bool {
    use curve25519_dalek::constants::ED25519_BASEPOINT_POINT;
    use curve25519_dalek::edwards::CompressedEdwardsY;
    use curve25519_dalek::montgomery::MontgomeryPoint;
    use curve25519_dalek::scalar::Scalar;
    use sha2::{Digest, Sha512};

    let raw_u = match from_djb_bytes(u_bytes) {
        Ok(raw) => raw,
        Err(_) => return false,
    };

    // Parse R point and s scalar
    let mut enc_r = [0u8; 32];
    enc_r.copy_from_slice(&sig[..32]);
    let mut s_bytes = [0u8; 32];
    s_bytes.copy_from_slice(&sig[32..]);

    let Some(r_point) = CompressedEdwardsY(enc_r).decompress() else {
        return false;
    };
    let Some(s_scalar) = Option::<Scalar>::from(Scalar::from_canonical_bytes(s_bytes)) else {
        return false;
    };

    // Convert Montgomery u to Edwards A with sign bit 0 (convert_mont)
    let mont = MontgomeryPoint(raw_u);
    let Some(a_point) = mont.to_edwards(0) else {
        return false;
    };
    let enc_a = a_point.compress();

    // Compute h = hash(R || A || M) mod q
    let mut h_standard = Sha512::new();
    h_standard.update(enc_r);
    h_standard.update(enc_a.as_bytes());
    h_standard.update(msg);
    let h_hash: [u8; 64] = h_standard.finalize().into();
    let h_scalar = Scalar::from_bytes_mod_order_wide(&h_hash);

    // Check s * B == R + h * A
    let s_b = ED25519_BASEPOINT_POINT * s_scalar;
    let r_plus_h_a = r_point + (a_point * h_scalar);

    s_b == r_plus_h_a
}

/// Signed prekey: rotated every ~7 days.  Signature = XEdDSA(identity_dh_key, spk_pub).
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct SignedPreKey {
    pub key_id: u32,
    pub key_pair: super::ratchet::KeyPairBytes,
    /// XEdDSA signature over the Curve25519 public key bytes (64 raw bytes).
    pub signature: Vec<u8>,
    /// Unix timestamp (ms) when this prekey was generated.
    pub created_at: i64,
}

impl SignedPreKey {
    pub fn generate(key_id: u32, identity: &IdentityKeyPair) -> Result<Self> {
        let kp = KeyPair::generate();
        let djb_pub = to_djb_bytes(kp.public.as_bytes());
        let signature = xeddsa_sign(&identity.dh.private, &djb_pub).to_vec();
        let created_at = now_ms();
        Ok(Self {
            key_id,
            key_pair: (&kp).into(),
            signature,
            created_at,
        })
    }

    /// Generate a signed prekey signing over 33-byte DJB format (standard Signal wire parity).
    pub fn generate_djb(key_id: u32, identity: &IdentityKeyPair) -> Result<Self> {
        Self::generate(key_id, identity)
    }
}

/// One-time prekey: consumed per X3DH session setup (perfect forward secrecy).
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct OneTimePreKey {
    pub key_id: u32,
    pub key_pair: super::ratchet::KeyPairBytes,
}

impl OneTimePreKey {
    pub fn generate(key_id: u32) -> Result<Self> {
        let kp = KeyPair::generate();
        Ok(Self {
            key_id,
            key_pair: (&kp).into(),
        })
    }

    /// Generate a batch of `n` prekeys starting from `start_id`.
    pub fn generate_batch(start_id: u32, n: usize) -> Result<Vec<Self>> {
        if n > 0 {
            let offset =
                u32::try_from(n - 1).map_err(|_| anyhow::anyhow!("Số prekey vượt giới hạn"))?;
            anyhow::ensure!(
                start_id.checked_add(offset).is_some(),
                "Đã hết định danh prekey"
            );
        }
        (0..n)
            .map(|i| Self::generate(start_id + i as u32))
            .collect()
    }
}

/// PreKey bundle: sent to the server for peers to initiate X3DH sessions with us.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PreKeyBundle {
    pub identity_key: [u8; 32],
    /// Ed25519 signing public key bound to this bundle (audit A04).
    /// Used by peers to verify the signed_pre_key signature.
    pub signing_identity_key: [u8; 32],
    pub registration_id: u32,
    pub signed_pre_key: BundleSignedPreKey,
    pub pre_keys: Vec<BundleOneTimePreKey>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct BundleSignedPreKey {
    pub key_id: u32,
    pub public_key: [u8; 32],
    pub signature: Vec<u8>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct BundleOneTimePreKey {
    pub key_id: u32,
    pub public_key: [u8; 32],
}

impl PreKeyBundle {
    pub fn from_local_keys(
        identity: &IdentityKeyPair,
        spk: &SignedPreKey,
        one_time: &[OneTimePreKey],
    ) -> Self {
        Self {
            identity_key: identity.dh.public,
            signing_identity_key: identity.dh.public,
            registration_id: identity.registration_id,
            signed_pre_key: BundleSignedPreKey {
                key_id: spk.key_id,
                public_key: spk.key_pair.public,
                signature: spk.signature.clone(),
            },
            pre_keys: one_time
                .iter()
                .map(|k| BundleOneTimePreKey {
                    key_id: k.key_id,
                    public_key: k.key_pair.public,
                })
                .collect(),
        }
    }

    /// Convert to JSON Value for export/upload with bound signing identity (audit A04).
    pub fn to_export_json(&self) -> serde_json::Value {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        serde_json::json!({
            "identityKey": B64.encode(self.identity_key),
            "signingIdentityKey": B64.encode(self.signing_identity_key),
            "registrationId": self.registration_id,
            "signedPreKey": {
                "keyId": self.signed_pre_key.key_id,
                "publicKey": B64.encode(self.signed_pre_key.public_key),
                "signature": B64.encode(&self.signed_pre_key.signature),
            },
            "preKeys": self.pre_keys.iter().map(|k| serde_json::json!({
                "keyId": k.key_id,
                "publicKey": B64.encode(k.public_key),
            })).collect::<Vec<_>>(),
        })
    }

    /// Convert self into RemoteBundle for local verification (audit A04).
    pub fn to_remote_bundle(&self) -> Result<super::prekey::RemoteBundle> {
        let sig: [u8; 64] = self
            .signed_pre_key
            .signature
            .clone()
            .try_into()
            .map_err(|_| anyhow::anyhow!("Chữ ký signedPreKey không đủ 64 bytes"))?;
        Ok(super::prekey::RemoteBundle {
            identity_key: self.identity_key,
            signing_identity_key: Some(self.signing_identity_key),
            registration_id: self.registration_id,
            signed_pre_key_id: self.signed_pre_key.key_id,
            signed_pre_key: self.signed_pre_key.public_key,
            signature: sig,
            one_time_pre_key: self.pre_keys.first().map(|k| (k.public_key, k.key_id)),
        })
    }
}

// --- X3DH key agreement ---

/// Alice (initiator) computes the X3DH shared secret from Bob's prekey bundle.
///
/// Returns `(master_secret_32_bytes, ephemeral_public_key)`.
/// Evidence: `re-notes/36-zalo-e2ee-signal-protocol-analysis.md` §3.1
pub fn x3dh_sender(
    our_identity: &IdentityKeyPair,
    their_identity_pub: &[u8; 32],
    their_spk_pub: &[u8; 32],
    their_otpk_pub: Option<&[u8; 32]>,
) -> Result<([u8; 32], [u8; 32])> {
    let ik_a = KeyPair::from_private_bytes(our_identity.dh.private);
    let ik_b = PublicKey::from(*their_identity_pub);
    let spk_b = PublicKey::from(*their_spk_pub);
    let ek_a = KeyPair::generate();

    // X3DH: DH1 || DH2 || DH3 [|| DH4]
    let dh1 = ik_a.dh(&spk_b)?;
    let dh2 = ek_a.dh(&ik_b)?;
    let dh3 = ek_a.dh(&spk_b)?;
    let mut km: Vec<u8> = Vec::with_capacity(128 + 32);
    // F = 0xFF * 32 (curve25519 "F" prefix per Signal spec §2.2)
    km.extend_from_slice(&[0xFF; 32]);
    km.extend_from_slice(dh1.as_bytes());
    km.extend_from_slice(dh2.as_bytes());
    km.extend_from_slice(dh3.as_bytes());
    if let Some(otpk_b_bytes) = their_otpk_pub {
        let otpk_b = PublicKey::from(*otpk_b_bytes);
        let dh4 = ek_a.dh(&otpk_b)?;
        km.extend_from_slice(dh4.as_bytes());
    }

    use hkdf::Hkdf;
    use sha2::Sha256;
    let hk = Hkdf::<Sha256>::new(Some(&[0u8; 32]), &km);
    let mut master = [0u8; 32];
    hk.expand(b"WhisperText", &mut master)
        .map_err(|_| anyhow::anyhow!("X3DH HKDF expand failed"))?;

    Ok((master, *ek_a.public.as_bytes()))
}

/// Bob (receiver) computes the X3DH shared secret from Alice's PreKeyWhisperMessage.
pub fn x3dh_receiver(
    our_identity: &IdentityKeyPair,
    our_spk: &SignedPreKey,
    our_otpk: Option<&OneTimePreKey>,
    their_identity_pub: &[u8; 32],
    their_ek_pub: &[u8; 32], // Alice's ephemeral key from message header
) -> Result<[u8; 32]> {
    let ik_b = KeyPair::from_private_bytes(our_identity.dh.private);
    let spk_b = KeyPair::from_private_bytes(our_spk.key_pair.private);
    let ik_a = PublicKey::from(*their_identity_pub);
    let ek_a = PublicKey::from(*their_ek_pub);

    let dh1 = spk_b.dh(&ik_a)?;
    let dh2 = ik_b.dh(&ek_a)?;
    let dh3 = spk_b.dh(&ek_a)?;
    let mut km: Vec<u8> = Vec::with_capacity(128 + 32);
    km.extend_from_slice(&[0xFF; 32]);
    km.extend_from_slice(dh1.as_bytes());
    km.extend_from_slice(dh2.as_bytes());
    km.extend_from_slice(dh3.as_bytes());
    if let Some(otpk) = our_otpk {
        let otpk_b = KeyPair::from_private_bytes(otpk.key_pair.private);
        let dh4 = otpk_b.dh(&ek_a)?;
        km.extend_from_slice(dh4.as_bytes());
    }

    use hkdf::Hkdf;
    use sha2::Sha256;
    let hk = Hkdf::<Sha256>::new(Some(&[0u8; 32]), &km);
    let mut master = [0u8; 32];
    hk.expand(b"WhisperText", &mut master)
        .map_err(|_| anyhow::anyhow!("X3DH HKDF expand failed (receiver)"))?;

    Ok(master)
}

fn random_32() -> Result<[u8; 32]> {
    use aws_lc_rs::rand::{SecureRandom, SystemRandom};
    let rng = SystemRandom::new();
    let mut buf = [0u8; 32];
    rng.fill(&mut buf)
        .map_err(|_| anyhow::anyhow!("CSPRNG fill failed"))?;
    Ok(buf)
}

fn random_u32() -> Result<u32> {
    let b = random_32()?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_key_generates_and_signs() {
        let identity = IdentityKeyPair::generate().unwrap();
        assert_eq!(identity.dh.public.len(), 32);
        assert!(identity.registration_id < 0x4000);
        let sig = identity.sign(b"test payload");
        assert_eq!(sig.len(), 64);
    }

    #[test]
    fn signed_prekey_signature_verifies() {
        let identity = IdentityKeyPair::generate().unwrap();
        let spk = SignedPreKey::generate(1, &identity).unwrap();
        let sig_bytes: [u8; 64] = spk
            .signature
            .clone()
            .try_into()
            .expect("Signature is 64 bytes");
        let djb_bytes = to_djb_bytes(&spk.key_pair.public);
        assert!(xeddsa_verify(&identity.dh.public, &djb_bytes, &sig_bytes));
    }

    #[test]
    fn x3dh_sender_receiver_agree() {
        let alice_id = IdentityKeyPair::generate().unwrap();
        let bob_id = IdentityKeyPair::generate().unwrap();
        let bob_spk = SignedPreKey::generate(1, &bob_id).unwrap();
        let bob_otpk = OneTimePreKey::generate(100).unwrap();

        let (alice_secret, alice_ek_pub) = x3dh_sender(
            &alice_id,
            &bob_id.dh.public,
            &bob_spk.key_pair.public,
            Some(&bob_otpk.key_pair.public),
        )
        .unwrap();

        let bob_secret = x3dh_receiver(
            &bob_id,
            &bob_spk,
            Some(&bob_otpk),
            &alice_id.dh.public,
            &alice_ek_pub,
        )
        .unwrap();

        assert_eq!(alice_secret, bob_secret);
    }

    /// Audit A04: PreKeyBundle must bind signing identity key upon generation and export,
    /// and verifier must succeed using the bound signing key.
    #[test]
    fn test_prekey_bundle_binds_signing_identity_and_verifies_a04() {
        let identity = IdentityKeyPair::generate().unwrap();
        let spk = SignedPreKey::generate(1, &identity).unwrap();
        let otpk = OneTimePreKey::generate_batch(100, 3).unwrap();

        let bundle = PreKeyBundle::from_local_keys(&identity, &spk, &otpk);
        // Ensure signing_identity_key is bound to the bundle
        assert_eq!(bundle.signing_identity_key, identity.dh.public);
        assert_eq!(bundle.identity_key, identity.dh.public);

        // Convert to export json and verify signingIdentityKey presence
        let json_val = bundle.to_export_json();
        let b64_signing = json_val
            .get("signingIdentityKey")
            .and_then(|v| v.as_str())
            .unwrap();
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        assert_eq!(B64.decode(b64_signing).unwrap(), identity.dh.public);

        // Parse through production bundle parser and verify signature
        let parsed =
            crate::crypto::prekey::parse_bundle(&json_val).expect("parse production bundle");
        assert_eq!(parsed.signing_identity_key, Some(identity.dh.public));
        assert!(crate::crypto::session::verify_signed_prekey(&parsed));

        // Tamper with signingIdentityKey: verify must fail
        let mut tampered_json = json_val.clone();
        tampered_json["signingIdentityKey"] = serde_json::json!(B64.encode([99u8; 32]));
        let tampered_parsed = crate::crypto::prekey::parse_bundle(&tampered_json).unwrap();
        assert!(!crate::crypto::session::verify_signed_prekey(
            &tampered_parsed
        ));
    }

    #[test]
    fn test_signed_prekey_djb_verifies() {
        let identity = IdentityKeyPair::generate().unwrap();
        let spk_djb = SignedPreKey::generate_djb(1, &identity).unwrap();
        let djb_bytes = to_djb_bytes(&spk_djb.key_pair.public);
        let sig_bytes: [u8; 64] = spk_djb
            .signature
            .clone()
            .try_into()
            .expect("Signature is 64 bytes");
        assert!(xeddsa_verify(&identity.dh.public, &djb_bytes, &sig_bytes));
    }
}
