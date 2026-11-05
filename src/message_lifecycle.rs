//! Server-defined message expiry, replay protection and bounded derived-index backfill.
use anyhow::Result;
use rusqlite::{params, Connection};
use serde_json::Value;
fn number(value: Option<&Value>) -> Option<i64> {
    value.and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
}
/// PC `8Nax` + `wlxX`: TTL and serverTime/sendDttm are milliseconds.
/// Missing time never falls back to this device's receipt time.
pub fn deadline(value: &Value, fallback: i64) -> Option<i64> {
    let sources = [
        Some(value),
        value.pointer("/message_state/wire_ref/fields"),
        value.pointer("/message_state/history_source"),
        value.pointer("/message_state/history_source/desktop_backup"),
    ];
    for source in sources.into_iter().flatten() {
        let Some(ttl) = number(source.get("ttl")).filter(|ttl| *ttl > 0) else {
            continue;
        };
        let base = number(source.get("serverTime"))
            .filter(|ts| *ts > 0)
            .or_else(|| number(source.get("sendDttm")).filter(|ts| *ts > 0))
            .or_else(|| number(source.get("ts")).filter(|ts| *ts > 0))
            .unwrap_or(fallback);
        if base <= 0 {
            return None;
        }
        return crate::clock::normalize_ts_ms(base).checked_add(ttl);
    }
    None
}
/// Resident messages use their retained wire metadata, with no JSON serialization per frame.
pub fn expires_at(message: &crate::chat::ChatMessage) -> Option<i64> {
    let fields = &message.wire_ref.fields;
    let end = number(fields.get("ttl"))
        .filter(|ttl| *ttl > 0)
        .and_then(|ttl| {
            let base = number(fields.get("serverTime"))
                .filter(|ts| *ts > 0)
                .or_else(|| number(fields.get("sendDttm")).filter(|ts| *ts > 0))
                .unwrap_or(message.ts);
            if base > 0 {
                crate::clock::normalize_ts_ms(base).checked_add(ttl)
            } else {
                None
            }
        });
    end.or_else(|| {
        message
            .history_source
            .as_ref()
            .and_then(|source| deadline(source, message.ts))
    })
}
/// Check a resident message against its original server deadline.
pub fn expired(message: &crate::chat::ChatMessage, now: i64) -> bool {
    expires_at(message).is_some_and(|deadline| deadline <= now)
}
/// Install the deterministic projection on every store connection before using its triggers.
pub(crate) fn register(conn: &Connection) -> Result<()> {
    conn.create_scalar_function(
        "ctdo_expiry",
        2,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
            | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let raw = ctx.get::<String>(0)?;
            let ts = ctx.get::<i64>(1)?;
            Ok(if raw.len() <= 4 * 1024 * 1024 {
                serde_json::from_str::<Value>(&raw)
                    .ok()
                    .and_then(|value| deadline(&value, ts))
                    .unwrap_or(0)
            } else {
                0
            })
        },
    )?;
    Ok(())
}
/// Schema 10 stores only derived deadlines; no message or checkpoint is discarded by migration.
pub(crate) fn migrate(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS message_expiry(thread_id TEXT NOT NULL,msg_id TEXT NOT NULL,expires_at INTEGER NOT NULL CHECK(expires_at>0),PRIMARY KEY(thread_id,msg_id),FOREIGN KEY(thread_id,msg_id) REFERENCES messages(thread_id,msg_id) ON DELETE CASCADE);
    CREATE INDEX IF NOT EXISTS message_expiry_due ON message_expiry(expires_at,thread_id,msg_id);
    CREATE TABLE IF NOT EXISTS expiry_index_state(id INTEGER PRIMARY KEY CHECK(id=1),last_rowid INTEGER NOT NULL,is_complete INTEGER NOT NULL);
    INSERT OR IGNORE INTO expiry_index_state VALUES(1,0,0);
    CREATE TRIGGER IF NOT EXISTS expiry_message_insert AFTER INSERT ON messages WHEN ctdo_expiry(COALESCE(NEW.raw_json,''),COALESCE(NEW.ts,0))>0 BEGIN
        INSERT INTO message_expiry VALUES(NEW.thread_id,NEW.msg_id,ctdo_expiry(COALESCE(NEW.raw_json,''),COALESCE(NEW.ts,0))) ON CONFLICT(thread_id,msg_id) DO UPDATE SET expires_at=excluded.expires_at; END;
    CREATE TRIGGER IF NOT EXISTS expiry_message_update AFTER UPDATE OF raw_json,ts ON messages WHEN ctdo_expiry(COALESCE(NEW.raw_json,''),COALESCE(NEW.ts,0))>0 BEGIN
        INSERT INTO message_expiry VALUES(NEW.thread_id,NEW.msg_id,ctdo_expiry(COALESCE(NEW.raw_json,''),COALESCE(NEW.ts,0))) ON CONFLICT(thread_id,msg_id) DO UPDATE SET expires_at=excluded.expires_at; END;
    CREATE TRIGGER IF NOT EXISTS reject_expired_replay BEFORE INSERT ON messages WHEN ctdo_expiry(COALESCE(NEW.raw_json,''),COALESCE(NEW.ts,0))>0 AND ctdo_expiry(COALESCE(NEW.raw_json,''),COALESCE(NEW.ts,0))<=CAST(strftime('%s','now') AS INTEGER)*1000 BEGIN
        INSERT OR IGNORE INTO deleted_messages(thread_id,msg_id) VALUES(NEW.thread_id,NEW.msg_id); SELECT RAISE(IGNORE); END;
    CREATE TABLE IF NOT EXISTS restored_assets(thread_id TEXT NOT NULL,msg_id TEXT NOT NULL,quality TEXT NOT NULL,name TEXT NOT NULL,PRIMARY KEY(thread_id,msg_id,quality),FOREIGN KEY(thread_id,msg_id) REFERENCES messages(thread_id,msg_id) ON DELETE CASCADE);
    CREATE INDEX IF NOT EXISTS restored_assets_name ON restored_assets(name);
    CREATE TABLE IF NOT EXISTS restored_gc(name TEXT PRIMARY KEY);
    CREATE TRIGGER IF NOT EXISTS restored_asset_delete BEFORE DELETE ON restored_assets BEGIN INSERT OR IGNORE INTO restored_gc VALUES(OLD.name); END;
    CREATE TABLE IF NOT EXISTS conversation_policies(thread_id TEXT NOT NULL,is_group INTEGER NOT NULL,is_protected INTEGER NOT NULL,ttl INTEGER NOT NULL,PRIMARY KEY(thread_id,is_group));
    CREATE TABLE IF NOT EXISTS pending_reactions(key TEXT PRIMARY KEY,data TEXT NOT NULL,next_try INTEGER NOT NULL DEFAULT 0);
    CREATE INDEX IF NOT EXISTS pending_reactions_due ON pending_reactions(next_try);
    CREATE TABLE IF NOT EXISTS reaction_previews(thread_id TEXT PRIMARY KEY,data TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS history_heads(thread_id TEXT NOT NULL,is_group INTEGER NOT NULL,fetched_at INTEGER NOT NULL,PRIMARY KEY(thread_id,is_group));
    CREATE TABLE IF NOT EXISTS group_metadata(thread_id TEXT PRIMARY KEY,version TEXT NOT NULL,data TEXT NOT NULL);
    PRAGMA user_version=10;")?;
    tx.commit()?;
    Ok(())
}
/// Backfill at most 500 existing rows. Returns true when this rebuild is complete.
pub(crate) fn backfill(conn: &Connection) -> Result<bool> {
    let (last, done): (i64, bool) = conn.query_row(
        "SELECT last_rowid,is_complete FROM expiry_index_state WHERE id=1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if done {
        return Ok(true);
    }
    let rows = {
        let mut stmt=conn.prepare("SELECT rowid,thread_id,msg_id,COALESCE(raw_json,''),COALESCE(ts,0) FROM messages WHERE rowid>?1 ORDER BY rowid LIMIT 500")?;
        let rows = stmt
            .query_map([last], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    let tx = conn.unchecked_transaction()?;
    for (_, thread, id, raw, ts) in &rows {
        if raw.len() <= 4 * 1024 * 1024 {
            if let Some(deadline) = serde_json::from_str::<Value>(raw)
                .ok()
                .and_then(|raw| deadline(&raw, *ts))
            {
                tx.execute(
                    "INSERT OR IGNORE INTO message_expiry VALUES(?1,?2,?3)",
                    params![thread, id, deadline],
                )?;
            }
        }
    }
    let complete = rows.len() < 500;
    tx.execute(
        "UPDATE expiry_index_state SET last_rowid=?1,is_complete=?2 WHERE id=1",
        params![rows.last().map(|row| row.0).unwrap_or(last), complete],
    )?;
    tx.commit()?;
    Ok(complete)
}
/// Delete one due batch and retain content-free identities so backup/history replay cannot revive it.
pub(crate) fn purge(conn: &Connection, now: i64) -> Result<Vec<(String, String)>> {
    let due = {
        let mut stmt=conn.prepare("SELECT thread_id,msg_id FROM message_expiry WHERE expires_at<=?1 ORDER BY expires_at LIMIT 500")?;
        let rows = stmt
            .query_map([now], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    if due.is_empty() {
        return Ok(due);
    }
    let tx = conn.unchecked_transaction()?;
    for (thread, id) in &due {
        tx.execute(
            "INSERT OR IGNORE INTO deleted_messages(thread_id,msg_id) VALUES(?1,?2)",
            params![thread, id],
        )?;
        tx.execute(
            "DELETE FROM messages WHERE thread_id=?1 AND msg_id=?2",
            params![thread, id],
        )?;
        tx.execute("DELETE FROM reaction_previews WHERE thread_id=?1 AND CASE WHEN json_valid(data) THEN json_extract(data,'$.target') END=?2", params![thread,id])?;
    }
    for thread in due
        .iter()
        .map(|(thread, _)| thread)
        .collect::<std::collections::HashSet<_>>()
    {
        tx.execute("UPDATE threads SET last_ts=COALESCE((SELECT MAX(ts) FROM messages WHERE thread_id=?1),0) WHERE thread_id=?1",[thread])?;
    }
    tx.commit()?;
    Ok(due)
}
/// Process only queued generated filenames; shared live attachments are never unlinked.
pub(crate) fn clean_restored(session: &crate::session::Session, conn: &Connection) -> Result<()> {
    let names = {
        let mut stmt = conn.prepare("SELECT name FROM restored_gc LIMIT 100")?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        names
    };
    let root = session
        .history_path()
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Missing account directory"))?
        .join("restored-media");
    for name in names {
        let referenced: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM restored_assets WHERE name=?1)",
            [&name],
            |row| row.get(0),
        )?;
        if !referenced && name.len() == 64 && name.bytes().all(|b| b.is_ascii_hexdigit()) {
            let source = root.join(&name);
            let open_cache = session
                .cache_path()
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Missing account cache directory"))?
                .join("restored-open")
                .join(crate::crypto::md5_hex(&source.to_string_lossy()));
            match std::fs::remove_dir_all(&open_cache) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => continue,
            }
            match std::fs::remove_file(&source) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => continue,
            }
        }
        conn.execute("DELETE FROM restored_gc WHERE name=?1", [name])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MessageStore;
    use serde_json::json;
    #[test]
    fn deadline_uses_server_time_plus_milliseconds_and_never_receipt_time() {
        assert_eq!(
            deadline(
                &json!({"ttl":3600000,"serverTime":1800000000000i64,"sendDttm":1900000000000i64}),
                0
            ),
            Some(1800003600000)
        );
        assert_eq!(deadline(&json!({"ttl":3600000}), 0), None);
        assert_eq!(
            deadline(&json!({"ttl":0,"serverTime":1800000000000i64}), 0),
            None
        );
    }
    #[test]
    fn history_limit_is_applied_after_expiry_even_when_purge_batch_is_exhausted() {
        let store = MessageStore::open(":memory:").unwrap();
        store.conn.execute("INSERT INTO messages(thread_id,msg_id,text,ts,is_outgoing,raw_json) VALUES('peer','live','keep',1,0,'{}')", []).unwrap();
        store.conn.execute_batch("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<501) INSERT INTO messages(thread_id,msg_id,text,ts,is_outgoing,raw_json) SELECT 'peer','expired-'||i,'gone',1000+i,0,'{}' FROM n;
            INSERT INTO message_expiry(thread_id,msg_id,expires_at) SELECT thread_id,msg_id,1 FROM messages WHERE msg_id!='live';").unwrap();
        let rows = store.messages_for("peer", 1).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].msg_id, "live");
    }
    #[test]
    fn expiry_deletes_search_projection_and_rejects_replay() {
        let store = MessageStore::open(":memory:").unwrap();
        let raw = json!({"ttl":1000,"serverTime":1800000000000i64}).to_string();
        store.conn.execute("INSERT INTO messages(thread_id,msg_id,text,ts,is_outgoing,raw_json) VALUES('peer','msg','secret',1800000000000,0,?1)",[&raw]).unwrap();
        store
            .conn
            .execute(
                "INSERT INTO reaction_previews(thread_id,data) VALUES('peer',?1)",
                [json!({"target":"msg"}).to_string()],
            )
            .unwrap();
        assert_eq!(purge(&store.conn, 1800000001000).unwrap().len(), 1);
        assert_eq!(
            store
                .conn
                .query_row("SELECT COUNT(*) FROM reaction_previews", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM search_entries", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
        assert_eq!(store.conn.execute("INSERT OR IGNORE INTO messages(thread_id,msg_id,text,ts,is_outgoing,raw_json) VALUES('peer','msg','secret',1800000000000,0,?1)",[&raw]).unwrap(),0);
    }
}
