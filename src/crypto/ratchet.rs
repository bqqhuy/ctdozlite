//! Signal Protocol Double Ratchet state machine (WP16-2).
//!
//! Implements X3DH initial key agreement and the Double Ratchet algorithm
//! using the same primitives as Zalo PC's vendored `libsignal-protocol.js`
//! (upstream: `signalapp/libsignal-protocol-javascript`, GPL-3.0).
//!
//! **Evidence (E2 — static bundle analysis):**
//! - `re-notes/36-zalo-e2ee-signal-protocol-analysis.md` §3.1
//! - Confirmed: X3DH 4-DH, Double Ratchet with HKDF-SHA256, AES-256-CBC + HMAC-SHA256
//! - Each message type: 1 (WhisperMessage) or 3 (PreKeyWhisperMessage)
//! - DEVICE_ID = 1 (Zalo PC constant)
//!
//! Implementation is written from the standard Signal Protocol specification.
//! No code from libsignal-protocol-javascript is reproduced.
//!
//! # Ratchet invariant
//!
//! Both peers keep a *current ratchet key pair*.  The chain key used to
//! decrypt a message sent with ratchet key `K_peer` is always
//! `KDF_CK` derived from `KDF_RK(root, DH(my_current_private, K_peer))`, and the
//! sender derives the very same value as `KDF_RK(root, DH(K_peer_private, my_current_public))`.
//! A *ratchet step* performs two such derivations in a fixed order — first the
//! receiving chain, then the sending chain of a freshly generated key — so both
//! peers advance the root key in lockstep.

use anyhow::{Context, Result};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use x25519_dalek::{PublicKey, SharedSecret, StaticSecret};
use zeroize::Zeroize;

type HmacSha256 = Hmac<Sha256>;

// --- Key derivation constants (Signal spec §3.3) ---

/// Info label for the root-key / chain-key derivation.
const KDF_RK_INFO: &[u8] = b"WhisperRatchet";
/// Info label for the message-key derivation.
const KDF_KEYS_INFO: &[u8] = b"WhisperMessageKeysV2";

/// Maximum number of skipped message keys retained per ratchet step.
const MAX_SKIP: usize = 1000;

/// A Curve25519 key pair (static, long-lived or ephemeral ratchet step).
#[derive(Clone)]
pub struct KeyPair {
    pub private: StaticSecret,
    pub public: PublicKey,
}

impl KeyPair {
    /// Generate a new random key pair using the OS CSPRNG.
    pub fn generate() -> Self {
        // Random 32 bytes from the OS CSPRNG; curve25519 clamps the scalar during
        // the DH operation, so no additional clamping is required here.
        let mut bytes = [0u8; 32];
        use aws_lc_rs::rand::{SecureRandom, SystemRandom};
        SystemRandom::new()
            .fill(&mut bytes)
            .expect("OS CSPRNG fill must succeed");
        let private = StaticSecret::from(bytes);
        let public = PublicKey::from(&private);
        Self { private, public }
    }

    /// Deserialize private key bytes (32 bytes, little-endian Curve25519 scalar).
    pub fn from_private_bytes(bytes: [u8; 32]) -> Self {
        let private = StaticSecret::from(bytes);
        let public = PublicKey::from(&private);
        Self { private, public }
    }

    pub fn dh(&self, their_public: &PublicKey) -> Result<SharedSecret> {
        let shared = self.private.diffie_hellman(their_public);
        anyhow::ensure!(shared.was_contributory(), "Non-contributory DH secret rejected");
        Ok(shared)
    }
}

impl std::fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyPair")
            .field("public", &hex::encode(self.public.as_bytes()))
            .finish_non_exhaustive()
    }
}

// NOTE: random key material comes from the OS CSPRNG via `KeyPair::generate`.

// --- KDF functions (Signal spec §3.3) ---

/// KDF_RK(rk, dh_out) → (rk', ck)
/// Derives a new root key and chain key from a DH output.
pub fn kdf_rk(root_key: &[u8; 32], dh_output: &[u8]) -> ([u8; 32], [u8; 32]) {
    let hk = Hkdf::<Sha256>::new(Some(root_key), dh_output);
    let mut okm = [0u8; 64];
    hk.expand(KDF_RK_INFO, &mut okm)
        .expect("HKDF-SHA256 expand with 64 bytes output must not fail");
    let mut new_rk = [0u8; 32];
    let mut new_ck = [0u8; 32];
    new_rk.copy_from_slice(&okm[..32]);
    new_ck.copy_from_slice(&okm[32..]);
    okm.zeroize();
    (new_rk, new_ck)
}

/// KDF_CK(ck) → (ck', mk)
/// Advances the chain key and derives a message key.
pub fn kdf_ck(chain_key: &[u8; 32]) -> ([u8; 32], MessageKey) {
    // Signal spec: HMAC-SHA256 with input byte 0x01 for message key, 0x02 for next chain key.
    let mut mac_msg = HmacSha256::new_from_slice(chain_key).expect("HMAC accepts any key length");
    mac_msg.update(&[0x01]);
    let mk_bytes: [u8; 32] = mac_msg.finalize().into_bytes().into();

    let mut mac_ck = HmacSha256::new_from_slice(chain_key).expect("HMAC accepts any key length");
    mac_ck.update(&[0x02]);
    let new_ck: [u8; 32] = mac_ck.finalize().into_bytes().into();

    (new_ck, MessageKey::derive_from(mk_bytes))
}

/// Derived message key material: cipher key, MAC key, IV.
#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct MessageKey {
    pub cipher_key: [u8; 32], // AES-256-CBC
    pub mac_key: [u8; 32],    // HMAC-SHA256
    pub iv: [u8; 16],
}

impl MessageKey {
    fn derive_from(material: [u8; 32]) -> Self {
        let hk = Hkdf::<Sha256>::new(None, &material);
        let mut okm = [0u8; 80];
        hk.expand(KDF_KEYS_INFO, &mut okm)
            .expect("HKDF expand to 80 bytes must not fail");
        let mut key = Self {
            cipher_key: [0u8; 32],
            mac_key: [0u8; 32],
            iv: [0u8; 16],
        };
        key.cipher_key.copy_from_slice(&okm[..32]);
        key.mac_key.copy_from_slice(&okm[32..64]);
        key.iv.copy_from_slice(&okm[64..80]);
        key
    }
}

// --- Double Ratchet session state ---

/// Index key for a skipped message (ratchet public key + message counter).
#[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SkipKey {
    pub dh_public: [u8; 32],
    pub counter: u32,
}

mod skipped_keys_codec {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::HashMap;

    #[derive(Serialize, Deserialize)]
    struct Entry {
        dh_public: [u8; 32],
        counter: u32,
        key: Vec<u8>,
    }

    pub fn serialize<S>(map: &HashMap<SkipKey, Vec<u8>>, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries: Vec<Entry> = map
            .iter()
            .map(|(k, v)| Entry {
                dh_public: k.dh_public,
                counter: k.counter,
                key: v.clone(),
            })
            .collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> std::result::Result<HashMap<SkipKey, Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<Entry>::deserialize(deserializer)?;
        let mut map = HashMap::with_capacity(entries.len());
        for e in entries {
            map.insert(
                SkipKey {
                    dh_public: e.dh_public,
                    counter: e.counter,
                },
                e.key,
            );
        }
        Ok(map)
    }
}

/// Serialisable Double Ratchet session state.
///
/// Stored in `signal_session` SQLite table (encrypted with DPAPI).
/// Lifecycle evidence: `re-notes/36-zalo-e2ee-signal-protocol-analysis.md` §3.3
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct RatchetSession {
    /// Current sending ratchet key pair.
    pub dh_sending: Option<KeyPairBytes>,
    /// Remote party's current ratchet public key.
    pub dh_remote: Option<[u8; 32]>,
    /// Root key (32 bytes).
    pub root_key: [u8; 32],
    /// Chain key for sending (32 bytes).
    pub chain_key_send: Option<[u8; 32]>,
    /// Chain key for receiving (32 bytes).
    pub chain_key_recv: Option<[u8; 32]>,
    /// Number of messages sent in current sending chain.
    pub send_count: u32,
    /// Number of messages received in current receiving chain.
    pub recv_count: u32,
    /// Number of messages sent in the previous sending chain (for header).
    pub prev_send_count: u32,
    /// Skipped message keys buffered for out-of-order delivery.
    /// Stored as 80 raw bytes: cipher_key(32) || mac_key(32) || iv(16).
    #[serde(with = "skipped_keys_codec", default)]
    pub skipped: std::collections::HashMap<SkipKey, Vec<u8>>,
    /// Zalo addition: timestamp of last session reset for UI "new device" notice.
    pub session_ts: Option<i64>,
}

/// Portable key pair representation (private bytes zeroized on drop).
#[derive(serde::Serialize, serde::Deserialize, Clone, Zeroize)]
#[zeroize(drop)]
pub struct KeyPairBytes {
    pub private: [u8; 32],
    pub public: [u8; 32],
}

impl From<&KeyPair> for KeyPairBytes {
    fn from(kp: &KeyPair) -> Self {
        Self {
            private: kp.private.to_bytes(),
            public: *kp.public.as_bytes(),
        }
    }
}

impl From<&KeyPairBytes> for KeyPair {
    fn from(b: &KeyPairBytes) -> Self {
        KeyPair::from_private_bytes(b.private)
    }
}

impl RatchetSession {
    /// Initialise session as **Alice** (sender of first message, initiator of X3DH).
    ///
    /// Call after completing X3DH: pass the shared master secret and Bob's signed prekey.
    /// The signed prekey acts as Bob's *current* ratchet key, so Alice's first
    /// sending chain and Bob's first receiving chain derive from the same DH.
    pub fn init_as_sender(
        shared_secret: &[u8],             // 32-byte X3DH shared secret
        bob_signed_prekey: &PublicKey,     // Bob's SPK (Bob's current ratchet key)
    ) -> Result<Self> {
        let our_ratchet = KeyPair::generate();
        let dh_out = our_ratchet.dh(bob_signed_prekey)?;
        let (root_key, chain_key_send) = kdf_rk(&root_from_secret(shared_secret), dh_out.as_bytes());
        Ok(Self {
            dh_sending: Some(KeyPairBytes::from(&our_ratchet)),
            dh_remote: Some(*bob_signed_prekey.as_bytes()),
            root_key,
            chain_key_send: Some(chain_key_send),
            chain_key_recv: None,
            send_count: 0,
            recv_count: 0,
            prev_send_count: 0,
            skipped: std::collections::HashMap::new(),
            session_ts: None,
        })
    }

    /// Initialise session as **Bob** (receiver of the first PreKeyWhisperMessage).
    ///
    /// * `shared_secret` — 32-byte X3DH output (the root key both peers start from).
    /// * `alice_ratchet_key` — Alice's ratchet public key, taken from the message header.
    /// * `our_signed_prekey` — Bob's signed prekey, which is Bob's current ratchet key.
    ///
    /// The receiving chain is derived first (it must match Alice's sending chain),
    /// then Bob ratchets: a fresh key pair produces his sending chain.
    pub fn init_as_receiver(
        shared_secret: &[u8],
        alice_ratchet_key: &PublicKey,
        our_signed_prekey: &KeyPair,
    ) -> Result<Self> {
        let alice_key = *alice_ratchet_key.as_bytes();
        let dh_recv = our_signed_prekey.dh(alice_ratchet_key)?;
        let (root_key, chain_key_recv) = kdf_rk(&root_from_secret(shared_secret), dh_recv.as_bytes());

        // Bob's ratchet step: a new key pair yields the sending chain Alice expects.
        let bob_ratchet = KeyPair::generate();
        let dh_send = bob_ratchet.dh(alice_ratchet_key)?;
        let (root_key, chain_key_send) = kdf_rk(&root_key, dh_send.as_bytes());

        Ok(Self {
            dh_sending: Some(KeyPairBytes::from(&bob_ratchet)),
            dh_remote: Some(alice_key),
            root_key,
            chain_key_send: Some(chain_key_send),
            chain_key_recv: Some(chain_key_recv),
            send_count: 0,
            recv_count: 0,
            prev_send_count: 0,
            skipped: std::collections::HashMap::new(),
            session_ts: Some(now_ms()),
        })
    }

    /// Encrypt `plaintext` returning `(header, ciphertext)`.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<(MessageHeader, Vec<u8>)> {
        let ck = self
            .chain_key_send
            .as_mut()
            .context("Chain key send not initialised — call init_as_sender first")?;
        let (new_ck, mk) = kdf_ck(ck);
        *ck = new_ck;
        let n = self.send_count;
        self.send_count += 1;
        let dh_pub = self
            .dh_sending
            .as_ref()
            .context("DH sending key not set")?
            .public;
        let ct = aes256_cbc_encrypt(plaintext, &mk.cipher_key, &mk.iv)?;
        let mac = compute_mac(&mk.mac_key, &dh_pub, n, self.prev_send_count, &ct);
        let header = MessageHeader {
            msg_type: MessageType::Whisper as u8,
            dh_public: dh_pub,
            pn: self.prev_send_count,
            n,
        };
        let mut ciphertext_with_mac = ct;
        ciphertext_with_mac.extend_from_slice(&mac);
        Ok((header, ciphertext_with_mac))
    }

    /// Decrypt a received `(header, ciphertext_with_mac)`.
    pub fn decrypt(&mut self, header: &MessageHeader, ciphertext_with_mac: &[u8]) -> Result<Vec<u8>> {
        anyhow::ensure!(
            ciphertext_with_mac.len() >= 32,
            "Ciphertext too short (must include 32-byte MAC)"
        );
        let (ct, mac) = ciphertext_with_mac.split_at(ciphertext_with_mac.len() - 32);

        let mut candidate = self.clone();

        // Check skipped message keys first.
        let skip_key = SkipKey {
            dh_public: header.dh_public,
            counter: header.n,
        };
        if let Some(mk_bytes) = candidate.skipped.remove(&skip_key) {
            // Stored layout: cipher_key(32) || mac_key(32) || iv(16).
            let mk = MessageKey {
                cipher_key: mk_bytes[..32].try_into().expect("cipher key slice"),
                mac_key: mk_bytes[32..64].try_into().expect("mac key slice"),
                iv: mk_bytes[64..80].try_into().expect("iv slice"),
            };
            verify_mac(&mk.mac_key, &header.dh_public, header.n, header.pn, ct, mac)?;
            let pt = aes256_cbc_decrypt(ct, &mk.cipher_key, &mk.iv)?;
            *self = candidate;
            return Ok(pt);
        }

        // Perform a DH ratchet step when the remote key changed.
        if candidate.dh_remote.as_ref() != Some(&header.dh_public) {
            candidate.ratchet_step(&header.dh_public)?;
        }

        // Advance the receiving chain, skipping over any gap.
        candidate.skip_over_recv_chain(header.n)?;
        let ck = candidate
            .chain_key_recv
            .as_mut()
            .context("No receive chain key")?;
        let (new_ck, mk) = kdf_ck(ck);
        *ck = new_ck;
        candidate.recv_count = header.n + 1;

        verify_mac(&mk.mac_key, &header.dh_public, header.n, header.pn, ct, mac)?;
        let pt = aes256_cbc_decrypt(ct, &mk.cipher_key, &mk.iv)?;
        *self = candidate;
        Ok(pt)
    }

    /// One DH ratchet step: derive the receiving chain for the peer's new key,
    /// then rotate our own key and derive the matching sending chain.
    fn ratchet_step(&mut self, remote_pub_bytes: &[u8; 32]) -> Result<()> {
        let remote_pub = PublicKey::from(*remote_pub_bytes);
        let current = self
            .dh_sending
            .as_ref()
            .context("DH sending key not set — cannot ratchet")?;
        let current_kp = KeyPair::from(current);

        // 1) Receiving chain — matches the peer's sending chain.
        let dh_recv = current_kp.dh(&remote_pub)?;
        let (root_key, chain_key_recv) = kdf_rk(&self.root_key, dh_recv.as_bytes());

        // 2) Rotate our ratchet key and derive the sending chain.
        let new_send = KeyPair::generate();
        let dh_send = new_send.dh(&remote_pub)?;
        let (root_key, chain_key_send) = kdf_rk(&root_key, dh_send.as_bytes());

        self.prev_send_count = self.send_count;
        self.send_count = 0;
        self.recv_count = 0;
        self.dh_remote = Some(*remote_pub_bytes);
        self.dh_sending = Some(KeyPairBytes::from(&new_send));
        self.root_key = root_key;
        self.chain_key_send = Some(chain_key_send);
        self.chain_key_recv = Some(chain_key_recv);
        Ok(())
    }

    /// Store the message keys of the messages skipped by an out-of-order delivery.
    fn skip_over_recv_chain(&mut self, until: u32) -> Result<()> {
        let Some(ck) = self.chain_key_recv else {
            return Ok(());
        };
        let from = self.recv_count;
        anyhow::ensure!(
            (until as usize).saturating_sub(from as usize) <= MAX_SKIP,
            "Message skipped too many times ({} > {MAX_SKIP})",
            until.saturating_sub(from)
        );
        let mut ck = ck;
        let remote = self.dh_remote.unwrap_or([0u8; 32]);
        for n in from..until {
            let (new_ck, mk) = kdf_ck(&ck);
            ck = new_ck;
            let skip_key = SkipKey {
                dh_public: remote,
                counter: n,
            };
            let mut mk_store = vec![0u8; 80];
            mk_store[..32].copy_from_slice(&mk.cipher_key);
            mk_store[32..64].copy_from_slice(&mk.mac_key);
            mk_store[64..].copy_from_slice(&mk.iv);
            self.skipped.insert(skip_key, mk_store);
        }
        self.chain_key_recv = Some(ck);
        self.recv_count = until;
        Ok(())
    }
}

/// Zalo E2EE message types (evidence E2 — `re-notes/36-zalo-e2ee-signal-protocol-analysis.md` §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum MessageType {
    /// Regular Double Ratchet message.
    Whisper = 1,
    /// First message of a session, carries the X3DH base key.
    PreKeyWhisper = 3,
}

/// Message header transmitted with every E2EE message.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MessageHeader {
    /// 1 = WhisperMessage, 3 = PreKeyWhisperMessage.
    pub msg_type: u8,
    /// Sender's current ratchet DH public key.
    pub dh_public: [u8; 32],
    /// Number of messages in previous sending chain.
    pub pn: u32,
    /// Message index in current sending chain.
    pub n: u32,
}

// --- Symmetric crypto helpers ---

/// Interpret the 32-byte X3DH output as the initial root key.
fn root_from_secret(shared_secret: &[u8]) -> [u8; 32] {
    let mut rk = [0u8; 32];
    let len = shared_secret.len().min(32);
    rk[..len].copy_from_slice(&shared_secret[..len]);
    rk
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn aes256_cbc_encrypt(plaintext: &[u8], key: &[u8; 32], iv: &[u8; 16]) -> Result<Vec<u8>> {
    use aes::Aes256;
    use cbc::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
    let enc = cbc::Encryptor::<Aes256>::new_from_slices(key, iv)
        .map_err(|e| anyhow::anyhow!("AES key/IV error: {e}"))?;
    let mut buf = vec![0u8; plaintext.len() + 16];
    buf[..plaintext.len()].copy_from_slice(plaintext);
    let ct = enc
        .encrypt_padded_mut::<Pkcs7>(&mut buf, plaintext.len())
        .map_err(|e| anyhow::anyhow!("AES encrypt error: {e}"))?;
    Ok(ct.to_vec())
}

fn aes256_cbc_decrypt(ciphertext: &[u8], key: &[u8; 32], iv: &[u8; 16]) -> Result<Vec<u8>> {
    use aes::Aes256;
    use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
    let dec = cbc::Decryptor::<Aes256>::new_from_slices(key, iv)
        .map_err(|e| anyhow::anyhow!("AES key/IV error: {e}"))?;
    let mut buf = ciphertext.to_vec();
    let pt = dec
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|_| anyhow::anyhow!("AES decrypt failed — wrong key or tampered ciphertext"))?;
    Ok(pt.to_vec())
}

fn compute_mac(
    mac_key: &[u8; 32],
    dh_pub: &[u8; 32],
    n: u32,
    pn: u32,
    ciphertext: &[u8],
) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(mac_key).expect("HMAC accepts any key length");
    mac.update(dh_pub);
    mac.update(&n.to_be_bytes());
    mac.update(&pn.to_be_bytes());
    mac.update(ciphertext);
    mac.finalize().into_bytes().into()
}

fn verify_mac(
    mac_key: &[u8; 32],
    dh_pub: &[u8; 32],
    n: u32,
    pn: u32,
    ciphertext: &[u8],
    expected: &[u8],
) -> Result<()> {
    let mut mac = HmacSha256::new_from_slice(mac_key).expect("HMAC accepts any key length");
    mac.update(dh_pub);
    mac.update(&n.to_be_bytes());
    mac.update(&pn.to_be_bytes());
    mac.update(ciphertext);
    mac.verify_slice(expected)
        .map_err(|_| anyhow::anyhow!("E2EE MAC verification failed — message may be tampered"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Derive a 32-byte shared secret simulating X3DH output (simplified test fixture).
    fn fake_x3dh(alice: &KeyPair, bob_spk: &KeyPair) -> [u8; 32] {
        let shared = alice.dh(&bob_spk.public).expect("fake_x3dh contributory");
        *shared.as_bytes()
    }

    /// Alice and Bob establish a session the way the real handshake does.
    fn pair() -> (RatchetSession, RatchetSession) {
        let bob_spk = KeyPair::generate();
        let alice_ek = KeyPair::generate();
        let ss = fake_x3dh(&alice_ek, &bob_spk);
        let alice = RatchetSession::init_as_sender(&ss, &bob_spk.public).expect("init alice sender");
        let alice_key = PublicKey::from(alice.dh_sending.as_ref().unwrap().public);
        let bob = RatchetSession::init_as_receiver(&ss, &alice_key, &bob_spk).expect("init bob receiver");
        (alice, bob)
    }

    #[test]
    fn roundtrip_establishes_session_then_ratchets() {
        let (mut alice, mut bob) = pair();

        let msg1 = b"Xin chao Bob";
        let (hdr1, ct1) = alice.encrypt(msg1).unwrap();
        let pt1 = bob.decrypt(&hdr1, &ct1).unwrap();
        assert_eq!(pt1, msg1);

        // Bob replies — triggers a ratchet step on Alice's side.
        let msg2 = b"Xin chao Alice";
        let (hdr2, ct2) = bob.encrypt(msg2).unwrap();
        let pt2 = alice.decrypt(&hdr2, &ct2).unwrap();
        assert_eq!(pt2, msg2);

        // Both directions keep working after the ratchet.
        let (hdr3, ct3) = alice.encrypt(b"third").unwrap();
        assert_eq!(bob.decrypt(&hdr3, &ct3).unwrap(), b"third");
        let (hdr4, ct4) = bob.encrypt(b"fourth").unwrap();
        assert_eq!(alice.decrypt(&hdr4, &ct4).unwrap(), b"fourth");
    }

    #[test]
    fn many_messages_in_both_directions() {
        let (mut alice, mut bob) = pair();
        for i in 0..10 {
            let text = format!("alice-{i}");
            let (h, c) = alice.encrypt(text.as_bytes()).unwrap();
            assert_eq!(bob.decrypt(&h, &c).unwrap(), text.as_bytes());
            let reply = format!("bob-{i}");
            let (h, c) = bob.encrypt(reply.as_bytes()).unwrap();
            assert_eq!(alice.decrypt(&h, &c).unwrap(), reply.as_bytes());
        }
    }

    #[test]
    fn out_of_order_message_is_decrypted_later() {
        let (mut alice, mut bob) = pair();
        let (h1, c1) = alice.encrypt(b"one").unwrap();
        let (h2, c2) = alice.encrypt(b"two").unwrap();
        let (h3, c3) = alice.encrypt(b"three").unwrap();
        // Deliver 3 then 1 — message 2 must still decrypt afterwards.
        assert_eq!(bob.decrypt(&h3, &c3).unwrap(), b"three");
        assert_eq!(bob.decrypt(&h1, &c1).unwrap(), b"one");
        assert_eq!(bob.decrypt(&h2, &c2).unwrap(), b"two");
    }

    #[test]
    fn tampered_ciphertext_rejected() {
        let (mut alice, mut bob) = pair();
        let (hdr, mut ct) = alice.encrypt(b"secret").unwrap();
        ct[0] ^= 0xff; // tamper
        assert!(bob.decrypt(&hdr, &ct).is_err());
    }

    #[test]
    fn wrong_peer_cannot_decrypt() {
        let bob_spk = KeyPair::generate();
        let alice_ek = KeyPair::generate();
        let ss = fake_x3dh(&alice_ek, &bob_spk);
        let mut alice = RatchetSession::init_as_sender(&ss, &bob_spk.public).unwrap();
        let alice_key = PublicKey::from(alice.dh_sending.as_ref().unwrap().public);
        let mut eve = RatchetSession::init_as_receiver(&ss, &alice_key, &bob_spk).unwrap();

        // A session established with a different peer must not decrypt these messages.
        let stranger_spk = KeyPair::generate();
        let ss2 = fake_x3dh(&alice_ek, &stranger_spk);
        let mut other = RatchetSession::init_as_sender(&ss2, &stranger_spk.public).unwrap();
        let (h2, c2) = other.encrypt(b"other").unwrap();
        assert!(eve.decrypt(&h2, &c2).is_err());

        // The genuine peer decrypts them.
        let mut bob = RatchetSession::init_as_receiver(&ss, &alice_key, &bob_spk).unwrap();
        let (h, c) = alice.encrypt(b"secret").unwrap();
        assert_eq!(bob.decrypt(&h, &c).unwrap(), b"secret");
    }

    #[test]
    fn kdf_rk_and_ck_are_deterministic() {
        let rk = [42u8; 32];
        let dh = [7u8; 32];
        let (rk2, ck) = kdf_rk(&rk, &dh);
        let (rk2b, ckb) = kdf_rk(&rk, &dh);
        assert_eq!(rk2, rk2b);
        assert_eq!(ck, ckb);

        let (ck2, mk) = kdf_ck(&ck);
        let (ck2b, mkb) = kdf_ck(&ck);
        assert_eq!(ck2, ck2b);
        assert_eq!(mk.cipher_key, mkb.cipher_key);
    }

    #[test]
    fn skipped_keys_json_serialization_roundtrip_a06() {
        let (mut alice, mut bob) = pair();
        let (h1, c1) = alice.encrypt(b"one").unwrap();
        let (_h2, _c2) = alice.encrypt(b"two").unwrap();
        let (h3, c3) = alice.encrypt(b"three").unwrap();

        // Deliver msg 3 to force msg 1 and 2 into bob's skipped map
        assert_eq!(bob.decrypt(&h3, &c3).unwrap(), b"three");
        assert!(!bob.skipped.is_empty(), "Bob must have skipped keys buffered");

        // Serialize bob's session to JSON (would fail without skipped_keys_codec)
        let json = serde_json::to_string(&bob).expect("Must serialize RatchetSession with skipped keys");
        let mut bob_restored: RatchetSession = serde_json::from_str(&json).expect("Must deserialize RatchetSession");

        // Ensure restored session can decrypt skipped message 1
        assert_eq!(bob_restored.decrypt(&h1, &c1).unwrap(), b"one");
    }

    #[test]
    fn decrypt_failure_rolls_back_state_a38() {
        let (mut alice, mut bob) = pair();
        let (h1, mut c1) = alice.encrypt(b"integrity test").unwrap();
        c1[0] ^= 0xFF; // Corrupt ciphertext / MAC

        let bob_state_before = bob.clone();
        assert!(bob.decrypt(&h1, &c1).is_err(), "Corrupt decryption must fail");

        // State must remain identical to before the failed decryption attempt
        assert_eq!(bob.recv_count, bob_state_before.recv_count);
        assert_eq!(bob.root_key, bob_state_before.root_key);
        assert_eq!(bob.chain_key_recv, bob_state_before.chain_key_recv);
        assert_eq!(bob.skipped.len(), bob_state_before.skipped.len());
    }

    #[test]
    fn counter_sync_after_skipping_a37() {
        let (mut alice, mut bob) = pair();
        let (_h1, _c1) = alice.encrypt(b"skip-0").unwrap();
        let (_h2, _c2) = alice.encrypt(b"skip-1").unwrap();
        let (h3, c3) = alice.encrypt(b"msg-2").unwrap();

        assert_eq!(bob.decrypt(&h3, &c3).unwrap(), b"msg-2");
        assert_eq!(bob.recv_count, 3, "Recv count must be advanced to header.n + 1");
    }
}
