//! SQLite-backed Signal Protocol key store (WP16-1/2).
//!
//! Persists identity keys, signed prekeys, one-time prekeys, session records,
//! and trusted identity keys.  All private key bytes are DPAPI-encrypted on
//! Windows before being written to the database.
//!
//! Schema migration is appended to the main `MessageStore` migrations — this
//! module only performs reads/writes; the schema is owned by `src/store.rs`
//! migration v12+.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

use crate::storage;
use super::keystore::{IdentityKeyPair, OneTimePreKey, SignedPreKey};
use super::ratchet::{KeyPairBytes, RatchetSession};

/// Open a dedicated crypto store connection (separate from message DB to keep
/// crypto data isolated).  The file lives at `<profile_root>/e2ee.db`.
pub fn open_crypto_db() -> Result<Connection> {
    let path = storage::root().join("e2ee.db");
    storage::private_dir(storage::root())?;
    open_crypto_db_at(&path)
}

/// Open a crypto store at an explicit path (used by tests and tooling).
/// Current schema version of the SQLite E2EE key store (A40).
pub const CURRENT_SCHEMA_VERSION: i32 = 4;

pub fn open_crypto_db_at(path: &std::path::Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        storage::private_dir(parent)?;
    }
    let mut conn = Connection::open(path)
        .with_context(|| format!("Không mở được E2EE database: {}", path.display()))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version > CURRENT_SCHEMA_VERSION {
        anyhow::bail!(
            "Phiên bản cơ sở dữ liệu E2EE ({version}) mới hơn phiên bản ứng dụng hỗ trợ ({CURRENT_SCHEMA_VERSION}). Vui lòng nâng cấp ứng dụng."
        );
    }
    if version == CURRENT_SCHEMA_VERSION {
        return Ok(());
    }

    // Atomic transaction guard for all migration steps (A40)
    let tx = conn.transaction()?;
    if version < 1 {
        tx.execute_batch("
            CREATE TABLE IF NOT EXISTS identity_key (
                profile_id TEXT NOT NULL PRIMARY KEY,
                public_key BLOB NOT NULL,
                private_bytes BLOB NOT NULL,
                registration_id INTEGER NOT NULL,
                signing_public BLOB NOT NULL,
                signing_private_enc BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS signed_prekey (
                profile_id TEXT NOT NULL,
                key_id INTEGER NOT NULL,
                public_key BLOB NOT NULL,
                private_bytes_enc BLOB NOT NULL,
                signature BLOB NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY (profile_id, key_id)
            );
            CREATE TABLE IF NOT EXISTS one_time_prekey (
                profile_id TEXT NOT NULL,
                key_id INTEGER NOT NULL,
                public_key BLOB NOT NULL,
                private_bytes_enc BLOB NOT NULL,
                PRIMARY KEY (profile_id, key_id)
            );
            CREATE TABLE IF NOT EXISTS signal_session (
                profile_id TEXT NOT NULL,
                peer_id TEXT NOT NULL,
                device_id INTEGER NOT NULL DEFAULT 1,
                session_record BLOB NOT NULL,
                session_ts INTEGER,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (profile_id, peer_id, device_id)
            );
            CREATE TABLE IF NOT EXISTS trusted_identity (
                profile_id TEXT NOT NULL,
                peer_id TEXT NOT NULL,
                identity_key BLOB NOT NULL,
                verified INTEGER NOT NULL DEFAULT 0,
                verified_at INTEGER,
                PRIMARY KEY (profile_id, peer_id)
            );
        ")?;
    }
    if version < 2 {
        let has_col: bool = tx
            .prepare("SELECT verified FROM trusted_identity LIMIT 0")
            .is_ok();
        if !has_col {
            let _ = tx.execute_batch("
                ALTER TABLE trusted_identity ADD COLUMN verified INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE trusted_identity ADD COLUMN verified_at INTEGER;
            ");
        }
    }
    if version < 3 {
        let has_table: bool = tx
            .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='seen_base_key'")
            .and_then(|mut s| s.exists([]))
            .unwrap_or(false);
        if !has_table {
            tx.execute_batch("
                CREATE TABLE IF NOT EXISTS seen_base_key (
                    profile_id TEXT NOT NULL,
                    peer_id TEXT NOT NULL,
                    base_key BLOB NOT NULL,
                    created_at INTEGER NOT NULL,
                    PRIMARY KEY (profile_id, peer_id, base_key)
                );
            ")?;
        }
    }
    if version < 4 {
        tx.execute_batch("
            CREATE INDEX IF NOT EXISTS idx_signed_prekey_created
            ON signed_prekey (profile_id, created_at);
        ")?;
    }
    tx.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

// --- Identity key ---

pub fn save_identity(conn: &Connection, profile_id: &str, identity: &IdentityKeyPair) -> Result<()> {
    let private_enc = storage::protect(&identity.dh.private)?;
    let signing_enc = storage::protect(&identity.signing_private)?;
    conn.execute(
        "INSERT OR REPLACE INTO identity_key
         (profile_id, public_key, private_bytes, registration_id, signing_public, signing_private_enc)
         VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            profile_id,
            &identity.dh.public[..],
            private_enc,
            identity.registration_id,
            &identity.signing_public[..],
            signing_enc,
        ],
    )?;
    Ok(())
}

pub fn load_identity(conn: &Connection, profile_id: &str) -> Result<Option<IdentityKeyPair>> {
    let row = conn.query_row(
        "SELECT public_key, private_bytes, registration_id, signing_public, signing_private_enc
         FROM identity_key WHERE profile_id=?1",
        params![profile_id],
        |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, Vec<u8>>(1)?,
                r.get::<_, u32>(2)?,
                r.get::<_, Vec<u8>>(3)?,
                r.get::<_, Vec<u8>>(4)?,
            ))
        },
    );
    match row {
        Ok((pub_bytes, priv_enc, reg_id, sign_pub, sign_priv_enc)) => {
            let priv_dec = storage::unprotect(&priv_enc)
                .map_err(|e| anyhow::anyhow!("Dữ liệu khóa riêng bị hỏng: {e}"))?;
            let sign_priv_dec = storage::unprotect(&sign_priv_enc)
                .map_err(|e| anyhow::anyhow!("Dữ liệu khóa ký riêng bị hỏng: {e}"))?;
            if pub_bytes.len() != 32 || priv_dec.len() != 32 || sign_pub.len() != 32 || sign_priv_dec.len() != 32 {
                anyhow::bail!("Dữ liệu khóa danh tính E2EE bị hỏng (corrupted): độ dài không hợp lệ");
            }
            let mut pub_arr = [0u8; 32];
            let mut priv_arr = [0u8; 32];
            let mut sign_pub_arr = [0u8; 32];
            let mut sign_priv_arr = [0u8; 32];
            pub_arr.copy_from_slice(&pub_bytes);
            priv_arr.copy_from_slice(&priv_dec);
            sign_pub_arr.copy_from_slice(&sign_pub);
            sign_priv_arr.copy_from_slice(&sign_priv_dec);
            Ok(Some(IdentityKeyPair {
                dh: KeyPairBytes { public: pub_arr, private: priv_arr },
                signing_public: sign_pub_arr,
                signing_private: sign_priv_arr,
                registration_id: reg_id,
            }))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

// --- Signed prekey ---

pub fn save_signed_prekey(conn: &Connection, profile_id: &str, spk: &SignedPreKey) -> Result<()> {
    let priv_enc = storage::protect(&spk.key_pair.private)?;
    conn.execute(
        "INSERT OR REPLACE INTO signed_prekey
         (profile_id, key_id, public_key, private_bytes_enc, signature, created_at)
         VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            profile_id,
            spk.key_id,
            &spk.key_pair.public[..],
            priv_enc,
            &spk.signature[..],
            spk.created_at,
        ],
    )?;
    Ok(())
}

pub fn load_signed_prekey(conn: &Connection, profile_id: &str, key_id: u32) -> Result<Option<SignedPreKey>> {
    let row = conn.query_row(
        "SELECT public_key, private_bytes_enc, signature, created_at
         FROM signed_prekey WHERE profile_id=?1 AND key_id=?2",
        params![profile_id, key_id],
        |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Vec<u8>>(1)?, r.get::<_, Vec<u8>>(2)?, r.get::<_, i64>(3)?)),
    );
    match row {
        Ok((pub_bytes, priv_enc, sig_bytes, created_at)) => {
            let priv_dec = storage::unprotect(&priv_enc)
                .map_err(|e| anyhow::anyhow!("Dữ liệu khóa riêng của signed prekey bị hỏng: {e}"))?;
            if pub_bytes.len() != 32 || priv_dec.len() != 32 || sig_bytes.len() != 64 {
                anyhow::bail!("Dữ liệu signed prekey bị hỏng (corrupted): độ dài không hợp lệ");
            }
            let mut pub_arr = [0u8; 32]; pub_arr.copy_from_slice(&pub_bytes);
            let mut priv_arr = [0u8; 32]; priv_arr.copy_from_slice(&priv_dec);
            let mut sig_arr = [0u8; 64]; sig_arr.copy_from_slice(&sig_bytes);
            Ok(Some(SignedPreKey {
                key_id,
                key_pair: KeyPairBytes { public: pub_arr, private: priv_arr },
                signature: sig_arr.to_vec(),
                created_at,
            }))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Load the most recently created signed prekey (active signed prekey) for a profile (A12).
pub fn load_latest_signed_prekey(conn: &Connection, profile_id: &str) -> Result<Option<SignedPreKey>> {
    let row = conn.query_row(
        "SELECT key_id, public_key, private_bytes_enc, signature, created_at
         FROM signed_prekey WHERE profile_id=?1 ORDER BY key_id DESC LIMIT 1",
        params![profile_id],
        |r| {
            Ok((
                r.get::<_, u32>(0)?,
                r.get::<_, Vec<u8>>(1)?,
                r.get::<_, Vec<u8>>(2)?,
                r.get::<_, Vec<u8>>(3)?,
                r.get::<_, i64>(4)?,
            ))
        },
    );
    match row {
        Ok((key_id, pub_bytes, priv_enc, sig_bytes, created_at)) => {
            let priv_dec = storage::unprotect(&priv_enc)
                .map_err(|e| anyhow::anyhow!("Dữ liệu khóa riêng của signed prekey bị hỏng: {e}"))?;
            if pub_bytes.len() != 32 || priv_dec.len() != 32 || sig_bytes.len() != 64 {
                anyhow::bail!("Dữ liệu signed prekey bị hỏng (corrupted): độ dài không hợp lệ");
            }
            let mut pub_arr = [0u8; 32]; pub_arr.copy_from_slice(&pub_bytes);
            let mut priv_arr = [0u8; 32]; priv_arr.copy_from_slice(&priv_dec);
            let mut sig_arr = [0u8; 64]; sig_arr.copy_from_slice(&sig_bytes);
            Ok(Some(SignedPreKey {
                key_id,
                key_pair: KeyPairBytes { public: pub_arr, private: priv_arr },
                signature: sig_arr.to_vec(),
                created_at,
            }))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Prune signed prekeys older than `retention_ms` while retaining the active key (A12 grace retention).
pub fn prune_old_signed_prekeys(conn: &Connection, profile_id: &str, retention_ms: i64) -> Result<usize> {
    let cutoff = now_ms().saturating_sub(retention_ms);
    let deleted = conn.execute(
        "DELETE FROM signed_prekey
         WHERE profile_id=?1
           AND created_at < ?2
           AND key_id NOT IN (
               SELECT key_id FROM signed_prekey WHERE profile_id=?1 ORDER BY key_id DESC LIMIT 1
           )",
        params![profile_id, cutoff],
    )?;
    Ok(deleted)
}

// --- One-time prekeys ---

pub fn save_one_time_prekeys(conn: &Connection, profile_id: &str, keys: &[OneTimePreKey]) -> Result<()> {
    for k in keys {
        let priv_enc = storage::protect(&k.key_pair.private)?;
        conn.execute(
            "INSERT OR REPLACE INTO one_time_prekey
             (profile_id, key_id, public_key, private_bytes_enc)
             VALUES (?1,?2,?3,?4)",
            params![profile_id, k.key_id, &k.key_pair.public[..], priv_enc],
        )?;
    }
    Ok(())
}

pub fn get_one_time_prekey(conn: &Connection, profile_id: &str, key_id: u32) -> Result<Option<OneTimePreKey>> {
    let row = conn.query_row(
        "SELECT public_key, private_bytes_enc FROM one_time_prekey
         WHERE profile_id=?1 AND key_id=?2",
        params![profile_id, key_id],
        |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Vec<u8>>(1)?)),
    );
    match row {
        Ok((pub_bytes, priv_enc)) => {
            let priv_dec = storage::unprotect(&priv_enc)
                .map_err(|e| anyhow::anyhow!("Dữ liệu khóa riêng one-time prekey bị hỏng: {e}"))?;
            if pub_bytes.len() != 32 || priv_dec.len() != 32 {
                anyhow::bail!("One-time prekey trong cơ sở dữ liệu bị hỏng (corrupted): độ dài không hợp lệ");
            }
            let mut pub_arr = [0u8; 32]; pub_arr.copy_from_slice(&pub_bytes);
            let mut priv_arr = [0u8; 32]; priv_arr.copy_from_slice(&priv_dec);
            Ok(Some(OneTimePreKey {
                key_id,
                key_pair: KeyPairBytes { public: pub_arr, private: priv_arr },
            }))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete_one_time_prekey(conn: &Connection, profile_id: &str, key_id: u32) -> Result<()> {
    conn.execute(
        "DELETE FROM one_time_prekey WHERE profile_id=?1 AND key_id=?2",
        params![profile_id, key_id],
    )?;
    Ok(())
}

pub fn consume_one_time_prekey(conn: &Connection, profile_id: &str, key_id: u32) -> Result<Option<OneTimePreKey>> {
    let key = get_one_time_prekey(conn, profile_id, key_id)?;
    if key.is_some() {
        delete_one_time_prekey(conn, profile_id, key_id)?;
    }
    Ok(key)
}

pub fn count_one_time_prekeys(conn: &Connection, profile_id: &str) -> Result<u32> {
    let n: u32 = conn.query_row(
        "SELECT COUNT(*) FROM one_time_prekey WHERE profile_id=?1",
        params![profile_id],
        |r| r.get(0),
    )?;
    Ok(n)
}

// --- Session records ---

pub fn save_session(
    conn: &Connection,
    profile_id: &str,
    peer_id: &str,
    session: &RatchetSession,
) -> Result<()> {
    let record = serde_json::to_vec(session)?;
    let record_enc = storage::protect(&record)?;
    let now = now_ms();
    let session_ts = session.session_ts;
    conn.execute(
        "INSERT OR REPLACE INTO signal_session
         (profile_id, peer_id, device_id, session_record, session_ts, updated_at)
         VALUES (?1,?2,1,?3,?4,?5)",
        params![profile_id, peer_id, record_enc, session_ts, now],
    )?;
    Ok(())
}

pub fn load_session(
    conn: &Connection,
    profile_id: &str,
    peer_id: &str,
) -> Result<Option<RatchetSession>> {
    let row = conn.query_row(
        "SELECT session_record FROM signal_session
         WHERE profile_id=?1 AND peer_id=?2 AND device_id=1",
        params![profile_id, peer_id],
        |r| r.get::<_, Vec<u8>>(0),
    );
    match row {
        Ok(record_enc) => {
            let record = storage::unprotect(&record_enc)
                .map_err(|e| anyhow::anyhow!("Dữ liệu phiên ratchet bị hỏng: {e}"))?;
            let session: RatchetSession = serde_json::from_slice(&record)
                .map_err(|e| anyhow::anyhow!("Dữ liệu phiên ratchet không giải mã được JSON: {e}"))?;
            Ok(Some(session))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete_session(conn: &Connection, profile_id: &str, peer_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM signal_session WHERE profile_id=?1 AND peer_id=?2 AND device_id=1",
        params![profile_id, peer_id],
    )?;
    Ok(())
}

// --- Trusted identities (TOFU) ---

pub fn save_trusted_identity(
    conn: &Connection,
    profile_id: &str,
    peer_id: &str,
    identity_key: &[u8; 32],
) -> Result<bool> {
    // Returns true if the key changed (device reset — show UI warning).
    // The very first sighting is not a change: trust-on-first-use.
    let existing: Option<(Vec<u8>, i32)> = conn
        .query_row(
            "SELECT identity_key, verified FROM trusted_identity WHERE profile_id=?1 AND peer_id=?2",
            params![profile_id, peer_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match existing {
        None => {
            conn.execute(
                "INSERT INTO trusted_identity (profile_id, peer_id, identity_key, verified) VALUES (?1,?2,?3,0)",
                params![profile_id, peer_id, &identity_key[..]],
            )?;
            Ok(false)
        }
        Some((ref k, _verified)) => {
            if k.as_slice() == identity_key.as_slice() {
                // Key unchanged: preserve verified status
                Ok(false)
            } else {
                // Key changed! Reset verified=0 and verified_at=NULL (A11 requirement)
                conn.execute(
                    "UPDATE trusted_identity SET identity_key=?1, verified=0, verified_at=NULL WHERE profile_id=?2 AND peer_id=?3",
                    params![&identity_key[..], profile_id, peer_id],
                )?;
                Ok(true)
            }
        }
    }
}

pub fn is_trusted_identity(
    conn: &Connection,
    profile_id: &str,
    peer_id: &str,
    identity_key: &[u8; 32],
) -> Result<bool> {
    let stored: Option<Vec<u8>> = conn
        .query_row(
            "SELECT identity_key FROM trusted_identity WHERE profile_id=?1 AND peer_id=?2",
            params![profile_id, peer_id],
            |r| r.get(0),
        )
        .optional()?;
    match stored {
        None => Ok(true), // TOFU: trust on first use
        Some(ref k) => Ok(k.as_slice() == identity_key.as_slice()),
    }
}

pub fn get_trusted_identity(
    conn: &Connection,
    profile_id: &str,
    peer_id: &str,
) -> Result<Option<([u8; 32], bool)>> {
    let row: Option<(Vec<u8>, i32)> = conn
        .query_row(
            "SELECT identity_key, COALESCE(verified, 0) FROM trusted_identity WHERE profile_id=?1 AND peer_id=?2",
            params![profile_id, peer_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((bytes, verified)) = row {
        if bytes.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            return Ok(Some((key, verified != 0)));
        }
    }
    Ok(None)
}

pub fn set_identity_verified(
    conn: &Connection,
    profile_id: &str,
    peer_id: &str,
    verified: bool,
) -> Result<()> {
    let now = now_ms();
    let val = if verified { 1 } else { 0 };
    let rows = conn.execute(
        "UPDATE trusted_identity SET verified=?1, verified_at=?2 WHERE profile_id=?3 AND peer_id=?4",
        params![val, now, profile_id, peer_id],
    )?;
    if rows == 0 {
        let mut placeholder = [0u8; 32];
        let digest = Sha256::digest(peer_id.as_bytes());
        placeholder.copy_from_slice(&digest);
        conn.execute(
            "INSERT OR REPLACE INTO trusted_identity (profile_id, peer_id, identity_key, verified, verified_at) VALUES (?1,?2,?3,?4,?5)",
            params![profile_id, peer_id, &placeholder[..], val, now],
        )?;
    }
    Ok(())
}

pub fn is_identity_verified(
    conn: &Connection,
    profile_id: &str,
    peer_id: &str,
) -> Result<bool> {
    let verified: Option<i32> = conn
        .query_row(
            "SELECT verified FROM trusted_identity WHERE profile_id=?1 AND peer_id=?2",
            params![profile_id, peer_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(verified.unwrap_or(0) != 0)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}
impl<T> OptionalExt<T> for std::result::Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// --- Seen base keys (Replay attack & session rollback protection - A11) ---

pub fn record_base_key(
    conn: &Connection,
    profile_id: &str,
    peer_id: &str,
    base_key: &[u8; 32],
) -> Result<()> {
    let now = now_ms();
    conn.execute(
        "INSERT OR IGNORE INTO seen_base_key (profile_id, peer_id, base_key, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![profile_id, peer_id, &base_key[..], now],
    )?;
    Ok(())
}

pub fn has_base_key(
    conn: &Connection,
    profile_id: &str,
    peer_id: &str,
    base_key: &[u8; 32],
) -> Result<bool> {
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM seen_base_key WHERE profile_id=?1 AND peer_id=?2 AND base_key=?3",
            params![profile_id, peer_id, &base_key[..]],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    Ok(exists)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_identity_verification_lifecycle() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("test_e2ee.db");
        let conn = open_crypto_db_at(&db_path).unwrap();

        let profile_id = "test_user_001";
        let peer_id = "test_peer_002";
        let key = [42u8; 32];

        // 1. Initial save of peer identity (TOFU)
        let changed = save_trusted_identity(&conn, profile_id, peer_id, &key).unwrap();
        assert!(!changed);

        // 2. Not yet verified
        assert!(!is_identity_verified(&conn, profile_id, peer_id).unwrap());
        let (stored_key, verified) = get_trusted_identity(&conn, profile_id, peer_id).unwrap().unwrap();
        assert_eq!(stored_key, key);
        assert!(!verified);

        // 3. Mark verified
        set_identity_verified(&conn, profile_id, peer_id, true).unwrap();
        assert!(is_identity_verified(&conn, profile_id, peer_id).unwrap());
        let (stored_key2, verified2) = get_trusted_identity(&conn, profile_id, peer_id).unwrap().unwrap();
        assert_eq!(stored_key2, key);
        assert!(verified2);
    }
    #[test]
    fn identity_change_resets_verified_flag_a11() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("test_e2ee_a11.db");
        let conn = open_crypto_db_at(&db_path).unwrap();

        let profile_id = "user_a";
        let peer_id = "user_b";
        let initial_key = [11u8; 32];
        let changed_key = [22u8; 32];

        // 1. Initial save of peer identity (TOFU)
        let changed = save_trusted_identity(&conn, profile_id, peer_id, &initial_key).unwrap();
        assert!(!changed, "TOFU must not be reported as identity change");
        assert!(!is_identity_verified(&conn, profile_id, peer_id).unwrap());

        // 2. Mark identity verified
        set_identity_verified(&conn, profile_id, peer_id, true).unwrap();
        assert!(is_identity_verified(&conn, profile_id, peer_id).unwrap());

        // 3. Saving the same identity preserves verified flag
        let changed_same = save_trusted_identity(&conn, profile_id, peer_id, &initial_key).unwrap();
        assert!(!changed_same, "Same key must return changed = false");
        assert!(is_identity_verified(&conn, profile_id, peer_id).unwrap(), "Verified flag must stay true");

        // 4. Changing identity MUST reset verified to 0 and report identity_changed = true (A11)
        let changed_new = save_trusted_identity(&conn, profile_id, peer_id, &changed_key).unwrap();
        assert!(changed_new, "Different key must return changed = true");
        assert!(!is_identity_verified(&conn, profile_id, peer_id).unwrap(), "Verified flag must be reset to false");

        let (stored_key, is_ver) = get_trusted_identity(&conn, profile_id, peer_id).unwrap().unwrap();
        assert_eq!(stored_key, changed_key);
        assert!(!is_ver);
    }

    #[test]
    fn seen_base_key_prevents_replay_a11() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("test_e2ee_base_key.db");
        let conn = open_crypto_db_at(&db_path).unwrap();

        let profile_id = "user_a";
        let peer_id = "user_b";
        let base_key1 = [101u8; 32];
        let base_key2 = [102u8; 32];

        assert!(!has_base_key(&conn, profile_id, peer_id, &base_key1).unwrap());
        record_base_key(&conn, profile_id, peer_id, &base_key1).unwrap();
        assert!(has_base_key(&conn, profile_id, peer_id, &base_key1).unwrap());

        // Different base key is not marked
        assert!(!has_base_key(&conn, profile_id, peer_id, &base_key2).unwrap());
    }

    #[test]
    fn schema_version_guard_rejects_future_version_a40() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("future_e2ee.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch("PRAGMA user_version = 99;").unwrap();
        }
        let err = open_crypto_db_at(&db_path).err().unwrap();
        let msg = err.to_string();
        assert!(msg.contains("99") && msg.contains("mới hơn"), "Error message must report future version: {msg}");
    }

    #[test]
    fn store_corruption_fails_safely_without_panic_a14() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("corrupt_e2ee.db");
        let conn = open_crypto_db_at(&db_path).unwrap();
        let profile_id = "victim_user";

        // Insert corrupt / short blob into identity_key
        let short_blob = vec![1u8, 2, 3]; // length < 32!
        let enc_blob = storage::protect(&short_blob).unwrap();
        conn.execute(
            "INSERT INTO identity_key (profile_id, public_key, private_bytes, registration_id, signing_public, signing_private_enc)
             VALUES (?1, ?2, ?3, 100, ?4, ?5)",
            params![profile_id, &short_blob, &enc_blob, &short_blob, &enc_blob],
        ).unwrap();

        // Must return Err instead of panicking on slice bounds
        let load_res = load_identity(&conn, profile_id);
        assert!(load_res.is_err(), "Must return Err on corrupt identity blob");

        // Insert corrupt short blob into signed_prekey
        conn.execute(
            "INSERT INTO signed_prekey (profile_id, key_id, public_key, private_bytes_enc, signature, created_at)
             VALUES (?1, 1, ?2, ?3, ?4, 1000)",
            params![profile_id, &short_blob, &enc_blob, &short_blob],
        ).unwrap();
        assert!(load_signed_prekey(&conn, profile_id, 1).is_err());
        assert!(load_latest_signed_prekey(&conn, profile_id).is_err());
    }

    #[test]
    fn signed_prekey_pruning_retains_latest_spk_a12() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("prune_spk.db");
        let conn = open_crypto_db_at(&db_path).unwrap();
        let profile_id = "prune_user";

        let now = now_ms();
        // Insert SPK 1 (very old: 60 days ago)
        let dummy_spk1 = SignedPreKey {
            key_id: 1,
            key_pair: KeyPairBytes { public: [1u8; 32], private: [2u8; 32] },
            signature: vec![3u8; 64],
            created_at: now - (60 * 86_400_000),
        };
        save_signed_prekey(&conn, profile_id, &dummy_spk1).unwrap();

        // Insert SPK 2 (active, recent)
        let dummy_spk2 = SignedPreKey {
            key_id: 2,
            key_pair: KeyPairBytes { public: [4u8; 32], private: [5u8; 32] },
            signature: vec![6u8; 64],
            created_at: now,
        };
        save_signed_prekey(&conn, profile_id, &dummy_spk2).unwrap();

        // Pruning older than 30 days should delete SPK 1 but preserve SPK 2
        let deleted = prune_old_signed_prekeys(&conn, profile_id, 30 * 86_400_000).unwrap();
        assert_eq!(deleted, 1);
        assert!(load_signed_prekey(&conn, profile_id, 1).unwrap().is_none());
        assert!(load_signed_prekey(&conn, profile_id, 2).unwrap().is_some());

        // Pruning again when only SPK 2 exists (even if we pretend retention is 0) must retain SPK 2
        let deleted2 = prune_old_signed_prekeys(&conn, profile_id, 0).unwrap();
        assert_eq!(deleted2, 0, "Active SPK must never be pruned");
        assert!(load_latest_signed_prekey(&conn, profile_id).unwrap().is_some());
    }
}
