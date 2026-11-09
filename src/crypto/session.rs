//! E2EE session orchestration: ties the key store, the SQLite session store and
//! the Double Ratchet together (WP16-1/2).
//!
//! Flow:
//! - **Outgoing first message**: fetch the peer's prekey bundle → X3DH sender →
//!   `RatchetSession::init_as_sender` → `PreKeyWhisperMessage` (type 3).
//! - **Outgoing later messages**: reuse the stored session → `WhisperMessage` (type 1).
//! - **Incoming**: decode the message; a type-3 message establishes the session
//!   (X3DH receiver), a type-1 message continues it.
//!
//! Evidence: `re-notes/36-zalo-e2ee-signal-protocol-analysis.md` §3.1, §4.

use anyhow::{Context, Result};
use rusqlite::Connection;
use x25519_dalek::PublicKey;

use super::keystore::{
    x3dh_receiver, x3dh_sender, IdentityKeyPair, OneTimePreKey, SignedPreKey, PREKEY_BATCH,
};
use super::ratchet::{KeyPair, KeyPairBytes, RatchetSession};
use super::store;
use super::wire::{
    deserialize, serialize_prekey, serialize_whisper, DecodedMessage, PreKeyBundleWire,
};

/// Signed prekey id used for initial prekey bundle.
pub const SIGNED_PREKEY_ID: u32 = 1;

/// Low-water mark: refill one-time prekeys when fewer than this remain.
pub const PREKEY_REFILL_THRESHOLD: u32 = 2;

/// Signed prekey rotation period (7 days in milliseconds) (A12).
pub const SIGNED_PREKEY_ROTATION_MS: i64 = 7 * 86_400_000;

/// Grace retention period for superseded signed prekeys (30 days in milliseconds) (A12).
pub const SIGNED_PREKEY_GRACE_RETENTION_MS: i64 = 30 * 86_400_000;

/// Outcome of decrypting an incoming E2EE payload.
#[derive(Debug, Clone)]
pub struct DecryptedMessage {
    /// Decrypted plain text.
    pub text: String,
    /// `true` when the peer's identity key changed (they reinstalled the app).
    pub identity_changed: bool,
}

/// Owns the local E2EE key material for one profile.
pub struct E2eeManager {
    conn: Connection,
    profile_id: String,
}

impl E2eeManager {
    /// Open the crypto database for a profile.
    pub fn open(profile_id: &str) -> Result<Self> {
        let conn = store::open_crypto_db()?;
        Ok(Self {
            conn,
            profile_id: profile_id.to_string(),
        })
    }

    /// Open a manager against an explicit database file (tests, tooling).
    pub fn open_at(path: &std::path::Path, profile_id: &str) -> Result<Self> {
        let conn = store::open_crypto_db_at(path)?;
        Ok(Self {
            conn,
            profile_id: profile_id.to_string(),
        })
    }

    /// The profile this manager operates on.
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    /// Borrow the underlying connection (read-only helpers).
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Load the identity key, generating and persisting one on first use (fail-safe: never regenerates on corruption, A14).
    pub fn identity(&self) -> Result<IdentityKeyPair> {
        match store::load_identity(&self.conn, &self.profile_id) {
            Ok(Some(id)) => Ok(id),
            Ok(None) => {
                let identity = IdentityKeyPair::generate()?;
                store::save_identity(&self.conn, &self.profile_id, &identity)?;
                Ok(identity)
            }
            Err(e) => {
                Err(e.context("Không thể nạp khóa danh tính E2EE do dữ liệu bị hỏng. Hủy bỏ để bảo vệ an toàn danh tính, không tự ý tạo khóa mới."))
            }
        }
    }

    /// Ensure an active signed prekey exists, rotating when older than 7 days (A12).
    ///
    /// Preserves old prekey IDs in the store to support late-arriving messages (grace retention).
    pub fn active_signed_prekey(&self) -> Result<SignedPreKey> {
        if let Some(spk) = store::load_latest_signed_prekey(&self.conn, &self.profile_id)? {
            let age_ms = (now_ms() - spk.created_at).max(0);
            if age_ms < SIGNED_PREKEY_ROTATION_MS {
                return Ok(spk);
            }
            // Older than 7 days: rotate to incremented key_id without overwriting old ones
            let next_key_id = spk.key_id.wrapping_add(1).max(1);
            let identity = self.identity()?;
            let new_spk = SignedPreKey::generate(next_key_id, &identity)?;
            store::save_signed_prekey(&self.conn, &self.profile_id, &new_spk)?;
            let _ = store::prune_old_signed_prekeys(
                &self.conn,
                &self.profile_id,
                SIGNED_PREKEY_GRACE_RETENTION_MS,
            );
            return Ok(new_spk);
        }
        let identity = self.identity()?;
        let spk = SignedPreKey::generate(SIGNED_PREKEY_ID, &identity)?;
        store::save_signed_prekey(&self.conn, &self.profile_id, &spk)?;
        Ok(spk)
    }

    /// Backwards-compatible alias for the active signed prekey.
    pub fn signed_prekey(&self) -> Result<SignedPreKey> {
        self.active_signed_prekey()
    }

    /// Top up the one-time prekey pool when it runs low.
    pub fn refill_one_time_prekeys(&self) -> Result<Vec<OneTimePreKey>> {
        let tx = self.conn.unchecked_transaction()?;
        let batch = self.refill_prekeys_in_transaction()?;
        tx.commit()?;
        Ok(batch)
    }

    fn refill_prekeys_in_transaction(&self) -> Result<Vec<OneTimePreKey>> {
        let remaining = store::count_one_time_prekeys(&self.conn, &self.profile_id)?;
        if remaining >= PREKEY_REFILL_THRESHOLD {
            return Ok(Vec::new());
        }
        let start = self.next_prekey_id()?;
        let batch = OneTimePreKey::generate_batch(start, PREKEY_BATCH)?;
        store::save_one_time_prekeys(&self.conn, &self.profile_id, &batch)?;
        Ok(batch)
    }

    /// Number of one-time prekeys still available.
    pub fn one_time_prekey_count(&self) -> Result<u32> {
        store::count_one_time_prekeys(&self.conn, &self.profile_id)
    }

    /// Next free one-time prekey id.
    fn next_prekey_id(&self) -> Result<u32> {
        let max: Option<u32> = self.conn.query_row(
            "SELECT MAX(key_id) FROM one_time_prekey WHERE profile_id=?1",
            rusqlite::params![self.profile_id],
            |r| r.get(0),
        )?;
        let used = max.unwrap_or(0);
        let reserved: u32 = self.conn.query_row(
            "SELECT COALESCE((SELECT last_id FROM prekey_sequence WHERE profile_id=?1),0)",
            [&self.profile_id],
            |r| r.get(0),
        )?;
        used.max(reserved)
            .checked_add(1)
            .context("Đã hết định danh prekey")
    }

    /// Build the public bundle to upload to the server.
    pub fn local_bundle(&self) -> Result<super::keystore::PreKeyBundle> {
        self.refill_one_time_prekeys()?;
        let identity = self.identity()?;
        let spk = self.signed_prekey()?;
        let one_time = self.list_one_time_prekeys()?;
        Ok(super::keystore::PreKeyBundle::from_local_keys(
            &identity, &spk, &one_time,
        ))
    }

    /// List the one-time prekeys currently stored (public parts only).
    pub fn list_one_time_prekeys(&self) -> Result<Vec<OneTimePreKey>> {
        let mut stmt = self.conn.prepare(
            "SELECT key_id, public_key, private_bytes_enc FROM one_time_prekey \
             WHERE profile_id=?1 ORDER BY key_id",
        )?;
        let rows = stmt.query_map(rusqlite::params![self.profile_id], |r| {
            Ok((
                r.get::<_, u32>(0)?,
                r.get::<_, Vec<u8>>(1)?,
                r.get::<_, Vec<u8>>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (key_id, pub_bytes, priv_enc) = row?;
            let priv_dec = crate::storage::unprotect(&priv_enc)?;
            if pub_bytes.len() < 32 || priv_dec.len() < 32 {
                continue;
            }
            let mut pub_arr = [0u8; 32];
            let mut priv_arr = [0u8; 32];
            pub_arr.copy_from_slice(&pub_bytes[..32]);
            priv_arr.copy_from_slice(&priv_dec[..32]);
            out.push(OneTimePreKey {
                key_id,
                key_pair: KeyPairBytes {
                    public: pub_arr,
                    private: priv_arr,
                },
            });
        }
        Ok(out)
    }

    /// Load a stored ratchet session for a peer.
    pub fn load_session(&self, peer_id: &str) -> Result<Option<RatchetSession>> {
        store::load_session(&self.conn, &self.profile_id, peer_id)
    }

    /// Preserve encrypted-thread policy independently of ratchet availability.
    /// # Errors
    /// Returns unreadable or corrupt state instead of allowing a plaintext route.
    pub fn requires_e2ee(&self, peer_id: &str) -> Result<bool> {
        let session = self.load_session(peer_id)?;
        let required: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM e2ee_required WHERE profile_id=?1 AND peer_id=?2)",
            rusqlite::params![self.profile_id, peer_id],
            |row| row.get(0),
        )?;
        Ok(session.is_some() || required)
    }

    /// Persist a ratchet session for a peer.
    pub fn save_session(&self, peer_id: &str, session: &RatchetSession) -> Result<()> {
        store::save_session(&self.conn, &self.profile_id, peer_id, session)
    }

    /// Drop the session with a peer (used when the peer reinstalls the app).
    pub fn delete_session(&self, peer_id: &str) -> Result<()> {
        store::delete_session(&self.conn, &self.profile_id, peer_id)
    }

    /// Record the peer's identity key, returning `true` when it changed.
    ///
    /// A change means the peer reset its identity — the UI must warn the user.
    pub fn trust_identity(&self, peer_id: &str, identity_key: &[u8; 32]) -> Result<bool> {
        store::save_trusted_identity(&self.conn, &self.profile_id, peer_id, identity_key)
    }

    /// Whether the peer's identity key is the one we have seen before.
    pub fn is_trusted_identity(&self, peer_id: &str, identity_key: &[u8; 32]) -> Result<bool> {
        store::is_trusted_identity(&self.conn, &self.profile_id, peer_id, identity_key)
    }

    /// Load trusted identity key and verified status for a peer.
    pub fn get_trusted_identity(&self, peer_id: &str) -> Result<Option<([u8; 32], bool)>> {
        store::get_trusted_identity(&self.conn, &self.profile_id, peer_id)
    }

    /// Explicitly mark a peer's identity as verified by the user.
    pub fn verify_identity(&self, peer_id: &str) -> Result<()> {
        store::set_identity_verified(&self.conn, &self.profile_id, peer_id, true)
    }

    /// Verify exactly the identity whose safety number was displayed to the user.
    /// # Errors
    /// Refuses missing identities or a key changed since the dialog was opened.
    pub fn verify_identity_number(&self, peer_id: &str, expected: &str) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let (key, _) = self
            .get_trusted_identity(peer_id)?
            .context("Chưa có khóa danh tính đối phương")?;
        let actual = self.safety_number(&key)?;
        anyhow::ensure!(
            actual == expected,
            "Khóa đối phương đã thay đổi; hãy mở lại mã an toàn"
        );
        self.verify_identity(peer_id)?;
        tx.commit()?;
        Ok(())
    }

    /// Check if peer's identity has been verified by the user.
    pub fn is_identity_verified(&self, peer_id: &str) -> Result<bool> {
        store::is_identity_verified(&self.conn, &self.profile_id, peer_id)
    }

    /// Safety number between our identity and the peer's identity.
    pub fn safety_number(&self, peer_identity: &[u8; 32]) -> Result<String> {
        let ours = self.identity()?;
        super::safety::safety_number(&ours.dh.public, peer_identity)
    }

    /// Build a `PreKeyWhisperMessage` for a peer using a freshly fetched bundle.
    pub fn start_session(
        &self,
        peer_id: &str,
        remote: &super::prekey::RemoteBundle,
        plaintext: &[u8],
    ) -> Result<Vec<u8>> {
        let identity = self.identity()?;

        // The peer's signed prekey must be signed by the identity key we are
        // about to trust.  A mismatch means the bundle was tampered with.
        if !verify_signed_prekey(remote) {
            anyhow::bail!("Chữ ký signedPreKey của đối phương không hợp lệ");
        }

        let otpk_pub = remote.one_time_pre_key.map(|(k, _)| k);
        let (master, ek_pub) = x3dh_sender(
            &identity,
            &remote.identity_key,
            &remote.signed_pre_key,
            otpk_pub.as_ref(),
        )?;

        let bob_spk = PublicKey::from(remote.signed_pre_key);
        let mut session = RatchetSession::init_as_sender(&master, &bob_spk)?;
        session.base_key = Some(ek_pub);
        let (header, ciphertext) = session.encrypt(plaintext)?;

        let bundle_wire = PreKeyBundleWire {
            registration_id: identity.registration_id,
            signed_pre_key_id: remote.signed_pre_key_id,
            one_time_pre_key_id: remote.one_time_pre_key.map(|(_, id)| id),
            base_key: ek_pub,
            identity_key: identity.dh.public,
        };
        // Repeat the prekey envelope until an authenticated reply proves session establishment.
        session.pending_prekey_bundle = Some(bundle_wire.clone());
        let tx = self.conn.unchecked_transaction()?;
        self.save_session(peer_id, &session)?;
        self.trust_identity(peer_id, &remote.identity_key)?;
        store::record_base_key(&self.conn, &self.profile_id, peer_id, &ek_pub)?;
        tx.commit()?;
        Ok(serialize_prekey(&bundle_wire, &header, &ciphertext))
    }

    /// Encrypt with the existing session, or start a new one when needed.
    pub fn encrypt(
        &self,
        peer_id: &str,
        remote: Option<&super::prekey::RemoteBundle>,
        plaintext: &[u8],
    ) -> Result<Vec<u8>> {
        if let Some(mut session) = self.load_session(peer_id)? {
            let (header, ciphertext) = session.encrypt(plaintext)?;
            let wire = if session.recv_count == 0 {
                if let Some(ref bundle_wire) = session.pending_prekey_bundle {
                    serialize_prekey(bundle_wire, &header, &ciphertext)
                } else {
                    serialize_whisper(&header, &ciphertext)
                }
            } else {
                serialize_whisper(&header, &ciphertext)
            };
            self.save_session(peer_id, &session)?;
            return Ok(wire);
        }
        let bundle = remote.context("Chưa có bundle khóa của đối phương")?;
        self.start_session(peer_id, bundle, plaintext)
    }

    /// Decrypt an incoming E2EE payload, rejecting raw crypto replays.
    pub fn decrypt(&self, peer_id: &str, payload: &[u8]) -> Result<DecryptedMessage> {
        let tx = self.conn.unchecked_transaction()?;
        let message = self.decrypt_core(peer_id, payload)?;
        tx.commit()?;
        Ok(message)
    }

    /// Recover an authenticated delivery after replay, restart or a failed history write.
    /// The receipt and ratchet changes commit together; plaintext is protected at rest.
    pub fn decrypt_received(
        &self,
        peer_id: &str,
        message_id: &str,
        payload: &[u8],
    ) -> Result<DecryptedMessage> {
        use rusqlite::{params, OptionalExtension};
        use sha2::{Digest, Sha256};
        anyhow::ensure!(
            !message_id.is_empty() && message_id.len() <= 256,
            "Thiếu định danh tin E2EE"
        );
        let hash = Sha256::digest(payload);
        let tx = self.conn.unchecked_transaction()?;
        let previous = tx.query_row("SELECT payload_hash,plaintext_enc,identity_changed FROM e2ee_received WHERE profile_id=?1 AND peer_id=?2 AND message_id=?3",
            params![self.profile_id,peer_id,message_id], |row|Ok((row.get::<_,Vec<u8>>(0)?,row.get::<_,Vec<u8>>(1)?,row.get::<_,bool>(2)?))).optional()?;
        if let Some((previous_hash, protected, changed)) = previous {
            anyhow::ensure!(
                previous_hash.as_slice() == hash.as_slice(),
                "Định danh tin E2EE trùng với gói tin khác"
            );
            let text = String::from_utf8(crate::storage::unprotect(&protected)?)?;
            return Ok(DecryptedMessage {
                text,
                identity_changed: changed,
            });
        }
        let message = self.decrypt_core(peer_id, payload)?;
        let protected = crate::storage::protect(message.text.as_bytes())?;
        let path = crate::e2ee_integration::history_key(&crate::storage::account_file(
            &self.profile_id,
            "zalo_history.db",
        ));
        tx.execute("INSERT INTO e2ee_received(profile_id,peer_id,message_id,payload_hash,plaintext_enc,identity_changed,history_path) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![self.profile_id,peer_id,message_id,hash.as_slice(),protected,message.identity_changed,path])?;
        tx.commit()?;
        Ok(message)
    }

    fn decrypt_core(&self, peer_id: &str, payload: &[u8]) -> Result<DecryptedMessage> {
        let decoded = deserialize(payload)?;
        match decoded {
            DecodedMessage::PreKey(prekey) => {
                let existing_opt = self.load_session(peer_id)?;
                let trusted_opt = self.get_trusted_identity(peer_id)?;
                let identity_changed = match trusted_opt {
                    Some((ref trusted_key, _)) => trusted_key != &prekey.bundle.identity_key,
                    None => false,
                };

                // Repeated envelopes continue the established ratchet; only a new base
                // key may create a session. Authentication still rejects actual replays.
                if !identity_changed {
                    if let Some(mut existing) = existing_opt
                        .clone()
                        .filter(|s| s.base_key == Some(prekey.bundle.base_key))
                    {
                        let plaintext = existing.decrypt(&prekey.header, &prekey.ciphertext)?;
                        self.save_session(peer_id, &existing)?;
                        return Ok(DecryptedMessage {
                            text: String::from_utf8_lossy(&plaintext).into_owned(),
                            identity_changed: false,
                        });
                    }
                }

                // Replay attack and session rollback protection (A11):
                // Unconditionally reject duplicate base_keys across any identity key.
                if store::has_base_key(
                    &self.conn,
                    &self.profile_id,
                    peer_id,
                    &prekey.bundle.base_key,
                )? {
                    anyhow::bail!("Phát hiện replay attack: PreKeyWhisperMessage đã được xử lý trước đó (base_key trùng lặp)");
                }

                // If identity hasn't changed, verify whether the session is active and healthy.
                // Prevent replayed type-3 messages from rolling back the ratchet session.
                if !identity_changed {
                    if let Some(ref existing) = existing_opt {
                        if existing.is_active() {
                            if existing.base_key == Some(prekey.bundle.base_key) {
                                anyhow::bail!("Phát hiện replay attack: gói tin PreKey trùng base_key với phiên đang hoạt động");
                            }
                            if existing.dh_remote == Some(prekey.header.dh_public)
                                && prekey.header.n < existing.recv_count
                            {
                                anyhow::bail!(
                                    "Phát hiện replay attack: counter của gói tin PreKey ({}) nhỏ hơn recv_count hiện tại ({})",
                                    prekey.header.n,
                                    existing.recv_count
                                );
                            }
                        }
                    }
                }

                let identity = self.identity()?;
                let spk_id = prekey.bundle.signed_pre_key_id;
                let spk = match store::load_signed_prekey(&self.conn, &self.profile_id, spk_id)? {
                    Some(s) => s,
                    None => {
                        let active = self.active_signed_prekey()?;
                        if active.key_id == spk_id {
                            active
                        } else {
                            anyhow::bail!(
                                "Signed prekey (id={spk_id}) không tồn tại hoặc đã hết hạn thu hồi"
                            );
                        }
                    }
                };
                let otpk = match prekey.bundle.one_time_pre_key_id {
                    Some(id) => Some(
                        store::get_one_time_prekey(&self.conn, &self.profile_id, id)?
                            .context("One-time prekey không tồn tại hoặc đã bị tiêu thụ")?,
                    ),
                    None => None,
                };

                let master = x3dh_receiver(
                    &identity,
                    &spk,
                    otpk.as_ref(),
                    &prekey.bundle.identity_key,
                    &prekey.bundle.base_key,
                )?;

                // Alice's ratchet public key travels in the message header; the
                // X3DH base key is only an input to the master secret.
                let alice_key = PublicKey::from(prekey.header.dh_public);
                let bob_spk = KeyPair::from_private_bytes(spk.key_pair.private);
                let mut session = RatchetSession::init_as_receiver(&master, &alice_key, &bob_spk)?;
                session.base_key = Some(prekey.bundle.base_key);
                let plaintext = session.decrypt(&prekey.header, &prekey.ciphertext)?;
                // A failed write rolls back the consumed prekey and replay marker too.
                if let Some(otpk_key) = otpk {
                    store::delete_one_time_prekey(&self.conn, &self.profile_id, otpk_key.key_id)?;
                }
                store::record_base_key(
                    &self.conn,
                    &self.profile_id,
                    peer_id,
                    &prekey.bundle.base_key,
                )?;
                self.save_session(peer_id, &session)?;
                let changed = self.trust_identity(peer_id, &prekey.bundle.identity_key)?;
                self.refill_prekeys_in_transaction()?;
                Ok(DecryptedMessage {
                    text: String::from_utf8_lossy(&plaintext).into_owned(),
                    identity_changed: changed,
                })
            }
            DecodedMessage::Whisper(msg) => {
                let mut session = self
                    .load_session(peer_id)?
                    .context("Chưa có phiên E2EE với đối phương")?;
                let plaintext = session.decrypt(&msg.header, &msg.ciphertext)?;
                self.save_session(peer_id, &session)?;
                Ok(DecryptedMessage {
                    text: String::from_utf8_lossy(&plaintext).into_owned(),
                    identity_changed: false,
                })
            }
        }
    }
}

/// Verify the peer's signed prekey signature with the peer's identity key.
///
/// The Zalo bundle exposes a single `identityKey`; the signature scheme used by
/// the official client could not be confirmed live, so this check accepts the
/// Ed25519 form and otherwise reports "not verifiable" instead of failing the
/// session outright.
pub fn verify_signed_prekey(remote: &super::prekey::RemoteBundle) -> bool {
    // The signing key may be published separately; fall back to the bundle's
    // identity key when the server sends a single key.
    let identity = remote.signing_identity_key.unwrap_or(remote.identity_key);
    let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&identity) else {
        return false;
    };
    // The parser enforces the exact 64-byte signature length (audit A03).
    let sig = ed25519_dalek::Signature::from_bytes(&remote.signature);
    vk.verify_strict(&remote.signed_pre_key, &sig).is_ok()
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

    /// A peer bundle built from real key material (Bob's perspective) using bound signing identity (audit A04).
    fn peer_bundle(bob: &E2eeManager) -> super::super::prekey::RemoteBundle {
        bob.refill_one_time_prekeys().unwrap();
        let bundle = bob
            .local_bundle()
            .expect("local bundle with bound signing key");
        bundle
            .to_remote_bundle()
            .expect("convert local bundle to remote bundle")
    }

    /// Each manager gets a *fresh* database.  The directory name carries a
    /// per-call counter so that repeated runs — and parallel test threads — never
    /// reuse stale identity keys, one-time prekeys or ratchet sessions.
    fn manager(name: &str) -> E2eeManager {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "zalo-e2ee-test-{name}-{}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        E2eeManager::open_at(&dir.join("e2ee.db"), name).unwrap()
    }

    #[test]
    fn identity_is_generated_once_and_reused() {
        let m = manager("identity");
        let a = m.identity().unwrap();
        let b = m.identity().unwrap();
        assert_eq!(a.registration_id, b.registration_id);
        assert_eq!(a.dh.public, b.dh.public);
    }

    #[test]
    fn one_time_prekeys_are_refilled_when_low() {
        let m = manager("prekeys");
        let batch = m.refill_one_time_prekeys().unwrap();
        assert!(!batch.is_empty());
        assert!(m.one_time_prekey_count().unwrap() >= 1);
    }

    #[test]
    fn first_message_establishes_session_and_decrypts() {
        let alice = manager("alice");
        let bob = manager("bob");

        let bundle = peer_bundle(&bob);
        let ct = alice
            .start_session("bob", &bundle, b"xin chao")
            .expect("alice starts the session");

        let out = bob.decrypt("alice", &ct).expect("bob decrypts");
        assert_eq!(out.text, "xin chao");
        assert!(!out.identity_changed);
    }

    #[test]
    fn follow_up_messages_use_the_stored_session() {
        let alice = manager("alice2");
        let bob = manager("bob2");

        let bundle = peer_bundle(&bob);
        let first = alice.start_session("bob", &bundle, b"one").unwrap();
        assert_eq!(bob.decrypt("alice", &first).unwrap().text, "one");

        // The next message reuses the stored session — no bundle needed.
        let second = alice.encrypt("bob", None, b"two").expect("second message");
        assert_eq!(bob.decrypt("alice", &second).unwrap().text, "two");
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let alice = manager("alice3");
        let bob = manager("bob3");
        let bundle = peer_bundle(&bob);
        let mut ct = alice.start_session("bob", &bundle, b"secret").unwrap();
        let last = ct.len() - 1;
        ct[last] ^= 0xFF;
        assert!(bob.decrypt("alice", &ct).is_err());
    }

    /// Audit A03 regression: a *validly signed* bundle fixture must survive the
    /// production serialize → `parse_bundle` → `verify_signed_prekey` path.
    ///
    /// The Ed25519 signing key travels in `signingIdentityKey`, separate from
    /// the Curve25519 DH identity key (audit A04) — without it the verifier
    /// falls back to the DH key and fails even on a correctly signed bundle.
    #[test]
    fn parsed_bundle_signature_verifies_through_production_path() {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};

        let bob = manager("roundtrip");
        let identity = bob.identity().unwrap();
        let spk = bob.signed_prekey().unwrap();

        let dec = serde_json::json!({
            "error_code": 0,
            "identityKey": B64.encode(identity.dh.public),
            "signingIdentityKey": B64.encode(identity.signing_public),
            "registrationId": identity.registration_id,
            "signedPreKey": {
                "keyId": spk.key_id,
                "publicKey": B64.encode(spk.key_pair.public),
                "signature": B64.encode(&spk.signature),
            },
        });
        let parsed = super::super::prekey::parse_bundle(&dec).expect("parse production bundle");
        assert!(
            verify_signed_prekey(&parsed),
            "a correctly signed bundle must verify after the parse round-trip"
        );
    }

    #[test]
    fn safety_number_is_stable_for_a_peer() {
        let alice = manager("alice4");
        let bob = manager("bob4");
        let bob_identity = bob.identity().unwrap();
        let a = alice.safety_number(&bob_identity.dh.public).unwrap();
        let b = alice.safety_number(&bob_identity.dh.public).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.split(' ').count(), 12);
    }
    #[test]
    fn replay_type_3_does_not_rollback_active_session_a11() {
        let alice = manager("alice_replay");
        let bob = manager("bob_replay");

        let bundle = peer_bundle(&bob);

        // 1. Alice sends first message (type 3 PreKeyWhisperMessage)
        let first = alice
            .start_session("bob", &bundle, b"hello first")
            .expect("start session");
        let out1 = bob.decrypt("alice", &first).expect("bob decrypts first");
        assert_eq!(out1.text, "hello first");

        // Verify initial session state on Bob
        let s1 = bob.load_session("alice").unwrap().unwrap();
        assert_eq!(s1.recv_count, 1);

        // 2. Alice sends second message (type 1 WhisperMessage)
        let second = alice
            .encrypt("bob", None, b"hello second")
            .expect("encrypt second");
        let out2 = bob.decrypt("alice", &second).expect("bob decrypts second");
        assert_eq!(out2.text, "hello second");

        let s2 = bob.load_session("alice").unwrap().unwrap();
        assert_eq!(s2.recv_count, 2);

        // 3. Replay attack: replay `first` (type 3 message) to Bob
        let replay_res = bob.decrypt("alice", &first);
        assert!(
            replay_res.is_err(),
            "Replaying type 3 message must be rejected"
        );

        // 4. Invariant: Bob's active session MUST NOT be rolled back to message 1
        let s_after = bob.load_session("alice").unwrap().unwrap();
        assert_eq!(
            s_after.recv_count, 2,
            "Session recv_count must remain 2, not rolled back to 1"
        );

        // 5. Subsequent legitimate message from Alice must continue to decrypt successfully
        let third = alice
            .encrypt("bob", None, b"hello third")
            .expect("encrypt third");
        let out3 = bob
            .decrypt("alice", &third)
            .expect("bob decrypts third after replay attempt");
        assert_eq!(out3.text, "hello third");

        let s3 = bob.load_session("alice").unwrap().unwrap();
        assert_eq!(s3.recv_count, 3);
    }

    #[test]
    fn peer_identity_change_resets_verified_and_signals_identity_changed_a11() {
        let alice = manager("alice_id1");
        let bob = manager("bob_id_test");

        let bundle = peer_bundle(&bob);
        let first = alice
            .start_session("bob", &bundle, b"msg from alice original")
            .unwrap();
        let out1 = bob.decrypt("alice", &first).unwrap();
        assert_eq!(out1.text, "msg from alice original");
        assert!(!out1.identity_changed, "TOFU is not identity changed");

        // Bob marks Alice as verified
        bob.verify_identity("alice").unwrap();
        assert!(
            bob.is_identity_verified("alice").unwrap(),
            "Alice should be verified"
        );

        // Now Alice reinstalls the app (creates a new identity key pair)
        let alice_new = manager("alice_id2_new");
        // Alice new gets a fresh bundle from Bob
        let bundle2 = peer_bundle(&bob);
        let first_new = alice_new
            .start_session("bob", &bundle2, b"msg from alice new device")
            .unwrap();

        // Bob receives message from Alice's new device
        let out2 = bob.decrypt("alice", &first_new).unwrap();
        assert_eq!(out2.text, "msg from alice new device");
        assert!(
            out2.identity_changed,
            "Must report identity_changed = true when identity key changes"
        );

        // Crucial A11 invariant: verified flag MUST be reset to false!
        assert!(
            !bob.is_identity_verified("alice").unwrap(),
            "Verified flag must be reset to false upon identity change"
        );
    }

    #[test]
    fn corrupt_identity_does_not_regenerate_key_a14() {
        let m = manager("corrupt_guard");
        let _orig_id = m.identity().unwrap();

        // Corrupt identity in DB
        let short_blob = vec![0xEEu8; 10]; // bad length
        let enc_blob = crate::storage::protect(&short_blob).unwrap();
        m.conn
            .execute(
                "UPDATE identity_key SET public_key=?1, private_bytes=?2 WHERE profile_id=?3",
                rusqlite::params![&short_blob, &enc_blob, m.profile_id()],
            )
            .unwrap();

        // Loading identity must return Err and must NOT regenerate
        assert!(m.identity().is_err(), "Must return Err on corrupt identity");

        // Verify the database was not overwritten with a new identity key
        let raw_pub: Vec<u8> = m
            .conn
            .query_row(
                "SELECT public_key FROM identity_key WHERE profile_id=?1",
                rusqlite::params![m.profile_id()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            raw_pub, short_blob,
            "Identity row must remain untouched without regeneration"
        );
    }

    #[test]
    fn signed_prekey_rotation_preserves_old_keys_and_decrypts_late_messages_a12() {
        let bob_mgr = manager("bob_spk_rot");
        let alice_mgr = manager("alice_spk_rot");

        // Initial SPK for Bob (id = 1)
        let bob_spk_v1 = bob_mgr.signed_prekey().unwrap();
        assert_eq!(bob_spk_v1.key_id, 1);

        // Alice fetches Bob's initial bundle (with SPK id = 1)
        let bob_bundle_v1 = peer_bundle(&bob_mgr);
        assert_eq!(bob_bundle_v1.signed_pre_key_id, 1);

        // Alice prepares a message to Bob using SPK id = 1
        let wire_msg1 = alice_mgr
            .start_session(bob_mgr.profile_id(), &bob_bundle_v1, b"hello with spk v1")
            .unwrap();

        // Simulate time advancing past 7 days for Bob
        bob_mgr
            .conn
            .execute(
                "UPDATE signed_prekey SET created_at = ?1 WHERE profile_id = ?2 AND key_id = 1",
                rusqlite::params![now_ms() - (8 * 86_400_000), bob_mgr.profile_id()],
            )
            .unwrap();

        // Bob rotates SPK -> should now have key_id = 2
        let bob_spk_v2 = bob_mgr.active_signed_prekey().unwrap();
        assert_eq!(bob_spk_v2.key_id, 2);
        assert_ne!(bob_spk_v2.key_pair.public, bob_spk_v1.key_pair.public);

        // Verify old SPK v1 is still present in Bob's store (grace retention)
        let stored_v1 = store::load_signed_prekey(bob_mgr.conn(), bob_mgr.profile_id(), 1).unwrap();
        assert!(
            stored_v1.is_some(),
            "Old signed prekey must be retained in store"
        );

        // Bob receives Alice's message which was encrypted with SPK v1!
        // Bob must be able to decrypt it using the retained SPK v1
        let decrypted = bob_mgr.decrypt(alice_mgr.profile_id(), &wire_msg1).unwrap();
        assert_eq!(decrypted.text, "hello with spk v1");
    }
    #[test]
    fn prekey_delivery_survives_loss_reordering_and_rejects_replay() {
        let alice = manager("delivery-alice");
        let bob = manager("delivery-bob");
        let remote = peer_bundle(&bob);
        let first = alice
            .start_session(bob.profile_id(), &remote, b"first")
            .unwrap();
        let second = alice.encrypt(bob.profile_id(), None, b"second").unwrap();
        assert!(matches!(
            deserialize(&second).unwrap(),
            DecodedMessage::PreKey(_)
        ));
        assert_eq!(
            bob.decrypt(alice.profile_id(), &second).unwrap().text,
            "second"
        );
        assert_eq!(
            bob.decrypt(alice.profile_id(), &first).unwrap().text,
            "first"
        );
        assert!(bob.decrypt(alice.profile_id(), &first).is_err());
        let reply = bob.encrypt(alice.profile_id(), None, b"reply").unwrap();
        alice.decrypt(bob.profile_id(), &reply).unwrap();
        let next = alice.encrypt(bob.profile_id(), None, b"third").unwrap();
        assert!(matches!(
            deserialize(&next).unwrap(),
            DecodedMessage::Whisper(_)
        ));
        assert_eq!(
            bob.decrypt(alice.profile_id(), &next).unwrap().text,
            "third"
        );
    }

    #[test]
    fn session_write_failure_rolls_back_consumed_key_and_replay_marker() {
        let alice = manager("atomic-alice");
        let bob = manager("atomic-bob");
        let remote = peer_bundle(&bob);
        let wire = alice
            .start_session(bob.profile_id(), &remote, b"recoverable")
            .unwrap();
        bob.conn.execute_batch("CREATE TRIGGER fail_session BEFORE INSERT ON signal_session BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
        assert!(bob.decrypt(alice.profile_id(), &wire).is_err());
        assert!(bob.load_session(alice.profile_id()).unwrap().is_none());
        assert!(store::get_one_time_prekey(
            bob.conn(),
            bob.profile_id(),
            remote.one_time_pre_key.unwrap().1
        )
        .unwrap()
        .is_some());
        bob.conn.execute_batch("DROP TRIGGER fail_session").unwrap();
        assert_eq!(
            bob.decrypt(alice.profile_id(), &wire).unwrap().text,
            "recoverable"
        );
    }

    #[test]
    fn verification_requires_the_displayed_real_identity() {
        let manager = manager("verification");
        assert!(manager.verify_identity("missing").is_err());
        assert!(manager.get_trusted_identity("missing").unwrap().is_none());
        manager.trust_identity("peer", &[7; 32]).unwrap();
        let displayed = manager.safety_number(&[7; 32]).unwrap();
        manager.trust_identity("peer", &[8; 32]).unwrap();
        assert!(manager.verify_identity_number("peer", &displayed).is_err());
        assert!(!manager.is_identity_verified("peer").unwrap());
        manager
            .verify_identity_number("peer", &manager.safety_number(&[8; 32]).unwrap())
            .unwrap();
        assert!(manager.is_identity_verified("peer").unwrap());
    }

    #[test]
    fn refilled_pool_never_reuses_consumed_prekey_ids() {
        let manager = manager("prekey-sequence");
        let first = manager.refill_one_time_prekeys().unwrap();
        let highest = first.iter().map(|key| key.key_id).max().unwrap();
        manager
            .conn
            .execute(
                "DELETE FROM one_time_prekey WHERE profile_id=?1",
                [manager.profile_id()],
            )
            .unwrap();
        let bundle = manager.local_bundle().unwrap();
        assert!(bundle.pre_keys.iter().all(|key| key.key_id > highest));
        manager
            .conn
            .execute(
                "DELETE FROM one_time_prekey WHERE profile_id=?1",
                [manager.profile_id()],
            )
            .unwrap();
        manager
            .conn
            .execute(
                "UPDATE prekey_sequence SET last_id=?1 WHERE profile_id=?2",
                rusqlite::params![u32::MAX - 1, manager.profile_id()],
            )
            .unwrap();
        assert!(manager.refill_one_time_prekeys().is_err());
    }
    #[test]
    fn corrupt_ratchet_never_authorizes_plaintext_and_policy_survives_absence() {
        let manager = manager("fail-closed-policy");
        manager
            .conn
            .execute(
                "INSERT INTO e2ee_required VALUES(?1,'peer')",
                [manager.profile_id()],
            )
            .unwrap();
        assert!(manager.requires_e2ee("peer").unwrap());
        manager.conn.execute("INSERT INTO signal_session(profile_id,peer_id,device_id,session_record,updated_at) VALUES(?1,'peer',1,x'010203',0)",[manager.profile_id()]).unwrap();
        assert!(manager.requires_e2ee("peer").is_err());
    }
    #[test]
    fn consuming_last_prekey_refills_pool_in_the_same_transaction() {
        let alice = manager("refill-alice");
        let bob = manager("refill-bob");
        let remote = peer_bundle(&bob);
        let key_id = remote.one_time_pre_key.unwrap().1;
        bob.conn
            .execute(
                "DELETE FROM one_time_prekey WHERE profile_id=?1 AND key_id<>?2",
                rusqlite::params![bob.profile_id(), key_id],
            )
            .unwrap();
        let packet = alice
            .start_session(bob.profile_id(), &remote, b"refill")
            .unwrap();
        assert_eq!(
            bob.decrypt(alice.profile_id(), &packet).unwrap().text,
            "refill"
        );
        assert!(bob.one_time_prekey_count().unwrap() >= PREKEY_REFILL_THRESHOLD);
        assert!(bob
            .list_one_time_prekeys()
            .unwrap()
            .iter()
            .all(|key| key.key_id > key_id));
    }
}
