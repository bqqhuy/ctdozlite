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
pub fn open_crypto_db_at(path: &std::path::Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        storage::private_dir(parent)?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("Không mở được E2EE database: {}", path.display()))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch("
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
                PRIMARY KEY (profile_id, peer_id)
            );
            PRAGMA user_version=1;
        ")?;
    }
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
            let priv_dec = storage::unprotect(&priv_enc)?;
            let sign_priv_dec = storage::unprotect(&sign_priv_enc)?;
            let mut pub_arr = [0u8; 32];
            let mut priv_arr = [0u8; 32];
            let mut sign_pub_arr = [0u8; 32];
            let mut sign_priv_arr = [0u8; 32];
            pub_arr.copy_from_slice(&pub_bytes[..32]);
            priv_arr.copy_from_slice(&priv_dec[..32]);
            sign_pub_arr.copy_from_slice(&sign_pub[..32]);
            sign_priv_arr.copy_from_slice(&sign_priv_dec[..32]);
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
            let priv_dec = storage::unprotect(&priv_enc)?;
            let mut pub_arr = [0u8; 32]; pub_arr.copy_from_slice(&pub_bytes[..32]);
            let mut priv_arr = [0u8; 32]; priv_arr.copy_from_slice(&priv_dec[..32]);
            let mut sig_arr = [0u8; 64]; sig_arr.copy_from_slice(&sig_bytes[..64]);
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
            let priv_dec = storage::unprotect(&priv_enc)?;
            anyhow::ensure!(pub_bytes.len() >= 32 && priv_dec.len() >= 32, "One-time prekey in store is corrupt");
            let mut pub_arr = [0u8; 32]; pub_arr.copy_from_slice(&pub_bytes[..32]);
            let mut priv_arr = [0u8; 32]; priv_arr.copy_from_slice(&priv_dec[..32]);
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
            let record = storage::unprotect(&record_enc)?;
            let session: RatchetSession = serde_json::from_slice(&record)?;
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
    let existing: Option<Vec<u8>> = conn
        .query_row(
            "SELECT identity_key FROM trusted_identity WHERE profile_id=?1 AND peer_id=?2",
            params![profile_id, peer_id],
            |r| r.get(0),
        )
        .optional()?;
    let changed = match existing {
        None => false,
        Some(ref k) => k.as_slice() != identity_key.as_slice(),
    };
    conn.execute(
        "INSERT OR REPLACE INTO trusted_identity (profile_id, peer_id, identity_key) VALUES (?1,?2,?3)",
        params![profile_id, peer_id, &identity_key[..]],
    )?;
    Ok(changed)
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
