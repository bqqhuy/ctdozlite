//! Identity-safe phone message conversion and transactional per-partition persistence.
use crate::{
    chat::{ChatMessage, Thread},
    store::MessageStore,
};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;

pub(super) fn number(value: &Value, name: &str) -> String {
    value
        .get(name)
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| v.to_string())
        })
        .unwrap_or_default()
}
pub(super) fn message_id(value: &Value) -> Result<String> {
    let global = number(value, "globalId");
    if !global.is_empty() && global != "0" {
        return Ok(global);
    }
    let client = number(value, "clientId");
    ensure!(
        !client.is_empty() && client != "0",
        "Phone message has no stable identity"
    );
    let sender = number(value, "senderId");
    ensure!(
        !sender.is_empty() && sender.len() <= 512,
        "Client-only phone message has no sender identity"
    );
    Ok(format!("phone-client:{}:{sender}:{client}", sender.len()))
}
fn message(
    value: &Value,
    senders: &HashMap<String, String>,
    self_id: &str,
    names: &HashMap<String, String>,
) -> Result<ChatMessage> {
    let sender = number(value, "senderId");
    // An unresolved sender must never be inferred to mean self/outgoing.
    let uid = senders
        .get(&sender)
        .cloned()
        .unwrap_or_else(|| format!("phone:{sender}"));
    let kind = match value.get("msgType").and_then(Value::as_i64).unwrap_or(0) {
        0 => "chat.text".into(),
        2 => "chat.doodle".into(),
        3 | 4 | 31 => "chat.photo".into(),
        6 => "chat.voice".into(),
        10 => "chat.sticker".into(),
        12 => "chat.link".into(),
        18 => "chat.location".into(),
        19 => "chat.video".into(),
        25 => "chat.video.live.msg".into(),
        21 => "chat.contact".into(),
        22 => "chat.file".into(),
        23 => "chat.gif".into(),
        36 => "chat.undo".into(),
        n => format!("phone.type_{n}"),
    };
    let mut wire = json!({"msgId":message_id(value)?,"cliMsgId":number(value,"clientId"),"uidFrom":uid,
        "dName":names.get(&uid).unwrap_or(&uid),"isOutgoing":uid==self_id && !self_id.is_empty(),
        "ts":number(value,"timestamp"),"msgType":kind,"content":value.get("content").cloned().unwrap_or(json!(""))});
    if let Some(meta) = value.get("meta") {
        if let Some(quote) = meta.get("quote") {
            wire["quote"] = quote.clone();
        }
        if kind != "chat.text" && kind != "chat.undo" {
            if let Some(attachment) = meta
                .get("attachsList")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
            {
                wire["content"] = attachment.clone();
            }
        }
    }
    let mut msg = crate::chat::parse_chat_message(&wire).context("Invalid phone message")?;
    msg.id = message_id(value)?;
    msg.history_source = Some(value.clone());
    if kind == "chat.undo" {
        msg.mark_recalled();
    }
    Ok(msg)
}

pub(super) struct Imported {
    /// Messages read from the batch for mapped conversations.
    pub read: usize,
    pub inserted: usize,
    /// Already stored before this batch.
    pub existing: usize,
    /// Refused by a local delete marker.
    pub deleted: usize,
    pub unmapped: usize,
    pub previews: Vec<Thread>,
}
/// Persist by real thread identity. Opaque partitions go only to a non-sendable quarantine table.
pub(super) fn save(
    store: &mut MessageStore,
    chunk: &Value,
    threads: &HashMap<String, Thread>,
    senders: &HashMap<String, String>,
    self_id: &str,
) -> Result<Imported> {
    let names: HashMap<_, _> = threads
        .values()
        .map(|t| (t.id.clone(), t.name.clone()))
        .collect();
    let mut result = Imported {
        read: 0,
        inserted: 0,
        existing: 0,
        deleted: 0,
        unmapped: 0,
        previews: Vec::new(),
    };
    for partition in chunk
        .get("partitions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = partition
            .get("id")
            .and_then(Value::as_str)
            .context("Missing phone partition identity")?;
        let messages = partition
            .get("messages")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if let Some(template) = threads.get(id) {
            let mut thread = template.clone();
            thread.messages = messages
                .iter()
                .map(|v| message(v, senders, self_id, &names))
                .collect::<Result<_>>()?;
            // Reconcile a stable client ID with live transport's server ID before SQLite insertion.
            for msg in &mut thread.messages {
                if msg.id.starts_with("phone-client:") {
                    let sender = msg
                        .wire_ref
                        .sender
                        .as_ref()
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let mut statement=store.conn.prepare("SELECT msg_id FROM messages WHERE thread_id=?1 AND is_outgoing=?2 AND (CASE WHEN json_valid(raw_json) THEN json_extract(raw_json,'$.message_state.client_message_id') END)=?3 AND ((CASE WHEN json_valid(raw_json) THEN json_extract(raw_json,'$.message_state.wire_ref.sender') END)=?4 OR (?2=1 AND (CASE WHEN json_valid(raw_json) THEN json_extract(raw_json,'$.message_state.wire_ref.sender') END)='0')) LIMIT 2")?;
                    let candidates = statement
                        .query_map(
                            rusqlite::params![
                                thread.id,
                                msg.is_outgoing,
                                msg.client_message_id,
                                sender
                            ],
                            |r| r.get::<_, String>(0),
                        )?
                        .collect::<rusqlite::Result<Vec<_>>>()?;
                    if candidates.len() == 1 {
                        msg.id = candidates[0].clone();
                    }
                }
            }
            result.read += thread.messages.len();
            let counts =
                crate::store::persist_threads_counted(store, std::slice::from_ref(&thread))?;
            result.inserted += counts.inserted;
            result.existing += counts.existing;
            result.deleted += counts.deleted;
            // Read back the bounded latest page, including durable local/recall state.
            thread.messages.clear();
            let mut hydrated = vec![thread];
            store.hydrate_selected_threads(&mut hydrated, 20)?;
            result.previews.extend(hydrated);
        } else {
            let transaction = store.conn.unchecked_transaction()?;
            for msg in messages {
                result.unmapped+=transaction.execute("INSERT OR IGNORE INTO phone_unmapped(partition_id,msg_id,raw_json) VALUES(?1,?2,?3)",rusqlite::params![id,message_id(msg)?,msg.to_string()])?;
            }
            transaction.commit()?;
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn client_fallback_is_scoped_to_sender_and_legacy_json_is_safe() {
        let a = json!({"globalId":"0","clientId":"11","senderId":"alice"});
        let b = json!({"globalId":"0","clientId":"11","senderId":"bob"});
        assert_ne!(message_id(&a).unwrap(), message_id(&b).unwrap());
        assert!(message_id(&json!({"globalId":"0","clientId":"11"})).is_err());
        let store = MessageStore::open(":memory:").unwrap();
        store
            .conn
            .execute(
                "INSERT INTO messages(thread_id,msg_id,raw_json) VALUES('t','legacy','')",
                [],
            )
            .unwrap();
        let plan:String=store.conn.query_row("EXPLAIN QUERY PLAN SELECT msg_id FROM messages WHERE thread_id='t' AND (CASE WHEN json_valid(raw_json) THEN json_extract(raw_json,'$.message_state.client_message_id') END)='11'",[],|r|r.get(3)).unwrap();
        assert!(plan.contains("idx_message_client"), "{plan}");
    }
    #[test]
    fn zero_server_ids_do_not_collapse_messages_or_create_sendable_threads() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let body = json!({"partitions":[{"id":"oneone/opaque","messages":[{"globalId":"0","clientId":"11","senderId":"fake-a","content":"a"},{"globalId":"0","clientId":"12","senderId":"fake-a","content":"b"}]}]});
        let result = save(&mut store, &body, &HashMap::new(), &HashMap::new(), "").unwrap();
        assert_eq!(result.unmapped, 2);
        assert_eq!(
            save(&mut store, &body, &HashMap::new(), &HashMap::new(), "")
                .unwrap()
                .unmapped,
            0
        );
        assert_eq!(
            store
                .conn
                .query_row("SELECT count(*) FROM threads", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn preserves_phone_metadata_and_recall_without_unread_inflation() {
        let template:Thread=serde_json::from_value(json!({"id":"123","name":"Test","preview":"","time":"","avatar":null,"is_group":false,"messages":[]})).unwrap();
        let threads = HashMap::from([("oneone/opaque".into(), template)]);
        let senders = HashMap::from([("self-global".into(), "999".into())]);
        let mut store = MessageStore::open(":memory:").unwrap();
        let body = json!({"partitions":[{"id":"oneone/opaque","messages":[{"globalId":"9007199254740999","clientId":"11","senderId":"self-global","timestamp":"1700000000000","content":"hello","msgType":0,"meta":{"ttl":"42"}}]}]});
        let result = save(&mut store, &body, &threads, &senders, "999").unwrap();
        assert_eq!(result.inserted, 1);
        assert!(result.previews[0].messages[0].is_outgoing);
        assert_eq!(
            result.previews[0].messages[0]
                .history_source
                .as_ref()
                .unwrap()["meta"]["ttl"],
            "42"
        );
        assert_eq!(
            save(&mut store, &body, &threads, &senders, "999")
                .unwrap()
                .inserted,
            0
        );
        let mut recall = body.clone();
        recall["partitions"][0]["messages"][0]["msgType"] = json!(36);
        save(&mut store, &recall, &threads, &senders, "999").unwrap();
        save(&mut store, &body, &threads, &senders, "999").unwrap();
        assert_eq!(
            store.messages_for("123", 20).unwrap()[0].text,
            "[Tin nhắn đã thu hồi]"
        );
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    #[test]
    #[ignore = "manual synthetic import performance measurement"]
    fn import_5000_phone_messages() {
        let mut times = Vec::new();
        for _ in 0..5 {
            let mut store = MessageStore::open(":memory:").unwrap();
            let thread = Thread {
                id: "123".into(),
                name: "Synthetic group".into(),
                is_group: true,
                ..Default::default()
            };
            let threads = HashMap::from([("group/fake".into(), thread)]);
            let senders = HashMap::from([("self".into(), "999".into())]);
            let messages:Vec<_>=(1..=5000).map(|i|json!({"globalId":i.to_string(),"clientId":(1700000000000i64+i).to_string(),"senderId":"self","timestamp":(1700000000000i64+i).to_string(),"content":"Synthetic message","msgType":0})).collect();
            let body = json!({"partitions":[{"id":"group/fake","messages":messages}]});
            let now = std::time::Instant::now();
            assert_eq!(
                save(&mut store, &body, &threads, &senders, "999")
                    .unwrap()
                    .inserted,
                5000
            );
            times.push(now.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{}",
            json!({"task":"import_5000_phone_messages","iterations":5,"min_ms":times[0],"avg_ms":times.iter().sum::<f64>()/5.0,"p50_ms":times[2],"p95_ms":times[4],"p99_ms":times[4],"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"profile":"debug"})
        );
    }
}

#[cfg(test)]
mod media_archive_tests {
    use super::*;
    #[test]
    fn verified_photo_v2_and_video_live_archive_codes_keep_media_metadata() {
        for (code, expected) in [
            (31, crate::message_view::MediaKind::Photo),
            (25, crate::message_view::MediaKind::Video),
        ] {
            let source = json!({"globalId":"archive-message","clientId":"11","senderId":"sender","timestamp":100,"msgType":code,"content":{"href":"https://photo.zdn.vn/file","thumb":"https://photo.zdn.vn/thumb","params":{"normalUrl":"https://photo.zdn.vn/normal"}}});
            let msg = message(&source, &HashMap::new(), "self", &HashMap::new()).unwrap();
            let media = crate::message_view::media(&msg);
            assert_eq!(media.kind, expected);
            assert!(media.thumbnail.is_some());
            assert!(media.original.is_some());
        }
    }
}
