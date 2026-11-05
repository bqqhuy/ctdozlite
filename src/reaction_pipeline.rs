//! Durable incoming reaction events, including targets that have not arrived yet.
use crate::{
    chat::Thread,
    reactions::{Preview, ReactionEvent},
    session::Session,
    store::{MessageStore, StoredMessage},
};
use anyhow::{ensure, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Only explicit reaction envelopes reach the parser; ordinary text cannot become a reaction.
pub(crate) fn events(payload: &Value) -> Vec<ReactionEvent> {
    let value = payload.get("data").unwrap_or(payload);
    let explicit = value.get("reacts").is_some_and(Value::is_array)
        || value.get("reactGroups").is_some_and(Value::is_array)
        || value.get("msgType").and_then(Value::as_str) == Some("chat.reaction")
        || value
            .get("messages")
            .and_then(Value::as_array)
            .is_some_and(|messages| {
                messages.iter().any(|message| {
                    message.get("msgType").and_then(Value::as_str) == Some("chat.reaction")
                })
            });
    if explicit {
        crate::reactions::parse_reaction_events(value)
    } else {
        Vec::new()
    }
}
/// Store before acknowledging a queue page; incomplete message targets remain pending.
pub(crate) fn enqueue(store: &MessageStore, events: &[ReactionEvent]) -> Result<()> {
    ensure!(
        events.len() <= 10000,
        "Quá nhiều sự kiện reaction trong một trang"
    );
    let tx = store.conn.unchecked_transaction()?;
    for event in events {
        let raw = serde_json::to_string(event)?;
        ensure!(raw.len() <= 8192, "Sự kiện reaction quá lớn");
        let key = format!("{:x}", Sha256::digest(raw.as_bytes()));
        tx.execute(
            "INSERT OR IGNORE INTO pending_reactions(key,data) VALUES(?1,?2)",
            params![key, raw],
        )?;
    }
    let count: i64 = tx.query_row("SELECT COUNT(*) FROM pending_reactions", [], |row| {
        row.get(0)
    })?;
    ensure!(
        count <= 20000,
        "Hàng chờ reaction đã đầy; chưa thể xác nhận cursor"
    );
    tx.commit()?;
    Ok(())
}
/// Apply a bounded batch transactionally; replay cannot inflate the per-reactor ledger.
pub(crate) fn drain(store: &MessageStore, session: &Session) -> Result<Vec<Thread>> {
    let now = crate::login::unix_now() * 1000;
    let due = {
        let mut statement=store.conn.prepare("SELECT key,data FROM pending_reactions WHERE next_try<=?1 ORDER BY next_try,key LIMIT 50")?;
        let values = statement
            .query_map([now], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        values
    };
    let tx = store.conn.unchecked_transaction()?;
    let mut updates: std::collections::HashMap<String, Thread> = std::collections::HashMap::new();
    for (key, raw) in due {
        ensure!(session.is_active(), "Phiên tài khoản đã đóng");
        let mut event: ReactionEvent = serde_json::from_str(&raw)?;
        let group = event.thread_id.starts_with('g');
        let clean = if group {
            event.thread_id.trim_start_matches('g').to_owned()
        } else if event.thread_id == session.account_key() {
            event.sender_id.clone()
        } else {
            event.thread_id.clone()
        };
        let alternate = if group {
            format!("g{clean}")
        } else {
            clean.clone()
        };
        let candidates = {
            let mut query=store.conn.prepare("SELECT m.thread_id,m.msg_id FROM messages m JOIN threads t ON t.thread_id=m.thread_id WHERE (m.thread_id=?1 OR m.thread_id=?2) AND t.is_group=?3 AND ((?4!='' AND m.msg_id=?4) OR (?4='' AND ?5!='' AND CASE WHEN json_valid(m.raw_json) THEN json_extract(m.raw_json,'$.message_state.client_message_id') END=?5)) LIMIT 2")?;
            let values = query
                .query_map(
                    params![
                        clean,
                        alternate,
                        group,
                        event.message_id,
                        event.client_message_id
                    ],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            values
        };
        if candidates.len() != 1 {
            let deleted:bool=store.conn.query_row("SELECT EXISTS(SELECT 1 FROM deleted_messages WHERE (thread_id=?1 OR thread_id=?2) AND msg_id=?3)",params![clean,alternate,event.message_id],|row|row.get(0))?;
            if deleted {
                tx.execute("DELETE FROM pending_reactions WHERE key=?1", [key])?;
            } else {
                tx.execute(
                    "UPDATE pending_reactions SET next_try=?1 WHERE key=?2",
                    params![now + 30000, key],
                )?;
            }
            continue;
        }
        let (thread_id, id) = &candidates[0];
        event.thread_id = thread_id.clone();
        let mut thread = crate::tools::open_hit(store, thread_id, id)?;
        let Some(mut message) = thread
            .messages
            .iter()
            .find(|message| message.id == *id)
            .cloned()
        else {
            continue;
        };
        if crate::message_lifecycle::expired(&message, now) {
            tx.execute("DELETE FROM pending_reactions WHERE key=?1", [key])?;
            continue;
        }
        thread.messages.clear();
        let old = store
            .conn
            .query_row(
                "SELECT data FROM reaction_previews WHERE thread_id=?1",
                [thread_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .and_then(|raw| serde_json::from_str::<Preview>(&raw).ok());
        thread.presentation.reaction_preview = old.clone();
        let changed = message.apply_reaction(&event);
        let stamp = crate::clock::normalize_ts_ms(event.ts);
        let fresh = !event.historical
            && stamp > 0
            && old.as_ref().is_none_or(|old| {
                stamp > old.ts
                    || (stamp == old.ts
                        && (event.event_id.len(), event.event_id.as_str())
                            > (old.event_id.len(), old.event_id.as_str()))
            });
        let mut preview_changed = false;
        if fresh {
            if event.r_type == -1 {
                if old
                    .as_ref()
                    .is_some_and(|old| old.reactor == event.sender_id && old.target == message.id)
                {
                    thread.presentation.reaction_preview = None;
                    preview_changed = true;
                }
            } else if !event.emoji.is_empty() {
                thread.presentation.reaction_preview = Some(Preview {
                    reactor: event.sender_id.clone(),
                    emoji: event.emoji.clone(),
                    target: message.id.clone(),
                    event_id: event.event_id.clone(),
                    ts: stamp,
                    by_self: event.sender_id == session.account_key(),
                    target_by_self: message.is_outgoing,
                });
                preview_changed = true;
            }
        }
        if changed {
            let mut stored = StoredMessage::from(&message);
            stored.thread_id = thread_id.clone();
            store.upsert_message(&stored)?;
        }
        if preview_changed {
            if let Some(preview) = &thread.presentation.reaction_preview {
                tx.execute("INSERT INTO reaction_previews VALUES(?1,?2) ON CONFLICT(thread_id) DO UPDATE SET data=excluded.data",params![thread_id,serde_json::to_string(preview)?])?;
            } else {
                tx.execute(
                    "DELETE FROM reaction_previews WHERE thread_id=?1",
                    [thread_id],
                )?;
            }
            thread.presentation.reaction_preview_fresh = true;
        }
        tx.execute("DELETE FROM pending_reactions WHERE key=?1", [key])?;
        if changed || preview_changed {
            thread.messages.push(message);
            merge_update(&mut updates, thread);
        }
    }
    tx.commit()?;
    Ok(updates.into_values().collect())
}

/// Preserve every changed target when one queue page contains multiple reactions.
fn merge_update(updates: &mut std::collections::HashMap<String, Thread>, thread: Thread) {
    if let Some(previous) = updates.get_mut(&thread.id) {
        for message in thread.messages {
            if let Some(existing) = previous
                .messages
                .iter_mut()
                .find(|old| old.id == message.id)
            {
                *existing = message;
            } else {
                previous.messages.push(message);
            }
        }
        if thread.presentation.reaction_preview_fresh {
            previous.presentation.reaction_preview = thread.presentation.reaction_preview;
            previous.presentation.reaction_preview_fresh = true;
        }
    } else {
        updates.insert(thread.id.clone(), thread);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_drain_preserves_distinct_targets_and_keeps_latest_state_per_target() {
        let mut updates = std::collections::HashMap::new();
        for (id, text) in [("one", "old"), ("two", "second"), ("one", "new")] {
            merge_update(
                &mut updates,
                Thread {
                    id: "peer".into(),
                    messages: vec![crate::chat::ChatMessage {
                        id: id.into(),
                        text: text.into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            );
        }
        let thread = &updates["peer"];
        assert_eq!(thread.messages.len(), 2);
        assert_eq!(
            thread.messages.iter().find(|m| m.id == "one").unwrap().text,
            "new"
        );
    }
}
