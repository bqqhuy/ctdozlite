//! Local message and thread store backed by SQLite (rusqlite bundled).
//!
//! Persists every message seen via polling or group history so chat history accumulates locally.

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::chat::{ChatMessage, Thread};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredMessage {
    pub msg_id: String,
    pub thread_id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub text: String,
    pub msg_type: String,
    pub ts: i64,
    pub is_outgoing: bool,
    pub raw_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredThread {
    pub thread_id: String,
    pub name: String,
    pub is_group: bool,
    pub last_ts: i64,
}

/// Local-only conversation presentation controls, isolated by account database.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConversationPreference {
    /// Pinned conversations sort before ordinary conversations.
    pub pinned: bool,
    /// Suppress desktop attention for this thread while still receiving messages.
    pub muted: bool,
    /// Hide from the normal list; recover through the hidden-conversations toggle.
    pub hidden: bool,
}

/// Successfully imported contiguous phone-history interval, scoped by the account database.
#[derive(Clone, Debug)]
pub(crate) struct PhoneSyncRange {
    pub thread_id: String,
    pub from_ts: i64,
    pub to_ts: i64,
}

pub struct MessageStore {
    pub conn: Connection,
}

impl MessageStore {
    /// Initial history window; deeper reads occur when opening a thread.
    pub const PRELOAD_MSGS: usize = 20;
    /// Open SQLite database at `path` and initialize table schemas and indices if absent.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path = path.as_ref();
        if path != std::path::Path::new(":memory:") {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                crate::storage::private_dir(parent)?;
            }
        }
        let conn = if path == std::path::Path::new(":memory:") {
            Connection::open_in_memory()?
        } else {
            Connection::open(path)?
        };

        let schema: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        anyhow::ensure!(
            schema <= 11,
            "Database requires a newer application version"
        );
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;",
        )?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS messages (
                msg_id TEXT NOT NULL,
                thread_id TEXT NOT NULL,
                sender_id TEXT,
                sender_name TEXT,
                text TEXT,
                msg_type TEXT,
                ts INTEGER,
                is_outgoing INTEGER,
                raw_json TEXT,
                PRIMARY KEY(thread_id, msg_id)
            );
            CREATE INDEX IF NOT EXISTS idx_thread_ts ON messages(thread_id, ts);
            CREATE TABLE IF NOT EXISTS history_sync (thread_id TEXT PRIMARY KEY, cursor TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS threads (
                thread_id TEXT PRIMARY KEY,
                name TEXT,
                is_group INTEGER,
                last_ts INTEGER
            );",
        )?;

        // Transactional migration from the original globally keyed message ID.
        let thread_pk: i64 = conn.query_row(
            "SELECT pk FROM pragma_table_info('messages') WHERE name='thread_id'",
            [],
            |row| row.get(0),
        )?;
        if thread_pk == 0 {
            let migration = conn.unchecked_transaction()?;
            migration.execute_batch("ALTER TABLE messages RENAME TO messages_v1;
                CREATE TABLE messages (msg_id TEXT NOT NULL, thread_id TEXT NOT NULL,
                    sender_id TEXT, sender_name TEXT, text TEXT, msg_type TEXT,
                    ts INTEGER, is_outgoing INTEGER, raw_json TEXT, PRIMARY KEY(thread_id,msg_id));
                INSERT INTO messages SELECT msg_id,thread_id,sender_id,sender_name,text,msg_type,ts,is_outgoing,raw_json FROM messages_v1;
                DROP TABLE messages_v1;
                CREATE INDEX idx_thread_ts ON messages(thread_id,ts);")?;
            migration.commit()?;
        }

        if schema < 2 {
            let migration = conn.unchecked_transaction()?;
            migration.execute("DELETE FROM messages WHERE text = 'sendBubbleMessage'", [])?;
            migration.pragma_update(None, "user_version", 2)?;
            migration.commit()?;
        }
        if schema < 3 {
            let migration = conn.unchecked_transaction()?;
            migration.execute_batch(
                "CREATE TABLE IF NOT EXISTS thread_preferences (
                thread_id TEXT PRIMARY KEY, is_pinned INTEGER NOT NULL DEFAULT 0,
                is_muted INTEGER NOT NULL DEFAULT 0, is_hidden INTEGER NOT NULL DEFAULT 0);
                PRAGMA user_version=3;",
            )?;
            migration.commit()?;
        }
        if schema < 4 {
            let migration = conn.unchecked_transaction()?;
            migration.execute_batch(
                "CREATE TABLE IF NOT EXISTS phone_unmapped (
                partition_id TEXT NOT NULL, msg_id TEXT NOT NULL, raw_json TEXT NOT NULL,
                PRIMARY KEY(partition_id,msg_id));
                CREATE INDEX IF NOT EXISTS idx_message_client ON messages(thread_id,
                    CASE WHEN json_valid(raw_json) THEN json_extract(raw_json,'$.message_state.client_message_id') END);
                PRAGMA user_version=4;",
            )?;
            migration.commit()?;
        }
        if schema < 5 {
            let migration = conn.unchecked_transaction()?;
            migration.execute_batch("CREATE TABLE IF NOT EXISTS phone_sync_ranges(partition_id TEXT PRIMARY KEY,thread_id TEXT NOT NULL,from_ts INTEGER NOT NULL CHECK(from_ts>=0),to_ts INTEGER NOT NULL CHECK(to_ts>=from_ts)); PRAGMA user_version=5;")?;
            migration.commit()?;
        }
        if schema < 6 {
            let migration = conn.unchecked_transaction()?;
            migration.execute_batch("CREATE TABLE IF NOT EXISTS tool_config(key TEXT PRIMARY KEY,value TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS scheduled_messages(id TEXT PRIMARY KEY,owner TEXT NOT NULL,thread_id TEXT NOT NULL,deliver_at INTEGER NOT NULL,message_json TEXT NOT NULL,status TEXT NOT NULL,error TEXT NOT NULL,started_at INTEGER NOT NULL DEFAULT 0);
                CREATE INDEX IF NOT EXISTS scheduled_due ON scheduled_messages(owner,status,deliver_at);
                CREATE INDEX IF NOT EXISTS scheduled_recent ON scheduled_messages(owner,deliver_at);
                CREATE INDEX IF NOT EXISTS scheduled_attempts ON scheduled_messages(owner,started_at);
                CREATE TABLE IF NOT EXISTS auto_reply_seen(thread_id TEXT NOT NULL,msg_id TEXT NOT NULL,seen_at INTEGER NOT NULL,PRIMARY KEY(thread_id,msg_id));
                PRAGMA user_version=6;")?;
            migration.commit()?;
        }
        if schema < 7 {
            // Old scoped/final-marker completion could acknowledge ranges with missing batches.
            // Only derived coverage is invalidated; messages and server cursors are preserved.
            let migration = conn.unchecked_transaction()?;
            migration.execute_batch("DELETE FROM phone_sync_ranges; PRAGMA user_version=7;")?;
            migration.commit()?;
        }
        if schema < 8 {
            let migration = conn.unchecked_transaction()?;
            migration.execute_batch("CREATE TABLE IF NOT EXISTS deleted_messages (
                thread_id TEXT NOT NULL, msg_id TEXT NOT NULL,
                client_message_id TEXT NOT NULL DEFAULT '', sender_id TEXT NOT NULL DEFAULT '', is_local INTEGER NOT NULL DEFAULT 0,
                is_outgoing INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY(thread_id,msg_id));
                CREATE INDEX IF NOT EXISTS deleted_client ON deleted_messages(thread_id,client_message_id,is_outgoing);
                CREATE TABLE IF NOT EXISTS composer_drafts(thread_id TEXT PRIMARY KEY,text TEXT NOT NULL);
                CREATE TRIGGER IF NOT EXISTS reject_deleted_replay BEFORE INSERT ON messages
                WHEN EXISTS(SELECT 1 FROM deleted_messages WHERE thread_id=NEW.thread_id AND msg_id=NEW.msg_id)
                    OR EXISTS(SELECT 1 FROM deleted_messages d WHERE d.thread_id=NEW.thread_id
                        AND d.client_message_id!='' AND d.client_message_id!='0'
                        AND d.client_message_id=CASE WHEN json_valid(NEW.raw_json) THEN json_extract(NEW.raw_json,'$.message_state.client_message_id') END
                        AND d.is_outgoing=NEW.is_outgoing
                        AND (d.is_local=1 OR NEW.msg_id GLOB 'phone-client:*' OR NEW.msg_id GLOB 'out_*' OR NEW.msg_id GLOB 'local_*')
                        AND (NEW.is_outgoing=1 OR (d.sender_id!='' AND d.sender_id=CASE WHEN json_valid(NEW.raw_json) THEN NEW.raw_json -> '$.message_state.wire_ref.sender' END)))
                BEGIN SELECT RAISE(IGNORE); END;
                PRAGMA user_version=8;")?;
            migration.commit()?;
        }
        crate::search::register(&conn)?;
        if schema < 9 {
            crate::search::migrate(&conn)?;
        }
        crate::message_lifecycle::register(&conn)?;
        if schema < 10 {
            crate::message_lifecycle::migrate(&conn)?;
        }
        crate::message_lifecycle::backfill(&conn)?;
        if schema < 11 {
            let migration = conn.unchecked_transaction()?;
            migration.execute_batch(
                "CREATE TABLE IF NOT EXISTS upload_sessions (
                    upload_id TEXT PRIMARY KEY,
                    file_name TEXT NOT NULL,
                    total_size INTEGER NOT NULL,
                    chunk_size INTEGER NOT NULL,
                    total_chunks INTEGER NOT NULL,
                    completed_chunks TEXT NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                PRAGMA user_version=11;",
            )?;
            migration.commit()?;
        }
        if schema < 12 {
            crate::message_lifecycle::migrate_v12(&conn)?;
        }
        Ok(Self { conn })
    }

    /// Read only coverage explicitly committed by a successful phone-sync range.
    /// # Errors
    /// Returns SQLite or invalid coverage failures.
    pub(crate) fn phone_sync_ranges(
        &self,
    ) -> Result<std::collections::HashMap<String, PhoneSyncRange>> {
        let mut stmt = self
            .conn
            .prepare("SELECT partition_id,thread_id,from_ts,to_ts FROM phone_sync_ranges")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                PhoneSyncRange {
                    thread_id: r.get(1)?,
                    from_ts: r.get(2)?,
                    to_ts: r.get(3)?,
                },
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
    /// Commit completed coverage after all downloads/imports succeed; never store transfer keys/URLs.
    /// # Errors
    /// Invalid, disconnected ranges or SQLite errors roll back the coverage transaction.
    pub(crate) fn save_phone_sync_ranges(&self, ranges: &[(String, PhoneSyncRange)]) -> Result<()> {
        use rusqlite::OptionalExtension;
        let tx = self.conn.unchecked_transaction()?;
        for (partition, range) in ranges {
            anyhow::ensure!(
                !range.thread_id.is_empty() && range.from_ts >= 0 && range.to_ts >= range.from_ts,
                "Invalid phone coverage"
            );
            let previous = tx
                .query_row(
                    "SELECT thread_id,from_ts,to_ts FROM phone_sync_ranges WHERE partition_id=?1",
                    [partition],
                    |r| {
                        Ok(PhoneSyncRange {
                            thread_id: r.get(0)?,
                            from_ts: r.get(1)?,
                            to_ts: r.get(2)?,
                        })
                    },
                )
                .optional()?;
            if let Some(old) = previous.filter(|old| old.thread_id == range.thread_id) {
                anyhow::ensure!(
                    range.from_ts <= old.to_ts && range.to_ts >= old.from_ts,
                    "Disconnected phone coverage"
                );
            }
            tx.execute("INSERT INTO phone_sync_ranges(partition_id,thread_id,from_ts,to_ts) VALUES(?1,?2,?3,?4) ON CONFLICT(partition_id) DO UPDATE SET thread_id=excluded.thread_id,from_ts=excluded.from_ts,to_ts=excluded.to_ts",params![partition,range.thread_id,range.from_ts,range.to_ts])?;
        }
        tx.commit()?;
        Ok(())
    }
    /// Explicit full rescan discards progress only, retaining every stored message.
    ///
    /// # Errors
    /// Returns the SQLite error if progress cannot be removed.
    pub(crate) fn reset_phone_sync_ranges(&self) -> Result<()> {
        self.conn.execute("DELETE FROM phone_sync_ranges", [])?;
        Ok(())
    }

    /// The conversation (and whether it is a group) holding `msg_id`, when exactly one does.
    /// # Errors
    /// Returns database failures.
    pub(crate) fn unique_thread_of_message(&self, msg_id: &str) -> Result<Option<(String, bool)>> {
        let mut statement = self.conn.prepare_cached(
            "SELECT m.thread_id,COALESCE(t.is_group,0) FROM messages m \
             LEFT JOIN threads t ON t.thread_id=m.thread_id WHERE m.msg_id=?1 LIMIT 2",
        )?;
        let rows = statement
            .query_map(params![msg_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok((rows.len() == 1).then(|| rows[0].clone()))
    }

    /// Raise the delivery state of outgoing messages named by receipts; never lowers it.
    ///
    /// A receipt that names its conversation only touches that conversation (and its kind,
    /// when the thread row records it). Without a conversation, a row is updated only when
    /// the message ID matches exactly one outgoing message.
    /// Returns `(thread_id, msg_id, new state)` for rows that changed.
    /// # Errors
    /// Returns database failures; malformed rows are skipped.
    pub(crate) fn apply_receipts(
        &self,
        receipts: &[crate::presence::Receipt],
    ) -> Result<Vec<(String, String, crate::message_model::DeliveryState)>> {
        use crate::message_model::DeliveryState;
        let mut changed = Vec::new();
        let transaction = self.conn.unchecked_transaction()?;
        {
            let mut scoped = transaction.prepare_cached(
                "SELECT m.thread_id,m.raw_json FROM messages m \
                 LEFT JOIN threads t ON t.thread_id=m.thread_id \
                 WHERE m.thread_id IN (?1,?2,?3) AND m.msg_id=?4 AND m.is_outgoing=1 \
                 AND (t.is_group IS NULL OR t.is_group=?5) LIMIT 3",
            )?;
            let mut unscoped = transaction.prepare_cached(
                "SELECT thread_id,raw_json FROM messages WHERE msg_id=?1 AND is_outgoing=1 LIMIT 2",
            )?;
            let mut update = transaction.prepare_cached(
                "UPDATE messages SET raw_json=?1 WHERE thread_id=?2 AND msg_id=?3",
            )?;
            for receipt in receipts.iter().take(512) {
                let incoming = if receipt.read {
                    DeliveryState::Read
                } else {
                    DeliveryState::Delivered
                };
                for id in receipt.msg_ids.iter().take(8) {
                    let row = |r: &rusqlite::Row<'_>| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                    };
                    let rows = if receipt.thread_id.is_empty() {
                        unscoped
                            .query_map(params![id], row)?
                            .collect::<rusqlite::Result<Vec<_>>>()?
                    } else {
                        let group = if receipt.is_group {
                            format!("g{}", receipt.thread_id)
                        } else {
                            receipt.thread_id.clone()
                        };
                        let prefixed = if receipt.is_group {
                            format!("group_{}", receipt.thread_id)
                        } else {
                            receipt.thread_id.clone()
                        };
                        scoped
                            .query_map(
                                params![receipt.thread_id, group, prefixed, id, receipt.is_group],
                                row,
                            )?
                            .collect::<rusqlite::Result<Vec<_>>>()?
                    };
                    // An unscoped receipt must resolve uniquely; scoped filtering happens in
                    // SQLite before LIMIT so collisions in other threads cannot hide its row.
                    let rows = if receipt.thread_id.is_empty() && rows.len() != 1 {
                        Vec::new()
                    } else {
                        rows
                    };
                    for (thread, raw) in rows {
                        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&raw) else {
                            continue;
                        };
                        if !value.is_object() {
                            continue;
                        }
                        if !value.get("message_state").is_some_and(|v| v.is_object()) {
                            value["message_state"] = serde_json::json!({});
                        }
                        let current: DeliveryState = value["message_state"]
                            .get("delivery")
                            .and_then(|v| serde_json::from_value(v.clone()).ok())
                            .unwrap_or_default();
                        let next = current.merge(incoming);
                        if next == current {
                            continue;
                        }
                        value["message_state"]["delivery"] = serde_json::to_value(next)?;
                        update.execute(params![value.to_string(), thread, id])?;
                        changed.push((thread, id.clone(), next));
                    }
                }
            }
        }
        transaction.commit()?;
        Ok(changed)
    }

    /// Update only renewed media metadata, preserving delivery, recall and local flags.
    /// # Errors
    /// Returns invalid metadata or database failures; never creates a missing message.
    pub(crate) fn update_media_metadata(&self, thread: &str, message: &ChatMessage) -> Result<()> {
        use rusqlite::OptionalExtension;
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT raw_json FROM messages WHERE thread_id=?1 AND msg_id=?2",
                params![thread, message.id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(raw) = raw else { return Ok(()) };
        let mut old: serde_json::Value = serde_json::from_str(&raw)?;
        anyhow::ensure!(old.is_object(), "Invalid media metadata");
        if old
            .get("message_state")
            .and_then(|s| s.get("kind"))
            .and_then(|k| k.as_str())
            == Some("recall")
        {
            return Ok(());
        }
        old["link_thumb"] = serde_json::json!(message.link_thumb);
        old["link_url"] = serde_json::json!(message.link_url);
        if let Some(v) = message.wire_ref.fields.get("renewed_media") {
            let mut node = &mut old;
            for key in ["message_state", "wire_ref", "fields"] {
                if node.get(key).is_none() {
                    node[key] = serde_json::json!({});
                }
                anyhow::ensure!(
                    node.get(key).is_some_and(|v| v.is_object()),
                    "Invalid media metadata parent"
                );
                node = &mut node[key];
            }
            node["renewed_media"] = v.clone();
        }
        self.conn.execute("UPDATE messages SET raw_json=?1 WHERE thread_id=?2 AND msg_id=?3 AND text != '[Tin nhắn đã thu hồi]'",params![old.to_string(),thread,message.id])?;
        Ok(())
    }
    /// Insert or reconcile a message without duplicating its composite identity.
    /// # Errors
    /// Returns SQLite or stored metadata validation failures.
    pub fn upsert_message(&self, msg: &StoredMessage) -> Result<bool> {
        if self.is_deleted_message(msg)? {
            return Ok(false);
        }
        let rows = self.conn.prepare_cached(
            "INSERT OR IGNORE INTO messages (msg_id, thread_id, sender_id, sender_name, text, msg_type, ts, is_outgoing, raw_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)")?.execute(
            params![
                msg.msg_id,
                msg.thread_id,
                msg.sender_id,
                msg.sender_name,
                msg.text,
                msg.msg_type,
                msg.ts,
                if msg.is_outgoing { 1 } else { 0 },
                msg.raw_json,
            ],
        )?;
        if rows == 0 {
            // Another writer may have deleted the row after the admission check.
            if self.is_deleted_message(msg)? {
                return Ok(false);
            }
            let (old_text, old_raw): (String, String) = self.conn.query_row(
                "SELECT text,raw_json FROM messages WHERE thread_id=?1 AND msg_id=?2",
                params![msg.thread_id, msg.msg_id],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                        r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    ))
                },
            )?;
            let old = serde_json::from_str::<serde_json::Value>(&old_raw)
                .unwrap_or(serde_json::json!({}));
            let mut new = serde_json::from_str::<serde_json::Value>(&msg.raw_json)
                .unwrap_or(serde_json::json!({}));
            // Network deltas omit local flags and older media details. Only an
            // explicit UI snapshot may change local pin/star choices.
            if let (Some(old_map), Some(new_map)) = (old.as_object(), new.as_object_mut()) {
                for (key, value) in old_map {
                    if key == "is_pinned" || key == "is_starred" || !new_map.contains_key(key) {
                        new_map.insert(key.clone(), value.clone());
                    }
                }
            }
            let mut state = ChatMessage::default();
            if let Some(value) = old.get("message_state") {
                state.restore_state(value)?;
            }
            if let Some(value) = serde_json::from_str::<serde_json::Value>(&msg.raw_json)
                .ok()
                .and_then(|v| v.get("message_state").cloned())
            {
                state.restore_state(&value)?;
            }
            let recalled =
                old_text == "[Tin nhắn đã thu hồi]" || msg.text == "[Tin nhắn đã thu hồi]";
            if recalled {
                state.mark_recalled();
            }
            if !new.is_object() {
                new = serde_json::json!({});
            }
            if let Some(renewed) = old.pointer("/message_state/wire_ref/fields/renewed_media") {
                if let Some(url) = renewed.get("original") {
                    new["link_url"] = url.clone();
                }
                if let Some(thumb) = renewed.get("thumbnail") {
                    new["link_thumb"] = thumb.clone();
                }
                state
                    .wire_ref
                    .fields
                    .insert("renewed_media".into(), renewed.clone());
            }
            new["message_state"] = state.durable_state();
            if recalled {
                for key in [
                    "stickerId",
                    "cateId",
                    "type",
                    "link_url",
                    "link_title",
                    "link_desc",
                    "link_thumb",
                    "link_src",
                    "reply_to_id",
                    "reply_to_text",
                    "reply_to_sender",
                ] {
                    new.as_object_mut().unwrap().remove(key);
                }
            }
            let text = if recalled {
                "[Tin nhắn đã thu hồi]"
            } else {
                &msg.text
            };
            self.conn.execute(
                "UPDATE messages SET raw_json=?1,msg_type=?2,text=?3 WHERE thread_id=?4 AND msg_id=?5",
                params![new.to_string(),msg.msg_type,text,msg.thread_id,msg.msg_id])?;
        }
        Ok(rows > 0)
    }

    // Called only inside persist_snapshot's transaction, so ID promotion and the
    // following metadata merge commit together. The client index bounds lookups.
    pub(crate) fn reconcile_send_identity(
        &self,
        incoming: &ChatMessage,
        row: &mut StoredMessage,
    ) -> Result<()> {
        let client = incoming.client_message_id.as_str();
        if !incoming.is_outgoing || client.is_empty() || client == "0" {
            return Ok(());
        }
        let mut statement=self.conn.prepare_cached("SELECT msg_id,text,msg_type,ts,raw_json FROM messages WHERE thread_id=?1 AND is_outgoing=1 AND (CASE WHEN json_valid(raw_json) THEN json_extract(raw_json,'$.message_state.client_message_id') END)=?2 LIMIT 2")?;
        let candidates = statement
            .query_map(params![row.thread_id, client], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if candidates.len() != 1 {
            return Ok(());
        }
        let (id, text, kind, ts, raw) = &candidates[0];
        if *id == row.msg_id {
            return Ok(());
        }
        let mut existing = ChatMessage {
            id: id.clone(),
            text: text.clone(),
            ts: *ts,
            is_outgoing: true,
            ..Default::default()
        };
        if let Some(state) = serde_json::from_str::<serde_json::Value>(raw)
            .ok()
            .and_then(|v| v.get("message_state").cloned())
        {
            existing.restore_state(&state)?;
        }
        if !crate::chat::same_message(&existing, incoming) {
            return Ok(());
        }
        let local = |id: &str| id.starts_with("out_") || id.starts_with("local_");
        if local(id) && !local(&row.msg_id) {
            // Do not delete or merge ambiguous pre-existing duplicates from old builds.
            self.conn.execute("UPDATE messages SET msg_id=?1,ts=CASE WHEN ?4>0 THEN ?4 ELSE ts END WHERE thread_id=?2 AND msg_id=?3 AND NOT EXISTS(SELECT 1 FROM messages WHERE thread_id=?2 AND msg_id=?1)",params![row.msg_id,row.thread_id,id,row.ts])?;
        } else if !local(id) && local(&row.msg_id) {
            row.msg_id = id.clone();
            row.text = text.clone();
            row.msg_type = kind.clone();
            row.ts = *ts;
        }
        Ok(())
    }

    /// Remove message content while retaining only identity needed to reject history replay.
    /// # Errors
    /// Returns SQLite failures; deletion and its marker commit atomically.
    pub fn delete_message(&mut self, thread_id: &str, msg_id: &str, ts: i64) -> Result<()> {
        use rusqlite::OptionalExtension;
        let key = if msg_id.is_empty() {
            format!("msg_{ts}")
        } else {
            msg_id.to_owned()
        };
        let row = self
            .conn
            .query_row(
                "SELECT raw_json,is_outgoing FROM messages WHERE thread_id=?1 AND msg_id=?2",
                params![thread_id, key],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                        r.get::<_, bool>(1)?,
                    ))
                },
            )
            .optional()?;
        let mut message = ChatMessage {
            id: key,
            ts,
            ..Default::default()
        };
        if let Some((raw, outgoing)) = row {
            message.is_outgoing = outgoing;
            if let Some(state) = serde_json::from_str::<serde_json::Value>(&raw)
                .ok()
                .and_then(|v| v.get("message_state").cloned())
            {
                message.restore_state(&state)?;
            }
        }
        self.delete_chat_message(thread_id, &message)
    }

    /// Delete an acknowledged message, including a local-only row not yet persisted.
    /// # Errors
    /// Returns invalid identity or SQLite failures without a partial deletion.
    pub fn delete_chat_message(&mut self, thread: &str, message: &ChatMessage) -> Result<()> {
        anyhow::ensure!(!thread.is_empty(), "Missing thread identity");
        let key = if message.id.is_empty() {
            format!("msg_{}", message.ts)
        } else {
            message.id.clone()
        };
        let local = key.starts_with("out_") || key.starts_with("local_");
        let sender = message
            .wire_ref
            .sender
            .as_ref()
            .map(serde_json::Value::to_string)
            .unwrap_or_default();
        let tx = self.conn.savepoint()?;
        tx.execute("INSERT INTO deleted_messages(thread_id,msg_id,client_message_id,is_local,is_outgoing,sender_id) VALUES(?1,?2,?3,?4,?5,?6)
            ON CONFLICT(thread_id,msg_id) DO UPDATE SET client_message_id=CASE WHEN excluded.client_message_id='' THEN deleted_messages.client_message_id ELSE excluded.client_message_id END",
            params![thread,key,message.client_message_id,local,message.is_outgoing,sender])?;
        tx.execute(
            "DELETE FROM messages WHERE thread_id=?1 AND msg_id=?2",
            params![thread, key],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn is_deleted_message(&self, message: &StoredMessage) -> Result<bool> {
        let raw = serde_json::from_str::<serde_json::Value>(&message.raw_json).unwrap_or_default();
        let client = raw
            .pointer("/message_state/client_message_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let sender = raw
            .pointer("/message_state/wire_ref/sender")
            .map(serde_json::Value::to_string)
            .unwrap_or_default();
        self.is_deleted_identity(
            &message.thread_id,
            &message.msg_id,
            client,
            message.is_outgoing,
            &sender,
        )
    }

    fn is_deleted_identity(
        &self,
        thread: &str,
        id: &str,
        client: &str,
        outgoing: bool,
        sender: &str,
    ) -> Result<bool> {
        // Two concrete server IDs remain distinct even when a client ID repeats.
        let provisional =
            id.starts_with("phone-client:") || id.starts_with("out_") || id.starts_with("local_");
        Ok(self.conn.prepare_cached("SELECT EXISTS(SELECT 1 FROM deleted_messages WHERE thread_id=?1 AND msg_id=?2)
            OR (?3!='' AND ?3!='0' AND EXISTS(SELECT 1 FROM deleted_messages WHERE thread_id=?1 AND client_message_id=?3
                AND is_outgoing=?4 AND (is_local=1 OR ?5=1) AND (?4=1 OR (?6!='' AND sender_id=?6))))")?
            .query_row(params![thread,id,client,outgoing,provisional,sender],|r|r.get(0))?)
    }

    /// Filter replayed deleted messages before UI publication and notification delivery.
    /// # Errors
    /// Returns SQLite failures; callers must not publish an unchecked batch on failure.
    pub(crate) fn filter_deleted(&self, threads: &mut [Thread]) -> Result<()> {
        for thread in threads {
            let mut kept = Vec::with_capacity(thread.messages.len());
            let mut removed = false;
            for message in std::mem::take(&mut thread.messages) {
                let key = if message.id.is_empty() {
                    format!("msg_{}", message.ts)
                } else {
                    message.id.clone()
                };
                let sender = message
                    .wire_ref
                    .sender
                    .as_ref()
                    .map(serde_json::Value::to_string)
                    .unwrap_or_default();
                if crate::message_lifecycle::expired(&message, crate::login::unix_now() * 1000)
                    || self.is_deleted_identity(
                        &thread.id,
                        &key,
                        &message.client_message_id,
                        message.is_outgoing,
                        &sender,
                    )?
                {
                    removed = true;
                } else {
                    kept.push(message);
                }
            }
            thread.messages = kept;
            let last_deleted = !thread.presentation.last_message_id.is_empty()
                && self.is_deleted_identity(
                    &thread.id,
                    &thread.presentation.last_message_id,
                    "",
                    false,
                    "",
                )?;
            if removed || last_deleted {
                thread.refresh_after_removal();
                if thread.messages.is_empty() {
                    thread.unread.fresh = false;
                }
            }
        }
        Ok(())
    }

    /// Load one account-scoped composer draft without loading other conversations.
    /// # Errors
    /// Returns SQLite failures.
    pub(crate) fn composer_draft(&self, thread: &str) -> Result<String> {
        use rusqlite::OptionalExtension;
        Ok(self
            .conn
            .query_row(
                "SELECT text FROM composer_drafts WHERE thread_id=?1",
                [thread],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or_default())
    }

    #[cfg(test)]
    fn save_composer_draft(&self, thread: &str, text: &str) -> Result<()> {
        self.save_composer_document(thread, &crate::mentions::Draft::from(text))
    }

    /// Load mention metadata only when it belongs to the exact persisted literal draft.
    pub(crate) fn composer_document(&self, thread: &str) -> Result<crate::mentions::Draft> {
        use rusqlite::OptionalExtension;
        let text = self.composer_draft(thread)?;
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM tool_config WHERE key=?1",
                [format!("draft_mentions:{thread}")],
                |r| r.get(0),
            )
            .optional()?;
        let saved = raw
            .filter(|r| r.len() <= 128 * 1024)
            .and_then(|r| serde_json::from_str::<crate::mentions::Draft>(&r).ok());
        Ok(saved
            .filter(|d| d.text == text && crate::mentions::validate(&d.text, &d.mentions).is_ok())
            .unwrap_or_else(|| text.into()))
    }

    /// Persist literal text and explicit identities in one SQLite transaction.
    pub(crate) fn save_composer_document(
        &self,
        thread: &str,
        draft: &crate::mentions::Draft,
    ) -> Result<()> {
        crate::mentions::validate(&draft.text, &draft.mentions)?;
        let tx = self.conn.unchecked_transaction()?;
        anyhow::ensure!(
            !thread.is_empty() && draft.text.len() <= 64 * 1024,
            "Invalid composer draft"
        );
        if draft.text.is_empty() {
            tx.execute("DELETE FROM composer_drafts WHERE thread_id=?1", [thread])?;
        } else {
            tx.execute("INSERT INTO composer_drafts(thread_id,text) VALUES(?1,?2) ON CONFLICT(thread_id) DO UPDATE SET text=excluded.text",params![thread,draft.text])?;
        }
        let key = format!("draft_mentions:{thread}");
        if draft.mentions.is_empty() {
            tx.execute("DELETE FROM tool_config WHERE key=?1", [&key])?;
        } else {
            tx.execute("INSERT INTO tool_config(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, serde_json::to_string(draft)?])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Read local conversation choices without loading message bodies.
    /// # Errors
    /// Returns SQLite errors; preferences belong to this account database only.
    pub fn conversation_preferences(
        &self,
    ) -> Result<std::collections::HashMap<String, ConversationPreference>> {
        let mut stmt = self
            .conn
            .prepare("SELECT thread_id,is_pinned,is_muted,is_hidden FROM thread_preferences")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get(0)?,
                ConversationPreference {
                    pinned: r.get(1)?,
                    muted: r.get(2)?,
                    hidden: r.get(3)?,
                },
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Persist one explicit local menu change; network snapshots cannot overwrite it.
    /// # Errors
    /// Rejects missing identity or a SQLite write failure.
    pub fn set_conversation_preference(
        &self,
        id: &str,
        value: ConversationPreference,
    ) -> Result<()> {
        anyhow::ensure!(!id.trim().is_empty(), "Missing thread identity");
        self.conn.execute("INSERT INTO thread_preferences(thread_id,is_pinned,is_muted,is_hidden) VALUES(?1,?2,?3,?4)
            ON CONFLICT(thread_id) DO UPDATE SET is_pinned=excluded.is_pinned,is_muted=excluded.is_muted,is_hidden=excluded.is_hidden",
            params![id,value.pinned,value.muted,value.hidden])?;
        Ok(())
    }

    /// Whether this account has completed its first server mute reconciliation.
    /// # Errors
    /// Returns SQLite errors.
    pub fn mute_sync_seeded(&self) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tool_config WHERE key='mute_sync_seeded' AND value='1')",
            [],
            |r| r.get(0),
        )?)
    }

    /// Group flag of a known thread, used to address account-wide mute changes.
    /// # Errors
    /// Returns SQLite errors; `None` means the thread kind is unknown.
    pub fn thread_is_group(&self, id: &str) -> Result<Option<bool>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .conn
            .query_row(
                "SELECT is_group FROM threads WHERE thread_id=?1",
                params![id],
                |r| r.get::<_, Option<bool>>(0),
            )
            .optional()?
            .flatten())
    }

    /// Apply server mute changes in one transaction, preserving local pin/hide choices.
    ///
    /// `mark_seeded` records that the first reconciliation finished, after which complete
    /// server snapshots are authoritative for the mute flag.
    /// # Errors
    /// Rejects an empty identity; any SQLite failure rolls back every change.
    pub fn apply_mute_changes(
        &self,
        changes: &[crate::mute_sync::MuteChange],
        mark_seeded: bool,
    ) -> Result<std::collections::HashMap<String, ConversationPreference>> {
        let tx = self.conn.unchecked_transaction()?;
        for change in changes {
            anyhow::ensure!(
                !change.thread_id.trim().is_empty(),
                "Missing thread identity"
            );
            tx.execute(
                "INSERT INTO thread_preferences(thread_id,is_muted) VALUES(?1,?2)
                ON CONFLICT(thread_id) DO UPDATE SET is_muted=excluded.is_muted",
                params![change.thread_id, change.muted],
            )?;
        }
        if mark_seeded {
            tx.execute("INSERT INTO tool_config(key,value) VALUES('mute_sync_seeded','1') ON CONFLICT(key) DO UPDATE SET value='1'", [])?;
        }
        tx.commit()?;
        self.conversation_preferences()
    }

    /// Delete local messages and history checkpoint atomically, retaining thread identity/preferences.
    /// # Errors
    /// Propagates SQLite failures; a later explicit sync may download history again.
    pub fn clear_thread_history(&mut self, id: &str) -> Result<()> {
        self.remove_thread_history(id)
    }

    /// Delete a server-acknowledged conversation and its visible/archived row identities atomically.
    /// # Errors
    /// Returns invalid identity or SQLite failures; content and markers roll back together.
    pub(crate) fn delete_conversation(&mut self, id: &str, visible: &[ChatMessage]) -> Result<()> {
        anyhow::ensure!(!id.is_empty(), "Missing thread identity");
        let tx = self.conn.transaction()?;
        tx.execute("INSERT OR IGNORE INTO deleted_messages(thread_id,msg_id,client_message_id,is_outgoing,sender_id)
            SELECT thread_id,msg_id,CASE WHEN json_valid(raw_json) THEN COALESCE(json_extract(raw_json,'$.message_state.client_message_id'),'') ELSE '' END,
            is_outgoing,CASE WHEN json_valid(raw_json) THEN COALESCE(raw_json -> '$.message_state.wire_ref.sender','') ELSE '' END
            FROM messages WHERE thread_id=?1",[id])?;
        for message in visible {
            let key = if message.id.is_empty() {
                format!("msg_{}", message.ts)
            } else {
                message.id.clone()
            };
            let sender = message
                .wire_ref
                .sender
                .as_ref()
                .map(serde_json::Value::to_string)
                .unwrap_or_default();
            tx.execute("INSERT OR IGNORE INTO deleted_messages(thread_id,msg_id,client_message_id,is_outgoing,sender_id,is_local) VALUES(?1,?2,?3,?4,?5,?6)",
                params![id,key,message.client_message_id,message.is_outgoing,sender,key.starts_with("out_")||key.starts_with("local_")])?;
        }
        for table in [
            "messages",
            "history_sync",
            "phone_sync_ranges",
            "threads",
            "thread_preferences",
            "composer_drafts",
        ] {
            tx.execute(&format!("DELETE FROM {table} WHERE thread_id=?1"), [id])?;
        }
        tx.execute(
            "DELETE FROM tool_config WHERE key=?1",
            [format!("draft_mentions:{id}")],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn remove_thread_history(&mut self, id: &str) -> Result<()> {
        anyhow::ensure!(!id.trim().is_empty(), "Missing thread identity");
        let tx = self.conn.savepoint()?;
        tx.execute("DELETE FROM messages WHERE thread_id=?1", params![id])?;
        tx.execute("DELETE FROM history_sync WHERE thread_id=?1", params![id])?;
        tx.execute(
            "DELETE FROM phone_sync_ranges WHERE thread_id=?1",
            params![id],
        )?;
        tx.execute(
            "UPDATE threads SET last_ts=0 WHERE thread_id=?1",
            params![id],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Insert or update thread metadata.
    pub fn upsert_thread(&self, t: &StoredThread) -> Result<()> {
        self.conn.prepare_cached(
            "INSERT INTO threads (thread_id, name, is_group, last_ts)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(thread_id) DO UPDATE SET
                 name = CASE WHEN excluded.name = '' OR excluded.name = 'Nhóm ' || excluded.thread_id
                    OR excluded.name = 'Người dùng ' || excluded.thread_id
                    THEN threads.name ELSE excluded.name END,
                 is_group = excluded.is_group,
                 last_ts = MAX(threads.last_ts, excluded.last_ts)")?.execute(
            params![
                t.thread_id,
                t.name,
                if t.is_group { 1 } else { 0 },
                t.last_ts,
            ],
        )?;
        Ok(())
    }

    /// Retrieve the newest `limit` messages in chronological order.
    pub fn messages_for(&self, thread_id: &str, limit: usize) -> Result<Vec<StoredMessage>> {
        crate::message_lifecycle::purge(&self.conn, crate::login::unix_now() * 1000)?;
        let mut stmt = self.conn.prepare(
            "SELECT msg_id, thread_id, sender_id, sender_name, text, msg_type, ts, is_outgoing, raw_json
             FROM (
                SELECT msg_id, thread_id, sender_id, sender_name, text, msg_type, ts, is_outgoing, raw_json
                FROM messages WHERE thread_id = ?1
                AND NOT EXISTS (SELECT 1 FROM message_expiry e
                    WHERE e.thread_id=messages.thread_id AND e.msg_id=messages.msg_id AND e.expires_at<=?3)
                ORDER BY ts DESC, msg_id DESC LIMIT ?2
             ) ORDER BY ts ASC, msg_id ASC",
        )?;

        let rows = stmt.query_map(
            params![thread_id, limit as i64, crate::login::unix_now() * 1000],
            |row| {
                let is_outgoing_num: i64 = row.get(7)?;
                Ok(StoredMessage {
                    msg_id: row.get(0)?,
                    thread_id: row.get(1)?,
                    sender_id: row.get(2).unwrap_or_default(),
                    sender_name: row.get(3).unwrap_or_default(),
                    text: row.get(4).unwrap_or_default(),
                    msg_type: row.get(5).unwrap_or_default(),
                    ts: row.get(6)?,
                    is_outgoing: is_outgoing_num != 0,
                    raw_json: row.get(8).unwrap_or_default(),
                })
            },
        )?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Retrieve all persisted thread metadata ordered by `last_ts` descending.
    pub fn all_threads(&self) -> Result<Vec<StoredThread>> {
        let mut stmt = self.conn.prepare(
            "SELECT thread_id, name, is_group, last_ts
             FROM threads
             ORDER BY last_ts DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let is_group_num: i64 = row.get(2).unwrap_or(0);
            Ok(StoredThread {
                thread_id: row.get(0)?,
                name: row.get(1).unwrap_or_default(),
                is_group: is_group_num != 0,
                last_ts: row.get(3).unwrap_or(0),
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Hydrate `threads` with persisted threads and message history from SQLite.
    /// Returns the total number of messages loaded into `threads`.
    pub fn hydrate_threads(
        &self,
        threads: &mut Vec<Thread>,
        limit_per_thread: usize,
    ) -> Result<usize> {
        self.hydrate_scoped(threads, limit_per_thread, true, None)
    }

    /// Read history only for the requested threads, leaving unrelated histories unloaded.
    /// # Errors
    /// Returns database query and decode failures without modifying other threads.
    pub fn hydrate_selected_threads(
        &self,
        threads: &mut Vec<Thread>,
        limit_per_thread: usize,
    ) -> Result<usize> {
        self.hydrate_scoped(threads, limit_per_thread, false, None)
    }

    /// Read a refresh baseline from actual SQLite rows before merging server previews.
    /// # Errors
    /// Returns history errors; callers must discard partial marks on failure.
    pub(crate) fn hydrate_refresh(
        &self,
        threads: &mut Vec<Thread>,
        limit: usize,
    ) -> Result<crate::chat::MessageFingerprints> {
        let mut marks = crate::chat::MessageFingerprints::new();
        self.hydrate_scoped(threads, limit, true, Some(&mut marks))?;
        Ok(marks)
    }

    fn hydrate_scoped(
        &self,
        threads: &mut Vec<Thread>,
        limit_per_thread: usize,
        include_unlisted: bool,
        mut marks: Option<&mut crate::chat::MessageFingerprints>,
    ) -> Result<usize> {
        self.filter_deleted(threads)?;
        crate::chat::deduplicate_threads(threads, None);

        let stored_threads = self.all_threads().unwrap_or_default();
        for st in stored_threads {
            if st.thread_id.is_empty() || st.thread_id == "0" {
                continue;
            }
            if let Some(existing) = threads.iter_mut().find(|t| t.id == st.thread_id) {
                if crate::chat::is_generic_thread_name(&existing.name) && !st.name.is_empty() {
                    existing.name = st.name;
                }
                existing.is_group = existing.is_group || st.is_group;
            } else if include_unlisted {
                threads.push(Thread {
                    presentation: Default::default(),
                    unread: Default::default(),
                    id: st.thread_id.clone(),
                    name: if st.name.is_empty() {
                        format!("Người dùng {}", st.thread_id)
                    } else {
                        st.name
                    },
                    preview: String::new(),
                    time: String::new(),
                    avatar: None,
                    is_group: st.is_group,
                    messages: Vec::new(),
                });
            }
        }

        let mut loaded_count = 0;
        for t in threads.iter_mut() {
            use rusqlite::OptionalExtension;
            if let Some(raw) = self
                .conn
                .query_row(
                    "SELECT data FROM reaction_previews WHERE thread_id=?1",
                    [&t.id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
            {
                t.presentation.reaction_preview = serde_json::from_str(&raw).ok();
            }
            let saved_msgs = self.messages_for(&t.id, limit_per_thread)?;
            for sm in saved_msgs {
                let time = if sm.ts > 0 {
                    crate::clock::message_time(sm.ts)
                } else {
                    crate::ui::clock()
                };
                let mut sticker_id = None;
                let mut sticker_cate_id = None;
                let mut sticker_type = None;
                let mut reply_to_id = None;
                let mut reply_to_sender = None;
                let mut reply_to_text = None;
                let mut reactions = Vec::new();
                let mut is_pinned = false;
                let mut is_starred = false;
                let mut link_url = None;
                let mut link_title = None;
                let mut link_desc = None;
                let mut link_thumb = None;
                let mut link_src = None;
                let mut saved_state = None;
                if !sm.raw_json.is_empty() {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&sm.raw_json) {
                        saved_state = val.get("message_state").cloned();
                        sticker_id = val
                            .get("stickerId")
                            .or_else(|| val.get("id"))
                            .and_then(|v| v.as_u64());
                        sticker_cate_id = val
                            .get("cateId")
                            .or_else(|| val.get("catId"))
                            .and_then(|v| v.as_u64());
                        sticker_type = val.get("type").and_then(|v| v.as_u64()).map(|n| n as u32);
                        reply_to_id = val
                            .get("reply_to_id")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        reply_to_sender = val
                            .get("reply_to_sender")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        reply_to_text = val
                            .get("reply_to_text")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        if let Some(arr) = val.get("reactions").and_then(|v| v.as_array()) {
                            reactions = arr
                                .iter()
                                .filter_map(|v| v.as_str().map(str::to_string))
                                .collect();
                        }
                        is_pinned = val
                            .get("is_pinned")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        is_starred = val
                            .get("is_starred")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        link_url = val
                            .get("link_url")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        link_title = val
                            .get("link_title")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        link_desc = val
                            .get("link_desc")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        link_thumb = val
                            .get("link_thumb")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        link_src = val
                            .get("link_src")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                    }
                }
                let mut cm = ChatMessage {
                    id: sm.msg_id,
                    sender: if !sm.sender_name.is_empty() {
                        sm.sender_name
                    } else {
                        sm.sender_id
                    },
                    text: sm.text,
                    time,
                    is_outgoing: sm.is_outgoing,
                    ts: sm.ts,
                    sticker_id,
                    sticker_cate_id,
                    sticker_type,
                    reply_to_id,
                    reply_to_sender,
                    reply_to_text,
                    reactions,
                    is_pinned,
                    is_starred,
                    link_url,
                    link_title,
                    link_desc,
                    link_thumb,
                    link_src,

                    ..Default::default()
                };
                if let Some(state) = saved_state {
                    if cm.restore_state(&state).is_err() {
                        crate::logger::log_error(
                            "history metadata",
                            "Invalid optional message state; retained base message",
                        );
                    }
                }
                if crate::message_lifecycle::expired(&cm, crate::login::unix_now() * 1000) {
                    continue;
                }
                if cm.text == "[Tin nhắn đã thu hồi]" {
                    cm.mark_recalled();
                }
                if let (Some(marks), Some(mark)) =
                    (marks.as_deref_mut(), crate::chat::message_fingerprint(&cm))
                {
                    marks.insert((t.id.clone(), cm.id.clone()), mark);
                }
                if let Some(existing) = t
                    .messages
                    .iter_mut()
                    .find(|existing| crate::chat::same_message(existing, &cm))
                {
                    existing.merge_reaction_metadata(&cm);
                    if existing.sticker_id.is_none() && cm.sticker_id.is_some() {
                        existing.sticker_id = cm.sticker_id;
                        existing.sticker_cate_id = cm.sticker_cate_id;
                        existing.sticker_type = cm.sticker_type;
                    }
                    if existing.reply_to_text.is_none() && cm.reply_to_text.is_some() {
                        existing.reply_to_id = cm.reply_to_id;
                        existing.reply_to_sender = cm.reply_to_sender;
                        existing.reply_to_text = cm.reply_to_text;
                    }
                    if existing.link_url.is_none() && cm.link_url.is_some() {
                        existing.link_url = cm.link_url;
                        existing.link_title = cm.link_title;
                        existing.link_desc = cm.link_desc;
                        existing.link_thumb = cm.link_thumb;
                        existing.link_src = cm.link_src;
                        if !cm.text.is_empty() {
                            existing.text = cm.text;
                        }
                    }
                    if existing.reactions.is_empty() && !cm.reactions.is_empty() {
                        existing.reactions = cm.reactions;
                    }
                    existing.is_pinned = existing.is_pinned || cm.is_pinned;
                    existing.is_starred = existing.is_starred || cm.is_starred;
                } else {
                    if crate::message_view::is_call_log(&cm) {
                        if t.presentation.last_message_id == cm.id {
                            t.presentation.last_message_ts = 0;
                            t.presentation.last_message_id.clear();
                            t.preview.clear();
                            t.time.clear();
                        }
                        continue;
                    }
                    t.messages.push(cm);
                    loaded_count += 1;
                }
            }
            crate::message_view::remove_call_logs(t);
            if !t.messages.is_empty() {
                t.messages.sort_by_key(|m| m.ts);
                if let Some(newest) = t.messages.last() {
                    if !newest.text.is_empty() {
                        t.preview = newest.text.replace('\n', " ");
                        t.time = newest.time.clone();
                    }
                }
            }
        }
        crate::chat::deduplicate_threads(threads, None);
        Ok(loaded_count)
    }
}

impl From<&ChatMessage> for StoredMessage {
    fn from(m: &ChatMessage) -> Self {
        let recalled = (m.text == "[Tin nhắn đã thu hồi]"
            || m.kind == crate::message_model::MessageKind::Recall)
            .then(|| {
                let mut message = m.clone();
                message.mark_recalled();
                message
            });
        let m = recalled.as_ref().unwrap_or(m);
        let mut meta = serde_json::Map::new();
        let msg_type = if let Some(sid) = m.sticker_id {
            let cid = m.sticker_cate_id.unwrap_or(0);
            let st = m.sticker_type.unwrap_or(7);
            meta.insert("stickerId".into(), serde_json::json!(sid));
            meta.insert("cateId".into(), serde_json::json!(cid));
            meta.insert("type".into(), serde_json::json!(st));
            "chat.sticker".to_string()
        } else {
            "chat.text".to_string()
        };
        if let Some(ref rid) = m.reply_to_id {
            meta.insert("reply_to_id".into(), serde_json::json!(rid));
        }
        if let Some(ref rsender) = m.reply_to_sender {
            meta.insert("reply_to_sender".into(), serde_json::json!(rsender));
        }
        if let Some(ref rtext) = m.reply_to_text {
            meta.insert("reply_to_text".into(), serde_json::json!(rtext));
        }
        if !m.reactions.is_empty() {
            meta.insert("reactions".into(), serde_json::json!(m.reactions));
        }
        if m.is_pinned {
            meta.insert("is_pinned".into(), serde_json::json!(true));
        }
        if m.is_starred {
            meta.insert("is_starred".into(), serde_json::json!(true));
        }
        if let Some(lurl) = &m.link_url {
            meta.insert("link_url".into(), serde_json::json!(lurl));
        }
        if let Some(ltitle) = &m.link_title {
            meta.insert("link_title".into(), serde_json::json!(ltitle));
        }
        if let Some(ldesc) = &m.link_desc {
            meta.insert("link_desc".into(), serde_json::json!(ldesc));
        }
        if let Some(lthumb) = &m.link_thumb {
            meta.insert("link_thumb".into(), serde_json::json!(lthumb));
        }
        if let Some(lsrc) = &m.link_src {
            meta.insert("link_src".into(), serde_json::json!(lsrc));
        }
        meta.insert("message_state".into(), m.durable_state());
        let raw_json = if meta.is_empty() {
            String::new()
        } else {
            serde_json::Value::Object(meta).to_string()
        };
        Self {
            msg_id: if !m.id.is_empty() {
                m.id.clone()
            } else {
                format!("msg_{}", m.ts)
            },
            thread_id: String::new(),
            sender_id: if m.is_outgoing {
                "0".into()
            } else {
                m.sender.clone()
            },
            sender_name: m.sender.clone(),
            text: m.text.clone(),
            msg_type,
            ts: m.ts,
            is_outgoing: m.is_outgoing,
            raw_json,
        }
    }
}

/// Persist threads and their messages to SQLite store, returning the count of newly inserted messages.
pub fn persist_threads(store: &mut MessageStore, threads: &[Thread]) -> Result<usize> {
    persist_snapshot(store, threads, false).map(|c| c.inserted)
}

/// Outcome of persisting a snapshot, message by message.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PersistCounts {
    /// New rows.
    pub inserted: usize,
    /// Already stored (deduplicated or merged into the existing row).
    pub existing: usize,
    /// Refused because the user deleted that message on this device.
    pub deleted: usize,
}

/// Like [`persist_threads`], also reporting duplicates and delete-marker refusals.
/// # Errors
/// Returns database failures and rolls back the entire snapshot.
pub fn persist_threads_counted(
    store: &mut MessageStore,
    threads: &[Thread],
) -> Result<PersistCounts> {
    persist_snapshot(store, threads, false)
}

/// Persist an explicit UI edit, including local-only pin/star flags.
/// # Errors
/// Returns database failures and rolls back the entire snapshot.
pub fn persist_local_threads(store: &mut MessageStore, threads: &[Thread]) -> Result<usize> {
    persist_snapshot(store, threads, true).map(|c| c.inserted)
}
fn persist_snapshot(
    store: &mut MessageStore,
    threads: &[Thread],
    local_flags: bool,
) -> Result<PersistCounts> {
    let transaction = store.conn.unchecked_transaction()?;
    let mut counts = PersistCounts::default();
    for t in threads {
        let last_ts = t.messages.iter().map(|m| m.ts).max().unwrap_or(0);
        let st = StoredThread {
            thread_id: t.id.clone(),
            name: t.name.clone(),
            is_group: t.is_group,
            last_ts,
        };
        store.upsert_thread(&st)?;

        for m in &t.messages {
            let mut sm = StoredMessage::from(m);
            if m.is_outgoing && crate::chat::server_message_id(m).is_some() {
                let mut accepted = m.clone();
                accepted.delivery = accepted
                    .delivery
                    .merge(crate::message_model::DeliveryState::ServerAccepted);
                accepted.outgoing_job = None;
                sm = StoredMessage::from(&accepted);
            }
            sm.thread_id = t.id.clone();
            if sm.msg_id.is_empty() {
                sm.msg_id = format!("{}_{}", t.id, sm.ts);
            }
            store.reconcile_send_identity(m, &mut sm)?;
            if store.is_deleted_message(&sm)? {
                counts.deleted += 1;
                continue;
            }
            if store.upsert_message(&sm)? {
                counts.inserted += 1;
            } else if store.is_deleted_message(&sm)? {
                counts.deleted += 1;
            } else {
                counts.existing += 1;
            }
            if local_flags {
                store.conn.execute("UPDATE messages SET raw_json=json_set(raw_json,'$.is_starred',json(?1),'$.is_pinned',json(?2)) WHERE thread_id=?3 AND msg_id=?4",
                    params![if m.is_starred {"true"} else {"false"},if m.is_pinned {"true"} else {"false"},t.id,sm.msg_id])?;
            }
        }
    }
    transaction.commit()?;
    Ok(counts)
}

/// Convenience helper: open `zalo_history.db` (or `path`) and hydrate `threads` with saved history.
pub fn load_history_into_threads(
    path: impl AsRef<std::path::Path>,
    threads: &mut Vec<Thread>,
    limit_per_thread: usize,
) {
    if let Ok(store) = MessageStore::open(path) {
        let _ = store.hydrate_threads(threads, limit_per_thread);
    }
}

impl MessageStore {
    /// Load resumable upload state from persistence.
    /// # Errors
    /// Returns SQLite errors.
    pub fn load_upload_state(&self, upload_id: &str) -> Result<Option<crate::upload::ResumableUploadState>> {
        use rusqlite::OptionalExtension;
        let row = self.conn.query_row(
            "SELECT upload_id, file_name, total_size, chunk_size, total_chunks, completed_chunks FROM upload_sessions WHERE upload_id=?1",
            [upload_id],
            |r| {
                let completed_str: String = r.get(5)?;
                let completed_chunks: Vec<usize> = serde_json::from_str(&completed_str)
                    .unwrap_or_default();
                Ok(crate::upload::ResumableUploadState {
                    upload_id: r.get(0)?,
                    file_name: r.get(1)?,
                    total_size: r.get(2)?,
                    chunk_size: r.get(3)?,
                    total_chunks: r.get(4)?,
                    completed_chunks,
                })
            },
        ).optional()?;
        Ok(row)
    }

    /// Save or update resumable upload state.
    /// # Errors
    /// Returns SQLite or JSON serialization errors.
    pub fn save_upload_state(&self, state: &crate::upload::ResumableUploadState) -> Result<()> {
        let completed_json = serde_json::to_string(&state.completed_chunks)?;
        self.conn.execute(
            "INSERT INTO upload_sessions(upload_id, file_name, total_size, chunk_size, total_chunks, completed_chunks, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(upload_id) DO UPDATE SET completed_chunks=excluded.completed_chunks, updated_at=excluded.updated_at",
            params![
                &state.upload_id,
                &state.file_name,
                state.total_size,
                state.chunk_size,
                state.total_chunks,
                completed_json,
                crate::login::unix_now()
            ],
        )?;
        Ok(())
    }

    /// Delete completed or abandoned upload session.
    /// # Errors
    /// Returns SQLite errors.
    pub fn delete_upload_state(&self, upload_id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM upload_sessions WHERE upload_id=?1", [upload_id])?;
        Ok(())
    }

    /// List all active upload sessions (for recovery after restart).
    /// # Errors
    /// Returns SQLite errors.
    pub fn list_upload_sessions(&self) -> Result<Vec<crate::upload::ResumableUploadState>> {
        let mut stmt = self.conn.prepare(
            "SELECT upload_id, file_name, total_size, chunk_size, total_chunks, completed_chunks
             FROM upload_sessions ORDER BY updated_at DESC"
        )?;
        let rows = stmt.query_map([], |r| {
            let completed_str: String = r.get(5)?;
            let completed_chunks: Vec<usize> = serde_json::from_str(&completed_str)
                .unwrap_or_default();
            Ok(crate::upload::ResumableUploadState {
                upload_id: r.get(0)?,
                file_name: r.get(1)?,
                total_size: r.get(2)?,
                chunk_size: r.get(3)?,
                total_chunks: r.get(4)?,
                completed_chunks,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod upload_persistence_tests {
    use super::*;

    #[test]
    fn upload_state_roundtrip_and_delete() {
        let store = MessageStore::open(":memory:").unwrap();
        let mut state = crate::upload::ResumableUploadState::new(
            "upload_abc".into(),
            "large.zip".into(),
            10_000_000,
            4_000_000,
        );

        // Save initial state
        store.save_upload_state(&state).unwrap();
        let loaded = store.load_upload_state("upload_abc").unwrap().unwrap();
        assert_eq!(loaded, state);

        // Update state with progress
        state.mark_chunk_complete(0);
        state.mark_chunk_complete(2);
        store.save_upload_state(&state).unwrap();

        let reloaded = store.load_upload_state("upload_abc").unwrap().unwrap();
        assert_eq!(reloaded.completed_chunks, vec![0, 2]);
        assert!(!reloaded.is_complete());

        // Complete and delete
        state.mark_chunk_complete(1);
        assert!(state.is_complete());
        store.delete_upload_state("upload_abc").unwrap();
        assert!(store.load_upload_state("upload_abc").unwrap().is_none());
    }

    #[test]
    fn list_upload_sessions_returns_all_active() {
        let store = MessageStore::open(":memory:").unwrap();

        let state1 = crate::upload::ResumableUploadState::new(
            "up1".into(), "file1.bin".into(), 1000, 500,
        );
        let state2 = crate::upload::ResumableUploadState::new(
            "up2".into(), "file2.bin".into(), 2000, 500,
        );

        store.save_upload_state(&state1).unwrap();
        store.save_upload_state(&state2).unwrap();

        let sessions = store.list_upload_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.iter().any(|s| s.upload_id == "up1"));
        assert!(sessions.iter().any(|s| s.upload_id == "up2"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_baseline_excludes_unsaved_server_previews_and_preserves_thread_edits() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let message = |id: &str| ChatMessage {
            id: id.into(),
            text: id.into(),
            ts: 1000,
            ..Default::default()
        };
        let saved = Thread {
            id: "thread".into(),
            name: "Old name".into(),
            messages: vec![message("stored")],
            ..Default::default()
        };
        persist_threads(&mut store, &[saved]).unwrap();
        let mut list = vec![Thread {
            id: "thread".into(),
            name: "New name".into(),
            messages: vec![message("server-preview")],
            ..Default::default()
        }];
        let baseline = store.hydrate_refresh(&mut list, 20).unwrap();
        assert!(!baseline.contains_key(&("thread".into(), "server-preview".into())));
        let changes = crate::chat::changed_since(&list, &baseline);
        assert_eq!(changes[0].messages.len(), 1);
        assert_eq!(changes[0].messages[0].id, "server-preview");
        persist_threads(&mut store, &changes).unwrap();
        assert_eq!(store.messages_for("thread", 20).unwrap().len(), 2);
        assert_eq!(store.all_threads().unwrap()[0].name, "New name");
        // A metadata-only refresh must still retain a renamed thread.
        let baseline = store.hydrate_refresh(&mut list, 20).unwrap();
        list[0].name = "Renamed again".into();
        let changes = crate::chat::changed_since(&list, &baseline);
        assert!(changes[0].messages.is_empty());
        persist_threads(&mut store, &changes).unwrap();
        assert_eq!(store.all_threads().unwrap()[0].name, "Renamed again");
    }

    #[test]
    fn warm_refresh_writes_no_message_rows_but_keeps_metadata_and_real_deltas() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let source: Vec<_> = (0..100)
            .map(|n| Thread {
                id: format!("thread-{n}"),
                name: format!("Name {n}"),
                messages: (0..20)
                    .map(|m| ChatMessage {
                        id: format!("{m}"),
                        text: format!("Message {m}"),
                        sender: "Alice".into(),
                        ts: 1_800_000_000_000 + m,
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            })
            .collect();
        persist_threads(&mut store, &source).unwrap();
        let mut loaded: Vec<_> = source
            .iter()
            .map(|t| Thread {
                id: t.id.clone(),
                ..Default::default()
            })
            .collect();
        let marks = store.hydrate_refresh(&mut loaded, 20).unwrap();
        store.conn.execute_batch("CREATE TABLE refresh_writes(kind TEXT); CREATE TEMP TRIGGER refresh_updated AFTER UPDATE ON messages BEGIN INSERT INTO refresh_writes VALUES('update'); END; CREATE TEMP TRIGGER refresh_inserted AFTER INSERT ON messages BEGIN INSERT INTO refresh_writes VALUES('insert'); END;").unwrap();
        let mut before = Vec::new();
        let mut after = Vec::new();
        for _ in 0..5 {
            let start = std::time::Instant::now();
            persist_threads(&mut store, &loaded).unwrap();
            before.push(start.elapsed().as_secs_f64() * 1000.0);
            let writes: i64 = store
                .conn
                .query_row("SELECT count(*) FROM refresh_writes", [], |r| r.get(0))
                .unwrap();
            assert_eq!(writes, 2000);
            store
                .conn
                .execute("DELETE FROM refresh_writes", [])
                .unwrap();
            let start = std::time::Instant::now();
            let changed = crate::chat::changed_since(&loaded, &marks);
            assert!(changed.iter().all(|t| t.messages.is_empty()));
            persist_threads(&mut store, &changed).unwrap();
            after.push(start.elapsed().as_secs_f64() * 1000.0);
            let writes: i64 = store
                .conn
                .query_row("SELECT count(*) FROM refresh_writes", [], |r| r.get(0))
                .unwrap();
            assert_eq!(writes, 0);
        }
        before.sort_by(f64::total_cmp);
        after.sort_by(f64::total_cmp);
        println!(
            "{}",
            serde_json::json!({"task":"warm_refresh_durable_writes", "environment":"Linux x86_64 debug SQLite memory", "messages":2000,"iterations":5,
            "before":{"writes":2000,"min_ms":before[0],"avg_ms":before.iter().sum::<f64>()/5.0,"p50_ms":before[2],"p95_ms":before[4],"p99_ms":before[4]},
            "after":{"writes":0,"min_ms":after[0],"avg_ms":after.iter().sum::<f64>()/5.0,"p50_ms":after[2],"p95_ms":after[4],"p99_ms":after[4]}})
        );
        loaded[0].name = "Renamed".into();
        loaded[0].messages[0].mark_recalled();
        let changed = crate::chat::changed_since(&loaded, &marks);
        persist_threads(&mut store, &changed).unwrap();
        let writes: i64 = store
            .conn
            .query_row("SELECT count(*) FROM refresh_writes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(writes, 1);
        assert_eq!(
            store.messages_for("thread-0", 20).unwrap()[0].text,
            "[Tin nhắn đã thu hồi]"
        );
        assert!(store
            .all_threads()
            .unwrap()
            .iter()
            .any(|t| t.thread_id == "thread-0" && t.name == "Renamed"));
    }
    #[test]
    fn unchanged_deleted_rows_leave_the_refreshed_view() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let message = ChatMessage {
            id: "stored".into(),
            text: "old".into(),
            ts: 1000,
            ..Default::default()
        };
        persist_threads(
            &mut store,
            &[Thread {
                id: "thread".into(),
                messages: vec![message.clone()],
                ..Default::default()
            }],
        )
        .unwrap();
        let mut list = vec![Thread {
            id: "thread".into(),
            ..Default::default()
        }];
        let baseline = store.hydrate_refresh(&mut list, 20).unwrap();
        assert!(crate::chat::changed_since(&list, &baseline)[0]
            .messages
            .is_empty());
        store.delete_chat_message("thread", &message).unwrap();
        store.filter_deleted(&mut list).unwrap();
        assert!(list[0].messages.is_empty());
        persist_threads(&mut store, &crate::chat::changed_since(&list, &baseline)).unwrap();
        assert!(store.messages_for("thread", 20).unwrap().is_empty());
    }
    #[test]
    fn opening_one_thread_does_not_load_other_histories() {
        let store = MessageStore::open(":memory:").unwrap();
        for id in ["thread-1", "thread-2"] {
            store
                .upsert_thread(&StoredThread {
                    thread_id: id.into(),
                    name: format!("Name {id}"),
                    is_group: true,
                    last_ts: 1,
                })
                .unwrap();
        }
        let mut selected = vec![Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "thread-1".into(),
            name: String::new(),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: true,
            messages: Vec::new(),
        }];
        store.hydrate_selected_threads(&mut selected, 500).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "Name thread-1");
    }

    #[test]
    fn scoped_receipt_finds_its_row_after_many_message_id_collisions() {
        let store = MessageStore::open(":memory:").unwrap();
        for index in 0..20 {
            store
                .upsert_message(&StoredMessage {
                    msg_id: "shared-id".into(),
                    thread_id: format!("thread-{index:02}"),
                    sender_id: "me".into(),
                    sender_name: String::new(),
                    text: String::new(),
                    msg_type: "chat.text".into(),
                    ts: 1,
                    is_outgoing: true,
                    raw_json: serde_json::json!({"message_state":{"delivery":"server_accepted"}})
                        .to_string(),
                })
                .unwrap();
        }
        let changed = store
            .apply_receipts(&[crate::presence::Receipt {
                thread_id: "thread-19".into(),
                is_group: false,
                msg_ids: vec!["shared-id".into()],
                read: true,
            }])
            .unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].0, "thread-19");
        assert!(store
            .apply_receipts(&[crate::presence::Receipt {
                thread_id: String::new(),
                is_group: false,
                msg_ids: vec!["shared-id".into()],
                read: true,
            }])
            .unwrap()
            .is_empty());
    }
    #[test]
    fn receipts_only_raise_outgoing_delivery_state() {
        use crate::message_model::DeliveryState;
        let store = MessageStore::open(":memory:").unwrap();
        for (id, outgoing, delivery) in [
            ("m1", true, "server_accepted"),
            ("m2", false, "unknown"),
            ("m3", true, "read"),
        ] {
            store
                .upsert_message(&StoredMessage {
                    msg_id: id.into(),
                    thread_id: "t".into(),
                    sender_id: "me".into(),
                    sender_name: String::new(),
                    text: "x".into(),
                    msg_type: "chat.text".into(),
                    ts: 1,
                    is_outgoing: outgoing,
                    raw_json: serde_json::json!({"message_state":{"delivery":delivery}})
                        .to_string(),
                })
                .unwrap();
        }
        let receipt = |id: &str, read: bool| crate::presence::Receipt {
            thread_id: "t".into(),
            is_group: false,
            msg_ids: vec![id.into()],
            read,
        };
        let changed = store
            .apply_receipts(&[
                receipt("m1", false),
                receipt("m2", true),
                receipt("m3", false),
            ])
            .unwrap();
        assert_eq!(
            changed,
            vec![("t".into(), "m1".into(), DeliveryState::Delivered)]
        );
        let changed = store.apply_receipts(&[receipt("m1", true)]).unwrap();
        assert_eq!(changed[0].2, DeliveryState::Read);
        assert!(store
            .apply_receipts(&[receipt("m1", false)])
            .unwrap()
            .is_empty());
    }
    #[test]
    fn limited_history_keeps_newest_rows_and_uses_thread_index() {
        let store = MessageStore::open(":memory:").unwrap();
        for ts in 1..=10 {
            store
                .upsert_message(&StoredMessage {
                    msg_id: format!("m{ts}"),
                    thread_id: "thread-1".into(),
                    sender_id: "user-1".into(),
                    sender_name: "Example".into(),
                    text: "hello".into(),
                    msg_type: "chat.text".into(),
                    ts,
                    is_outgoing: false,
                    raw_json: String::new(),
                })
                .unwrap();
        }
        assert_eq!(
            store
                .messages_for("thread-1", 3)
                .unwrap()
                .iter()
                .map(|m| m.ts)
                .collect::<Vec<_>>(),
            vec![8, 9, 10]
        );
        let mut selected = vec![Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "thread-1".into(),
            name: String::new(),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: false,
            messages: Vec::new(),
        }];
        store.hydrate_selected_threads(&mut selected, 3).unwrap();
        assert_eq!(
            selected[0].messages.len(),
            3,
            "Distinct IDs with repeated text must survive SQLite hydration"
        );
        let plan: String = store.conn.query_row("EXPLAIN QUERY PLAN SELECT msg_id FROM messages WHERE thread_id=?1 ORDER BY ts DESC LIMIT 3", ["thread-1"],|row|row.get(3)).unwrap();
        assert!(plan.contains("idx_thread_ts"), "{plan}");
    }
    #[test]
    fn newer_database_is_rejected_without_writes() {
        let path = std::env::temp_dir().join(format!(
            "ctdoz-newer-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        drop(conn);
        assert!(MessageStore::open(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn test_insert_twice_returns_false() {
        let store = MessageStore::open(":memory:").unwrap();
        let msg = StoredMessage {
            msg_id: "m_1001".into(),
            thread_id: "thread_1".into(),
            sender_id: "u_1".into(),
            sender_name: "Alice".into(),
            text: "Xin chao".into(),
            msg_type: "chat.text".into(),
            ts: 1729500000000,
            is_outgoing: false,
            raw_json: "".into(),
        };

        assert!(store.upsert_message(&msg).unwrap());
        assert!(!store.upsert_message(&msg).unwrap());
    }

    #[test]
    fn test_messages_for_returns_ordered_rows() {
        let store = MessageStore::open(":memory:").unwrap();
        let thread_id = "thread_order_test";

        let m1 = StoredMessage {
            msg_id: "m1".into(),
            thread_id: thread_id.into(),
            sender_id: "u1".into(),
            sender_name: "User".into(),
            text: "Msg 200".into(),
            msg_type: "chat.text".into(),
            ts: 200,
            is_outgoing: false,
            raw_json: "".into(),
        };
        let m2 = StoredMessage {
            msg_id: "m2".into(),
            thread_id: thread_id.into(),
            sender_id: "u1".into(),
            sender_name: "User".into(),
            text: "Msg 100".into(),
            msg_type: "chat.text".into(),
            ts: 100,
            is_outgoing: false,
            raw_json: "".into(),
        };
        let m3 = StoredMessage {
            msg_id: "m3".into(),
            thread_id: thread_id.into(),
            sender_id: "u1".into(),
            sender_name: "User".into(),
            text: "Msg 300".into(),
            msg_type: "chat.text".into(),
            ts: 300,
            is_outgoing: false,
            raw_json: "".into(),
        };

        store.upsert_message(&m1).unwrap();
        store.upsert_message(&m2).unwrap();
        store.upsert_message(&m3).unwrap();

        let msgs = store.messages_for(thread_id, 10).unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].ts, 100);
        assert_eq!(msgs[0].msg_id, "m2");
        assert_eq!(msgs[1].ts, 200);
        assert_eq!(msgs[1].msg_id, "m1");
        assert_eq!(msgs[2].ts, 300);
        assert_eq!(msgs[2].msg_id, "m3");
    }

    #[test]
    fn test_persist_threads_and_upsert_thread() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let thread = Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: "t_test".into(),
            name: "Test Thread".into(),
            preview: "Preview".into(),
            time: "10:00".into(),
            avatar: None,
            is_group: false,
            messages: vec![
                ChatMessage {
                    id: "pt_m1".into(),
                    sender: "Alice".into(),
                    text: "Hello".into(),
                    time: "10:00".into(),
                    is_outgoing: false,
                    ts: 1000,
                    ..Default::default()
                },
                ChatMessage {
                    id: "pt_m2".into(),
                    sender: "Tôi".into(),
                    text: "Hi Alice".into(),
                    time: "10:01".into(),
                    is_outgoing: true,
                    ts: 2000,
                    ..Default::default()
                },
            ],
        };

        let count1 = persist_threads(&mut store, std::slice::from_ref(&thread)).unwrap();
        assert_eq!(count1, 2);

        // Second persist returns 0 newly inserted
        let count2 = persist_threads(&mut store, &[thread]).unwrap();
        assert_eq!(count2, 0);

        let msgs = store.messages_for("t_test", 10).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].text, "Hello");
        assert_eq!(msgs[1].text, "Hi Alice");

        // Hydrate into an empty thread list
        let mut hydrated: Vec<Thread> = Vec::new();
        let loaded = store.hydrate_threads(&mut hydrated, 50).unwrap();
        assert_eq!(loaded, 2);
        assert_eq!(hydrated.len(), 1);
        assert_eq!(hydrated[0].id, "t_test");
        assert_eq!(hydrated[0].name, "Test Thread");
        assert_eq!(hydrated[0].messages.len(), 2);
        assert_eq!(hydrated[0].preview, "Hi Alice");
    }
}

#[cfg(test)]
mod transaction_tests {
    use super::*;
    #[test]
    fn failed_snapshot_rolls_back_preceding_rows() {
        let mut store = MessageStore::open(":memory:").unwrap();
        store.conn.execute_batch("CREATE TRIGGER reject_fixture BEFORE INSERT ON threads WHEN NEW.thread_id = 'bad' BEGIN SELECT RAISE(ABORT, 'fixture failure'); END;").unwrap();
        let thread = |id: &str| Thread {
            presentation: Default::default(),
            unread: Default::default(),
            id: id.into(),
            name: id.into(),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: false,
            messages: Vec::new(),
        };
        assert!(persist_threads(&mut store, &[thread("good"), thread("bad")]).is_err());
        assert!(store.all_threads().unwrap().is_empty());
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    fn message(thread_id: &str) -> StoredMessage {
        StoredMessage {
            msg_id: "same-id".into(),
            thread_id: thread_id.into(),
            sender_id: "sender".into(),
            sender_name: "Same name".into(),
            text: thread_id.into(),
            msg_type: "text".into(),
            ts: 1,
            is_outgoing: false,
            raw_json: "{}".into(),
        }
    }
    #[test]
    fn same_message_id_is_scoped_to_thread_and_delete_does_not_cross_threads() {
        let mut store = MessageStore::open(":memory:").unwrap();
        assert!(store.upsert_message(&message("a")).unwrap());
        assert!(store.upsert_message(&message("b")).unwrap());
        let mut update = message("a");
        update.text = "edited".into();
        assert!(!store.upsert_message(&update).unwrap());
        assert_eq!(store.messages_for("a", 10).unwrap()[0].text, "edited");
        assert_eq!(store.messages_for("b", 10).unwrap()[0].text, "b");
        let mut simultaneous = message("a");
        simultaneous.msg_id = "other-id".into();
        assert!(store.upsert_message(&simultaneous).unwrap());
        store.delete_message("a", "same-id", 1).unwrap();
        let remaining = store.messages_for("a", 10).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].msg_id, "other-id");
        assert_eq!(store.messages_for("b", 10).unwrap().len(), 1);
    }
    #[test]
    fn old_pages_do_not_erase_group_name_or_recency() {
        let store = MessageStore::open(":memory:").unwrap();
        let mut thread = StoredThread {
            thread_id: "123".into(),
            name: "Nhóm có tên".into(),
            is_group: true,
            last_ts: 99,
        };
        store.upsert_thread(&thread).unwrap();
        thread.name = "Nhóm 123".into();
        thread.last_ts = 1;
        store.upsert_thread(&thread).unwrap();
        thread.name.clear();
        store.upsert_thread(&thread).unwrap();
        let saved = store.all_threads().unwrap();
        assert_eq!(saved[0].name, "Nhóm có tên");
        assert_eq!(saved[0].last_ts, 99);
    }
    #[test]
    fn migrates_legacy_database_without_losing_messages() {
        let dir = std::env::temp_dir().join(format!(
            "ctdoz-migration-{}-{}",
            std::process::id(),
            crate::param_cipher::random_hex(8)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("history.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE messages (msg_id TEXT PRIMARY KEY,thread_id TEXT NOT NULL,sender_id TEXT,sender_name TEXT,text TEXT,msg_type TEXT,ts INTEGER,is_outgoing INTEGER,raw_json TEXT);
            INSERT INTO messages VALUES ('same-id','a','','','old','text',1,0,'{}');").unwrap();
        drop(conn);
        let store = MessageStore::open(&path).unwrap();
        assert_eq!(store.messages_for("a", 10).unwrap()[0].text, "old");
        assert!(store.upsert_message(&message("b")).unwrap());
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod durable_state_tests {
    use super::*;
    fn thread(message: ChatMessage) -> Thread {
        Thread {
            id: "thread-1".into(),
            name: "Example".into(),
            is_group: true,
            preview: String::new(),
            time: String::new(),
            avatar: None,
            presentation: Default::default(),
            unread: Default::default(),
            messages: vec![message],
        }
    }
    fn message() -> ChatMessage {
        ChatMessage {
            id: "msg-1".into(),
            text: "old-private-content".into(),
            ts: 1000,
            kind: crate::message_model::MessageKind::Photo,
            delivery: crate::message_model::DeliveryState::Read,
            wire_ref: crate::message_model::WireMessageRef::from_raw(
                &serde_json::json!({"msgId":"18446744073709550000","content":"old-private-content"}),
            ),
            link_url: Some("https://example.invalid/private.png".into()),
            is_starred: true,
            ..Default::default()
        }
    }
    #[test]
    fn metadata_roundtrip_keeps_large_wire_ids_receipts_and_local_flags() {
        let mut store = MessageStore::open(":memory:").unwrap();
        persist_local_threads(&mut store, &[thread(message())]).unwrap();
        persist_threads(
            &mut store,
            &[thread(ChatMessage {
                id: "msg-1".into(),
                text: "old-private-content".into(),
                ts: 1000,
                ..Default::default()
            })],
        )
        .unwrap();
        let mut loaded = Vec::new();
        store.hydrate_threads(&mut loaded, 20).unwrap();
        let restored = &loaded[0].messages[0];
        assert_eq!(restored.delivery, crate::message_model::DeliveryState::Read);
        assert_eq!(
            restored.wire_ref.server_id,
            Some(serde_json::json!("18446744073709550000"))
        );
        assert!(restored.is_starred);
        assert!(restored.link_url.is_some());
        let mut edited = loaded[0].clone();
        edited.messages[0].is_starred = false;
        persist_local_threads(&mut store, &[edited]).unwrap();
        let mut loaded = Vec::new();
        store.hydrate_threads(&mut loaded, 20).unwrap();
        assert!(
            !loaded[0].messages[0].is_starred,
            "Explicit local unstar must be applied"
        );
    }
    #[test]
    fn old_history_cannot_resurrect_recalled_content_or_attachments() {
        let mut store = MessageStore::open(":memory:").unwrap();
        persist_threads(&mut store, &[thread(message())]).unwrap();
        persist_threads(
            &mut store,
            &[thread(ChatMessage {
                id: "msg-1".into(),
                text: "[Tin nhắn đã thu hồi]".into(),
                ts: 1000,
                ..Default::default()
            })],
        )
        .unwrap();
        persist_threads(&mut store, &[thread(message())]).unwrap();
        let saved = store.messages_for("thread-1", 20).unwrap();
        assert_eq!(saved[0].text, "[Tin nhắn đã thu hồi]");
        assert!(!saved[0].raw_json.contains("old-private-content"));
        assert!(!saved[0].raw_json.contains("private.png"));
        let mut loaded = Vec::new();
        store.hydrate_threads(&mut loaded, 20).unwrap();
        assert_eq!(
            loaded[0].messages[0].kind,
            crate::message_model::MessageKind::Recall
        );
        assert!(loaded[0].messages[0].link_url.is_none());
    }
}

#[cfg(test)]
mod conversation_menu_tests {
    use super::*;
    #[test]
    fn preferences_are_isolated_and_survive_network_snapshot_writes() {
        let mut account_a = MessageStore::open(":memory:").unwrap();
        let account_b = MessageStore::open(":memory:").unwrap();
        let value = ConversationPreference {
            pinned: true,
            muted: true,
            hidden: true,
        };
        account_a
            .set_conversation_preference("group-fixture", value)
            .unwrap();
        persist_threads(
            &mut account_a,
            &[Thread {
                id: "group-fixture".into(),
                name: "Fixture".into(),
                presentation: Default::default(),
                preview: String::new(),
                time: String::new(),
                avatar: None,
                is_group: true,
                messages: Vec::new(),
                unread: Default::default(),
            }],
        )
        .unwrap();
        assert_eq!(
            account_a.conversation_preferences().unwrap()["group-fixture"],
            value
        );
        assert!(account_b.conversation_preferences().unwrap().is_empty());
        account_a
            .set_conversation_preference("group-fixture", ConversationPreference::default())
            .unwrap();
        assert_eq!(
            account_a.conversation_preferences().unwrap()["group-fixture"],
            ConversationPreference::default()
        );
    }
    #[test]
    fn mute_changes_keep_pin_and_hide_when_server_state_applied() {
        use crate::mute_sync::MuteChange;
        let store = MessageStore::open(":memory:").unwrap();
        store
            .set_conversation_preference(
                "1",
                ConversationPreference {
                    pinned: true,
                    muted: true,
                    hidden: true,
                },
            )
            .unwrap();
        assert!(!store.mute_sync_seeded().unwrap());
        let values = store
            .apply_mute_changes(
                &[
                    MuteChange {
                        thread_id: "1".into(),
                        muted: false,
                    },
                    MuteChange {
                        thread_id: "2".into(),
                        muted: true,
                    },
                ],
                true,
            )
            .unwrap();
        assert_eq!(
            values["1"],
            ConversationPreference {
                pinned: true,
                muted: false,
                hidden: true
            }
        );
        assert_eq!(
            values["2"],
            ConversationPreference {
                muted: true,
                ..Default::default()
            }
        );
        assert!(store.mute_sync_seeded().unwrap());
    }
    #[test]
    fn mute_changes_roll_back_when_one_identity_is_invalid() {
        use crate::mute_sync::MuteChange;
        let store = MessageStore::open(":memory:").unwrap();
        let changes = [
            MuteChange {
                thread_id: "1".into(),
                muted: true,
            },
            MuteChange {
                thread_id: " ".into(),
                muted: true,
            },
        ];
        assert!(store.apply_mute_changes(&changes, true).is_err());
        assert!(store.conversation_preferences().unwrap().is_empty());
        assert!(!store.mute_sync_seeded().unwrap());
    }
    #[test]
    fn clear_history_is_transactional_and_does_not_touch_another_thread() {
        let mut store = MessageStore::open(":memory:").unwrap();
        store.conn.execute_batch("INSERT INTO messages(thread_id,msg_id,text,is_outgoing,ts) VALUES('a','same','private-a',0,0),('b','same','private-b',0,0);
            INSERT INTO history_sync(thread_id,cursor) VALUES('a','cursor-a'),('b','cursor-b');
            CREATE TRIGGER block_clear BEFORE DELETE ON history_sync BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
        assert!(store.clear_thread_history("a").is_err());
        assert_eq!(store.messages_for("a", 10).unwrap().len(), 1);
        store
            .conn
            .execute_batch("DROP TRIGGER block_clear")
            .unwrap();
        store.clear_thread_history("a").unwrap();
        assert!(store.messages_for("a", 10).unwrap().is_empty());
        assert_eq!(store.messages_for("b", 10).unwrap().len(), 1);
        assert_eq!(
            store
                .conn
                .query_row(
                    "SELECT cursor FROM history_sync WHERE thread_id='b'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "cursor-b"
        );
    }
    #[test]
    fn schema_two_upgrade_preserves_composite_message_identity() {
        let directory = std::env::temp_dir().join(format!(
            "ctdoz-menu-migration-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("fixture.db");
        {
            let store = MessageStore::open(&path).unwrap();
            store.conn.execute_batch("INSERT INTO messages(thread_id,msg_id,text,is_outgoing,ts) VALUES('a','same','a',0,0),('b','same','b',0,0);DROP TABLE thread_preferences;PRAGMA user_version=2;").unwrap();
        }
        {
            let store = MessageStore::open(&path).unwrap();
            assert_eq!(
                store
                    .conn
                    .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
                    .unwrap(),
                11
            );
            assert!(store.conversation_preferences().unwrap().is_empty());
            assert_eq!(store.messages_for("a", 10).unwrap()[0].text, "a");
            assert_eq!(store.messages_for("b", 10).unwrap()[0].text, "b");
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[cfg(test)]
mod send_identity_tests {
    use super::*;
    fn thread(id: &str, client: &str) -> Thread {
        Thread {
            id: "thread-identity".into(),
            messages: vec![ChatMessage {
                id: id.into(),
                client_message_id: client.into(),
                text: "Repeated text".into(),
                is_outgoing: true,
                ts: 1000,
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    #[test]
    fn send_identity_is_idempotent_across_ack_push_order_and_rehydration() {
        for server_first in [false, true] {
            let mut store = MessageStore::open(":memory:").unwrap();
            let pending = thread("out_client-1", "client-1");
            let mut server = thread("server-1", "client-1");
            server.messages[0].ts = 2000;
            let (first, second) = if server_first {
                (&server, &pending)
            } else {
                (&pending, &server)
            };
            persist_threads(&mut store, std::slice::from_ref(first)).unwrap();
            persist_threads(&mut store, std::slice::from_ref(second)).unwrap();
            let saved = store.messages_for("thread-identity", 20).unwrap();
            assert_eq!(saved.len(), 1);
            assert_eq!(saved[0].msg_id, "server-1");
            assert_eq!(saved[0].ts, 2000);
            let mut hydrated = vec![Thread {
                id: "thread-identity".into(),
                ..Default::default()
            }];
            store.hydrate_selected_threads(&mut hydrated, 20).unwrap();
            assert_eq!(hydrated[0].messages.len(), 1);
            assert_eq!(hydrated[0].messages[0].id, "server-1");
        }
    }
    #[test]
    fn promotion_preserves_local_flags_and_rolls_back_with_snapshot() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let mut pending = thread("out_client-1", "client-1");
        pending.messages[0].is_starred = true;
        persist_local_threads(&mut store, &[pending]).unwrap();
        store.conn.execute_batch("CREATE TRIGGER reject_fixture BEFORE INSERT ON messages WHEN NEW.msg_id='fail' BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
        let mut batch = thread("server-1", "client-1");
        batch.messages.push(ChatMessage {
            id: "fail".into(),
            ..Default::default()
        });
        assert!(persist_threads(&mut store, &[batch]).is_err());
        assert_eq!(
            store.messages_for("thread-identity", 20).unwrap()[0].msg_id,
            "out_client-1"
        );
        persist_threads(&mut store, &[thread("server-1", "client-1")]).unwrap();
        let saved = store.messages_for("thread-identity", 20).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&saved[0].raw_json).unwrap()["is_starred"],
            true
        );
    }
    #[test]
    fn matching_text_or_ambiguous_client_ids_cannot_promote_saved_rows() {
        let mut store = MessageStore::open(":memory:").unwrap();
        persist_threads(
            &mut store,
            &[thread("out_a", "shared"), thread("out_b", "shared")],
        )
        .unwrap();
        persist_threads(&mut store, &[thread("server-a", "shared")]).unwrap();
        persist_threads(&mut store, &[thread("server-b", "other")]).unwrap();
        assert_eq!(store.messages_for("thread-identity", 20).unwrap().len(), 4);
    }
}

#[cfg(test)]
mod phone_resume_tests {
    use super::*;
    fn range(from_ts: i64, to_ts: i64) -> PhoneSyncRange {
        PhoneSyncRange {
            thread_id: "fake-thread".into(),
            from_ts,
            to_ts,
        }
    }
    #[test]
    fn coverage_is_explicit_atomic_and_reset_does_not_delete_messages() {
        let mut store = MessageStore::open(":memory:").unwrap();
        assert!(store.phone_sync_ranges().unwrap().is_empty());
        let thread = Thread {
            id: "fake-thread".into(),
            messages: vec![ChatMessage {
                id: "fake-message".into(),
                ts: 1000,
                ..Default::default()
            }],
            ..Default::default()
        };
        persist_threads(&mut store, &[thread]).unwrap();
        assert!(
            store.phone_sync_ranges().unwrap().is_empty(),
            "Stored rows do not prove complete history"
        );
        store
            .save_phone_sync_ranges(&[("oneone/fake".into(), range(100, 1000))])
            .unwrap();
        assert!(store
            .save_phone_sync_ranges(&[
                ("group/other".into(), range(0, 100)),
                ("oneone/fake".into(), range(2000, 3000))
            ])
            .is_err());
        assert_eq!(store.phone_sync_ranges().unwrap().len(), 1);
        store.reset_phone_sync_ranges().unwrap();
        assert_eq!(store.messages_for("fake-thread", 20).unwrap().len(), 1);
        store
            .save_phone_sync_ranges(&[("oneone/fake".into(), range(0, 1000))])
            .unwrap();
        store.clear_thread_history("fake-thread").unwrap();
        assert!(store.phone_sync_ranges().unwrap().is_empty());
    }
    #[test]
    fn schema_four_migrates_without_inventing_coverage_or_changing_messages() {
        let root = std::env::temp_dir().join(format!(
            "ctdoz-schema-four-{}",
            crate::param_cipher::random_hex(12)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("fixture.db");
        let mut store = MessageStore::open(&path).unwrap();
        persist_threads(
            &mut store,
            &[Thread {
                id: "migration-thread".into(),
                messages: vec![ChatMessage {
                    id: "migration-message".into(),
                    text: "preserve".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
        )
        .unwrap();
        store
            .conn
            .execute_batch("DROP TABLE phone_sync_ranges; PRAGMA user_version=4;")
            .unwrap();
        drop(store);
        let store = MessageStore::open(&path).unwrap();
        assert_eq!(
            store
                .conn
                .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            11
        );
        assert_eq!(
            store.messages_for("migration-thread", 20).unwrap()[0].text,
            "preserve"
        );
        assert!(store.phone_sync_ranges().unwrap().is_empty());
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn checkpoint_roundtrip_is_account_scoped_and_reopens_after_restart() {
        let root = std::env::temp_dir().join(format!(
            "ctdoz-phone-checkpoint-{}",
            crate::param_cipher::random_hex(12)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("account-a.db");
        MessageStore::open(&path)
            .unwrap()
            .save_phone_sync_ranges(&[("group/fake".into(), range(100, 1000))])
            .unwrap();
        let reopened = MessageStore::open(&path).unwrap();
        assert_eq!(
            reopened.phone_sync_ranges().unwrap()["group/fake"].from_ts,
            100
        );
        assert!(MessageStore::open(root.join("account-b.db"))
            .unwrap()
            .phone_sync_ranges()
            .unwrap()
            .is_empty());
        drop(reopened);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod coverage_migration_tests {
    use super::*;
    #[test]
    fn invalidates_old_coverage_once_but_preserves_messages_and_new_coverage() {
        let root = std::env::temp_dir().join(format!(
            "coverage-migration-{}",
            crate::chat::next_client_message_id()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("history.db");
        {
            let store = MessageStore::open(&path).unwrap();
            store.conn.execute_batch("INSERT INTO messages(thread_id,msg_id,text,ts,is_outgoing,sender_id,sender_name,msg_type,raw_json) VALUES('a','m','keep',0,0,'b','B','text','{}');INSERT INTO phone_sync_ranges VALUES('oneone/a','a',0,100);PRAGMA user_version=6;").unwrap();
        }
        {
            let store = MessageStore::open(&path).unwrap();
            assert!(store.phone_sync_ranges().unwrap().is_empty());
            assert_eq!(store.messages_for("a", 10).unwrap().len(), 1);
            store
                .save_phone_sync_ranges(&[(
                    "oneone/a".into(),
                    PhoneSyncRange {
                        thread_id: "a".into(),
                        from_ts: 0,
                        to_ts: 100,
                    },
                )])
                .unwrap();
        }
        {
            let store = MessageStore::open(&path).unwrap();
            assert_eq!(store.phone_sync_ranges().unwrap().len(), 1);
        }
        let _ = std::fs::remove_dir_all(root);
    }
}

#[cfg(test)]
mod media_projection_tests {
    use super::*;
    #[test]
    fn renewal_preserves_delivery_and_recall_and_survives_old_history_replay() {
        let store = MessageStore::open(":memory:").unwrap();
        let old = ChatMessage {
            id: "m".into(),
            text: "caption".into(),
            client_message_id: "11".into(),
            kind: crate::message_model::MessageKind::Photo,
            delivery: crate::message_model::DeliveryState::ServerAccepted,
            link_url: Some("https://photo.zdn.vn/old".into()),
            ..Default::default()
        };
        let mut row = StoredMessage::from(&old);
        row.thread_id = "thread".into();
        store.upsert_message(&row).unwrap();
        let mut updated = old.clone();
        let projection = crate::message_view::Media {
            kind: crate::message_view::MediaKind::Photo,
            original: Some("https://photo.zdn.vn/new".into()),
            preview: Some("https://photo.zdn.vn/new".into()),
            ..Default::default()
        };
        updated.link_url = projection.original.clone();
        updated.wire_ref.fields.insert(
            "renewed_media".into(),
            serde_json::to_value(projection).unwrap(),
        );
        store.update_media_metadata("thread", &updated).unwrap();
        store.upsert_message(&row).unwrap();
        let mut threads = vec![Thread {
            id: "thread".into(),
            ..Default::default()
        }];
        store.hydrate_selected_threads(&mut threads, 20).unwrap();
        let got = &threads[0].messages[0];
        assert_eq!(got.delivery, old.delivery);
        assert_eq!(got.link_url.as_deref(), Some("https://photo.zdn.vn/new"));
        assert_eq!(
            crate::message_view::media(got).original.as_deref(),
            Some("https://photo.zdn.vn/new")
        );
        store
            .conn
            .execute(
                "UPDATE messages SET text='[Tin nhắn đã thu hồi]' WHERE thread_id='thread'",
                [],
            )
            .unwrap();
        store.update_media_metadata("thread", &updated).unwrap();
        assert_eq!(
            store.messages_for("thread", 1).unwrap()[0].text,
            "[Tin nhắn đã thu hồi]"
        );
    }
}

#[cfg(test)]
mod draft_and_delete_tests {
    use super::*;
    fn message(id: &str, client: &str, outgoing: bool) -> ChatMessage {
        let mut message = ChatMessage {
            id: id.into(),
            client_message_id: client.into(),
            sender: "Người gửi thử".into(),
            text: "Nội dung phải bị xóa".into(),
            ts: 100,
            is_outgoing: outgoing,
            ..Default::default()
        };
        message.wire_ref.sender = Some(serde_json::json!("user-1"));
        message
    }
    fn row(thread: &str, message: &ChatMessage) -> StoredMessage {
        let mut row = StoredMessage::from(message);
        row.thread_id = thread.into();
        row
    }
    #[test]
    fn deleted_message_cannot_reappear_through_persist_hydrate_or_ui_snapshot() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let deleted = message("server-1", "client-1", false);
        store.upsert_message(&row("thread-a", &deleted)).unwrap();
        store.delete_chat_message("thread-a", &deleted).unwrap();
        assert!(!store.upsert_message(&row("thread-a", &deleted)).unwrap());
        assert!(store.messages_for("thread-a", 20).unwrap().is_empty());
        let mut stale = vec![Thread {
            id: "thread-a".into(),
            preview: deleted.text.clone(),
            messages: vec![deleted.clone()],
            ..Default::default()
        }];
        stale[0].refresh_sidebar_presentation();
        persist_threads(&mut store, &stale).unwrap();
        store.hydrate_selected_threads(&mut stale, 20).unwrap();
        assert!(stale[0].messages.is_empty());
        assert!(stale[0].preview.is_empty());
        assert!(store.upsert_message(&row("thread-b", &deleted)).unwrap()); // (INV-4)
        let marker_body_columns:i64=store.conn.query_row("SELECT count(*) FROM pragma_table_info('deleted_messages') WHERE name IN ('text','raw_json','sender_name')",[],|r|r.get(0)).unwrap();
        assert_eq!(marker_body_columns, 0);
    }
    #[test]
    fn phone_client_alias_is_suppressed_without_merging_two_real_server_ids() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let deleted = message("server-1", "client-1", false);
        store.delete_chat_message("thread-a", &deleted).unwrap();
        let alias = message("phone-client:6:user-1:client-1", "client-1", false);
        assert!(!store.upsert_message(&row("thread-a", &alias)).unwrap());
        let different_server = message("server-2", "client-1", false);
        assert!(store
            .upsert_message(&row("thread-a", &different_server))
            .unwrap());
        let mut another_sender = alias.clone();
        another_sender.wire_ref.sender = Some(serde_json::json!("user-2"));
        assert!(store
            .upsert_message(&row("thread-a", &another_sender))
            .unwrap());
        assert!(store.upsert_message(&row("thread-b", &alias)).unwrap());
    }
    #[test]
    fn deleted_local_send_stays_deleted_after_server_echo() {
        let mut store = MessageStore::open(":memory:").unwrap();
        store
            .delete_chat_message("thread-a", &message("out_1", "client-1", true))
            .unwrap();
        assert!(!store
            .upsert_message(&row("thread-a", &message("server-1", "client-1", true)))
            .unwrap());
        assert!(store
            .upsert_message(&row("thread-a", &message("server-2", "client-2", true)))
            .unwrap());
        assert!(store
            .upsert_message(&row("thread-a", &message("server-3", "client-1", false)))
            .unwrap());
    }
    #[test]
    fn delete_marker_and_content_roll_back_together_on_storage_failure() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let message = message("server-1", "client-1", true);
        store.upsert_message(&row("thread-a", &message)).unwrap();
        store.conn.execute_batch("CREATE TRIGGER fail_delete BEFORE DELETE ON messages BEGIN SELECT RAISE(ABORT,'fixture');END;").unwrap();
        assert!(store.delete_chat_message("thread-a", &message).is_err());
        assert_eq!(store.messages_for("thread-a", 20).unwrap().len(), 1);
        let count: i64 = store
            .conn
            .query_row("SELECT count(*) FROM deleted_messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
    #[test]
    fn remote_conversation_delete_marks_rows_but_local_cache_clear_allows_refetch() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let message = message("server-1", "client-1", true);
        store.upsert_message(&row("local-clear", &message)).unwrap();
        store.clear_thread_history("local-clear").unwrap();
        assert!(store.upsert_message(&row("local-clear", &message)).unwrap());
        store
            .upsert_message(&row("remote-delete", &message))
            .unwrap();
        store.save_composer_draft("remote-delete", "nháp").unwrap();
        store.delete_conversation("remote-delete", &[]).unwrap();
        assert!(!store
            .upsert_message(&row("remote-delete", &message))
            .unwrap());
        assert_eq!(store.composer_draft("remote-delete").unwrap(), "");
        // A genuinely new message in the same conversation remains receivable.
        assert!(store
            .upsert_message(&row(
                "remote-delete",
                &self::message("new", "new-client", false)
            ))
            .unwrap());
    }
    #[test]
    fn draft_is_literal_scoped_and_empty_draft_is_removed() {
        let store = MessageStore::open(":memory:").unwrap();
        let other_account = MessageStore::open(":memory:").unwrap();
        let literal = "  Tiếng Việt đang gõ\n2*3*4  ";
        store.save_composer_draft("thread-a", literal).unwrap();
        assert_eq!(store.composer_draft("thread-a").unwrap(), literal);
        assert_eq!(store.composer_draft("thread-b").unwrap(), "");
        assert_eq!(other_account.composer_draft("thread-a").unwrap(), "");
        store.save_composer_draft("thread-a", "").unwrap();
        assert_eq!(store.composer_draft("thread-a").unwrap(), "");
        assert!(store.save_composer_draft("", literal).is_err());
        assert!(store
            .save_composer_draft("thread-a", &"x".repeat(65537))
            .is_err());
    }
    #[test]
    fn schema_seven_migration_preserves_coverage_drafts_and_markers_after_reopen() {
        let root = std::env::temp_dir().join(format!(
            "ctdoz-followup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join("fixture.db");
        {
            let store = MessageStore::open(&path).unwrap();
            store.conn.execute_batch("DROP TABLE composer_drafts;DROP TABLE deleted_messages;INSERT INTO phone_sync_ranges VALUES('p','thread-a',0,100);PRAGMA user_version=7;").unwrap();
        }
        {
            let mut store = MessageStore::open(&path).unwrap();
            assert_eq!(store.phone_sync_ranges().unwrap().len(), 1);
            store
                .save_composer_draft("thread-a", "Bản nháp sau khởi động")
                .unwrap();
            store
                .delete_chat_message("thread-a", &message("server-1", "client-1", false))
                .unwrap();
        }
        {
            let store = MessageStore::open(&path).unwrap();
            assert_eq!(
                store.composer_draft("thread-a").unwrap(),
                "Bản nháp sau khởi động"
            );
            assert!(!store
                .upsert_message(&row("thread-a", &message("server-1", "client-1", false)))
                .unwrap());
            assert_eq!(store.phone_sync_ranges().unwrap().len(), 1);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod deleted_insert_race_tests {
    use super::*;
    #[test]
    fn sqlite_admission_rejects_a_deleted_row_even_if_an_older_writer_bypasses_rust_checks() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let message = ChatMessage {
            id: "server-1".into(),
            client_message_id: "client-1".into(),
            is_outgoing: true,
            ..Default::default()
        };
        store.delete_chat_message("thread-a", &message).unwrap();
        let raw = serde_json::json!({"message_state":{"client_message_id":"client-1"}}).to_string();
        let exact=store.conn.execute("INSERT OR IGNORE INTO messages(thread_id,msg_id,text,is_outgoing,raw_json) VALUES('thread-a','server-1','must not reappear',1,?1)",[&raw]).unwrap();
        assert_eq!(exact, 0);
        let alias=store.conn.execute("INSERT OR IGNORE INTO messages(thread_id,msg_id,text,is_outgoing,raw_json) VALUES('thread-a','phone-client:client-1','must not reappear',1,?1)",[&raw]).unwrap();
        assert_eq!(alias, 0);
        assert!(store.messages_for("thread-a", 20).unwrap().is_empty());
    }
    #[test]
    fn empty_id_legacy_row_uses_the_same_key_for_persistence_and_delete() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let message = ChatMessage {
            ts: 42,
            text: "legacy".into(),
            ..Default::default()
        };
        let thread = Thread {
            id: "thread-a".into(),
            messages: vec![message.clone()],
            ..Default::default()
        };
        persist_threads(&mut store, std::slice::from_ref(&thread)).unwrap();
        store.delete_chat_message("thread-a", &message).unwrap();
        assert_eq!(persist_threads(&mut store, &[thread]).unwrap(), 0);
        assert!(store.messages_for("thread-a", 20).unwrap().is_empty());
    }
}

#[cfg(test)]
mod mention_draft_tests {
    use super::*;
    #[test]
    fn document_roundtrip_binding_and_delete() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let mut draft = crate::mentions::Draft::from("Xin ");
        draft.insert(4, 4, "42", "An");
        store.save_composer_document("group_1", &draft).unwrap();
        assert_eq!(store.composer_draft("group_1").unwrap(), draft.text);
        assert_eq!(store.composer_document("group_1").unwrap(), draft);
        assert!(store
            .composer_document("group_2")
            .unwrap()
            .mentions
            .is_empty());
        assert!(MessageStore::open(":memory:")
            .unwrap()
            .composer_document("group_1")
            .unwrap()
            .mentions
            .is_empty());
        store
            .conn
            .execute(
                "UPDATE composer_drafts SET text='Khác' WHERE thread_id='group_1'",
                [],
            )
            .unwrap();
        assert!(store
            .composer_document("group_1")
            .unwrap()
            .mentions
            .is_empty());
        store.save_composer_document("group_1", &draft).unwrap();
        store.delete_conversation("group_1", &[]).unwrap();
        let count: i64 = store
            .conn
            .query_row(
                "SELECT count(*) FROM tool_config WHERE key='draft_mentions:group_1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
    #[test]
    fn metadata_failure_rolls_back_literal_text_too() {
        let store = MessageStore::open(":memory:").unwrap();
        store.save_composer_draft("group_1", "old").unwrap();
        store.conn.execute_batch("CREATE TRIGGER fail_mention BEFORE INSERT ON tool_config BEGIN SELECT RAISE(ABORT,'fixture');END;").unwrap();
        let mut draft = crate::mentions::Draft::default();
        draft.insert(0, 0, "42", "An");
        assert!(store.save_composer_document("group_1", &draft).is_err());
        assert_eq!(store.composer_draft("group_1").unwrap(), "old");
    }
}
