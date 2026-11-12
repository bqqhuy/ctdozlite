//! Account-local indexed history search and conversation library queries.
use crate::{chat::ChatMessage, store::MessageStore};
use anyhow::{ensure, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// Monotonic process generation prevents stale results after close, logout or reopening.
pub fn generation() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Search category; visual media remains discoverable without a working thumbnail.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    #[default]
    All,
    Contacts,
    Messages,
    Files,
    Visual,
    Links,
}
/// Keyset cursor is scoped by the complete request at the worker/UI boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    pub ts: i64,
    pub thread: String,
    pub id: String,
}
/// All filters are combined; `thread=None` searches the current account database.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub generation: u64,
    pub query: String,
    pub thread: Option<String>,
    pub category: Category,
    pub sender: String,
    pub extension: String,
    pub from: i64,
    pub to: i64,
    pub hidden: bool,
    pub cursor: Option<Cursor>,
}
/// A bounded page of exact identities; no signed URL is copied into search text.
#[derive(Clone, Debug, Default)]
pub struct Page {
    pub hits: Vec<Hit>,
    pub next: Option<Cursor>,
}
/// Result metadata is independent from GUI row indices and current thread ordering.
#[derive(Clone, Debug)]
pub struct Hit {
    pub thread: String,
    pub id: String,
    pub ts: i64,
    pub thread_name: String,
    pub sender: String,
    pub kind: String,
    pub title: String,
    pub text: String,
    pub group: bool,
    pub message: Option<Box<ChatMessage>>,
}
/// A sender identity and display name for a conversation library filter.
#[derive(Clone, Debug)]
pub struct Sender {
    pub id: String,
    pub name: String,
}

/// Normalize Vietnamese precomposed and decomposed accents without byte slicing.
pub fn fold(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .map(|c| match c {
            'à' | 'á' | 'ạ' | 'ả' | 'ã' | 'â' | 'ầ' | 'ấ' | 'ậ' | 'ẩ' | 'ẫ' | 'ă' | 'ằ' | 'ắ'
            | 'ặ' | 'ẳ' | 'ẵ' => 'a',
            'è' | 'é' | 'ẹ' | 'ẻ' | 'ẽ' | 'ê' | 'ề' | 'ế' | 'ệ' | 'ể' | 'ễ' => {
                'e'
            }
            'ì' | 'í' | 'ị' | 'ỉ' | 'ĩ' => 'i',
            'ò' | 'ó' | 'ọ' | 'ỏ' | 'õ' | 'ô' | 'ồ' | 'ố' | 'ộ' | 'ổ' | 'ỗ' | 'ơ' | 'ờ' | 'ớ'
            | 'ợ' | 'ở' | 'ỡ' => 'o',
            'ù' | 'ú' | 'ụ' | 'ủ' | 'ũ' | 'ư' | 'ừ' | 'ứ' | 'ự' | 'ử' | 'ữ' => {
                'u'
            }
            'ỳ' | 'ý' | 'ỵ' | 'ỷ' | 'ỹ' => 'y',
            'đ' => 'd',
            other => other,
        })
        .collect()
}
fn projection(text: String, raw: String, kind: String, id: String) -> String {
    let call_kind = matches!(
        kind.as_str(),
        "chat.call" | "chat.call.msg" | "call" | "voip" | "calllog" | "call_log"
    );
    let mut m = ChatMessage {
        id,
        text,
        kind: crate::message_model::MessageKind::from_wire_name(&kind),
        ..Default::default()
    };
    if raw.len() <= 1024 * 1024 {
        let _ = crate::tools::restore_import_metadata(&mut m, &raw);
    }
    let visible = !call_kind
        && !crate::message_view::is_call_log(&m)
        && !matches!(
            m.kind,
            crate::message_model::MessageKind::Recall | crate::message_model::MessageKind::Delete
        );
    let media = crate::message_view::media(&m);
    let category = match media.kind {
        crate::message_view::MediaKind::Photo | crate::message_view::MediaKind::Video => "visual",
        crate::message_view::MediaKind::File => "file",
        _ if m.link_url.is_some() || crate::chat::extract_first_url(&m.text).is_some() => "link",
        _ => "message",
    };
    let title = if category == "file" || category == "visual" {
        media.name
    } else {
        m.link_title.clone().unwrap_or_default()
    };
    let snippet: String = m.text.chars().take(400).collect();
    // Only explicit link messages contribute URLs. CDN signatures are not searchable metadata.
    let link = if category == "link" {
        m.link_url.as_deref().unwrap_or(&m.text)
    } else {
        ""
    };
    let extension = title
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .filter(|ext| ext.len() <= 16 && ext.bytes().all(|b| b.is_ascii_alphanumeric()))
        .unwrap_or_default();
    serde_json::json!({"visible":visible,"kind":category,"title":title,"extension":extension,"text":snippet,
        "search":fold(&format!("{} {} {}", m.text, title, link))})
    .to_string()
}
/// Register projection functions on every writer connection before installing triggers.
pub(crate) fn register(conn: &Connection) -> Result<()> {
    use rusqlite::functions::FunctionFlags as F;
    conn.create_scalar_function(
        "ctdo_search_projection",
        4,
        F::SQLITE_UTF8 | F::SQLITE_DETERMINISTIC,
        |ctx| {
            Ok(projection(
                ctx.get(0)?,
                ctx.get(1)?,
                ctx.get(2)?,
                ctx.get(3)?,
            ))
        },
    )?;
    conn.create_scalar_function(
        "ctdo_search_fold",
        1,
        F::SQLITE_UTF8 | F::SQLITE_DETERMINISTIC,
        |ctx| Ok(fold(&ctx.get::<String>(0)?)),
    )?;
    Ok(())
}
/// Add a rebuildable projection and trigram index, preserving history and checkpoints.
pub(crate) fn migrate(conn: &Connection) -> Result<()> {
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
    let version: u32 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version >= 9 {
        tx.commit()?;
        return Ok(());
    }
    tx.execute_batch(r#"
    DROP TRIGGER IF EXISTS search_message_insert;
    DROP TRIGGER IF EXISTS search_message_update;
    DROP TRIGGER IF EXISTS search_message_delete;
    DROP TRIGGER IF EXISTS search_fts_insert;
    DROP TRIGGER IF EXISTS search_fts_delete;
    DROP TRIGGER IF EXISTS search_fts_update;
    DROP TABLE IF EXISTS history_fts;
    DROP TABLE IF EXISTS search_entries;
    CREATE TABLE search_entries (
        thread_id TEXT NOT NULL,msg_id TEXT NOT NULL,ts INTEGER NOT NULL,sender_id TEXT NOT NULL,sender_name TEXT NOT NULL,
        projection TEXT NOT NULL,
        kind TEXT GENERATED ALWAYS AS (json_extract(projection,'$.kind')) STORED,
        title TEXT GENERATED ALWAYS AS (json_extract(projection,'$.title')) STORED,
        extension TEXT GENERATED ALWAYS AS (json_extract(projection,'$.extension')) STORED,
        snippet TEXT GENERATED ALWAYS AS (json_extract(projection,'$.text')) STORED,
        search_text TEXT GENERATED ALWAYS AS (json_extract(projection,'$.search')) STORED,
        visible INTEGER GENERATED ALWAYS AS (json_extract(projection,'$.visible')) STORED,
        UNIQUE(thread_id,msg_id));
    CREATE INDEX search_time ON search_entries(ts DESC,thread_id DESC,msg_id DESC);
    CREATE INDEX search_thread_time ON search_entries(thread_id,ts DESC,msg_id DESC);
    CREATE INDEX search_library ON search_entries(thread_id,kind,ts DESC,msg_id DESC);
    CREATE INDEX search_kind_time ON search_entries(kind,ts DESC,thread_id DESC,msg_id DESC);
    CREATE INDEX search_sender_time ON search_entries(thread_id,sender_id,ts DESC,msg_id DESC);
    CREATE VIRTUAL TABLE history_fts USING fts5(search_text,content='search_entries',content_rowid='rowid',tokenize='trigram');
    CREATE TRIGGER search_fts_insert AFTER INSERT ON search_entries BEGIN
        INSERT INTO history_fts(rowid,search_text) VALUES(NEW.rowid,NEW.search_text); END;
    CREATE TRIGGER search_fts_delete AFTER DELETE ON search_entries BEGIN
        INSERT INTO history_fts(history_fts,rowid,search_text) VALUES('delete',OLD.rowid,OLD.search_text); END;
    CREATE TRIGGER search_fts_update AFTER UPDATE ON search_entries BEGIN
        INSERT INTO history_fts(history_fts,rowid,search_text) VALUES('delete',OLD.rowid,OLD.search_text);
        INSERT INTO history_fts(rowid,search_text) VALUES(NEW.rowid,NEW.search_text); END;
    CREATE TRIGGER search_message_insert AFTER INSERT ON messages BEGIN
        INSERT INTO search_entries(thread_id,msg_id,ts,sender_id,sender_name,projection)
        VALUES(NEW.thread_id,NEW.msg_id,COALESCE(NEW.ts,0),COALESCE(CASE WHEN json_valid(NEW.raw_json) THEN CAST(COALESCE(json_extract(NEW.raw_json,'$.message_state.wire_ref.sender'),json_extract(NEW.raw_json,'$.uidFrom'),json_extract(NEW.raw_json,'$.fromUid')) AS TEXT) END,NEW.sender_id,''),COALESCE(NEW.sender_name,''),ctdo_search_projection(COALESCE(NEW.text,''),COALESCE(NEW.raw_json,''),COALESCE(NEW.msg_type,''),NEW.msg_id)); END;
    CREATE TRIGGER search_message_update AFTER UPDATE OF thread_id,msg_id,text,raw_json,msg_type,sender_id,sender_name,ts ON messages
    WHEN NEW.thread_id IS NOT OLD.thread_id OR NEW.msg_id IS NOT OLD.msg_id OR NEW.text IS NOT OLD.text OR NEW.raw_json IS NOT OLD.raw_json OR NEW.msg_type IS NOT OLD.msg_type OR NEW.sender_id IS NOT OLD.sender_id OR NEW.sender_name IS NOT OLD.sender_name OR NEW.ts IS NOT OLD.ts
    BEGIN
        DELETE FROM search_entries WHERE thread_id=OLD.thread_id AND msg_id=OLD.msg_id;
        INSERT INTO search_entries(thread_id,msg_id,ts,sender_id,sender_name,projection)
        VALUES(NEW.thread_id,NEW.msg_id,COALESCE(NEW.ts,0),COALESCE(CASE WHEN json_valid(NEW.raw_json) THEN CAST(COALESCE(json_extract(NEW.raw_json,'$.message_state.wire_ref.sender'),json_extract(NEW.raw_json,'$.uidFrom'),json_extract(NEW.raw_json,'$.fromUid')) AS TEXT) END,NEW.sender_id,''),COALESCE(NEW.sender_name,''),ctdo_search_projection(COALESCE(NEW.text,''),COALESCE(NEW.raw_json,''),COALESCE(NEW.msg_type,''),NEW.msg_id)); END;
    CREATE TRIGGER search_message_delete AFTER DELETE ON messages BEGIN
        DELETE FROM search_entries WHERE thread_id=OLD.thread_id AND msg_id=OLD.msg_id; END;
    CREATE TABLE IF NOT EXISTS search_index_state(id INTEGER PRIMARY KEY CHECK(id=1),thread_cursor TEXT NOT NULL,msg_cursor TEXT NOT NULL,is_complete INTEGER NOT NULL);
    INSERT INTO search_index_state VALUES(1,'','',0) ON CONFLICT(id) DO UPDATE SET thread_cursor='',msg_cursor='',is_complete=0;
    PRAGMA user_version=9;
    "#)?;
    tx.commit()?;
    Ok(())
}
/// Backfill only derived rows. New writes and deletions remain covered by triggers.
fn ensure_ready(store: &MessageStore, cancel: &dyn Fn() -> bool) -> Result<()> {
    loop {
        ensure!(!cancel(), "Đã dừng chuẩn bị chỉ mục tìm kiếm");
        let complete: bool = store.conn.query_row(
            "SELECT is_complete FROM search_index_state WHERE id=1",
            [],
            |r| r.get(0),
        )?;
        if complete {
            return Ok(());
        }
        let tx = rusqlite::Transaction::new_unchecked(
            &store.conn,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        let (thread, id, complete): (String, String, bool) = tx.query_row(
            "SELECT thread_cursor,msg_cursor,is_complete FROM search_index_state WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        if complete {
            tx.commit()?;
            return Ok(());
        }
        let keys = {
            let mut stmt=tx.prepare("SELECT thread_id,msg_id FROM messages WHERE (thread_id,msg_id)>(?1,?2) ORDER BY thread_id,msg_id LIMIT 500")?;
            let rows = stmt
                .query_map(params![thread, id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        if let Some((last_thread, last_id)) = keys.last() {
            tx.execute("INSERT OR IGNORE INTO search_entries(thread_id,msg_id,ts,sender_id,sender_name,projection)
                SELECT m.thread_id,m.msg_id,COALESCE(m.ts,0),COALESCE(CASE WHEN json_valid(m.raw_json) THEN CAST(COALESCE(json_extract(m.raw_json,'$.message_state.wire_ref.sender'),json_extract(m.raw_json,'$.uidFrom'),json_extract(m.raw_json,'$.fromUid')) AS TEXT) END,m.sender_id,''),COALESCE(m.sender_name,''),ctdo_search_projection(COALESCE(m.text,''),COALESCE(m.raw_json,''),COALESCE(m.msg_type,''),m.msg_id)
                FROM messages m WHERE (m.thread_id,m.msg_id)>(?1,?2) AND (m.thread_id,m.msg_id)<=(?3,?4)
                AND NOT EXISTS(SELECT 1 FROM search_entries e WHERE e.thread_id=m.thread_id AND e.msg_id=m.msg_id)",params![thread,id,last_thread,last_id])?;
            tx.execute("UPDATE search_index_state SET thread_cursor=?1,msg_cursor=?2,is_complete=?3 WHERE id=1",params![last_thread,last_id,keys.len()<500])?;
        } else {
            tx.execute("UPDATE search_index_state SET is_complete=1 WHERE id=1", [])?;
        }
        tx.commit()?;
        std::thread::yield_now();
    }
}

/// Search up to fifty results; invalid FTS syntax in user input is always literal.
/// # Errors
/// Returns validation or SQLite errors; never panics on Unicode queries.
pub fn query(store: &MessageStore, request: &Request) -> Result<Page> {
    query_with_cancel(store, request, &|| false)
}

/// Build missing projections in short transactions, checking cancellation between chunks.
/// # Errors
/// Returns cancellation, validation or SQLite errors without acknowledging partial results.
pub fn query_with_cancel(
    store: &MessageStore,
    request: &Request,
    cancel: &dyn Fn() -> bool,
) -> Result<Page> {
    crate::crash_report::phase("history_search_query");
    ensure!(
        request.thread.as_ref().is_none_or(|s| s.len() <= 512),
        "Định danh hội thoại quá dài"
    );
    ensure!(
        request.query.len() <= 512 && request.sender.len() <= 256,
        "Bộ lọc tìm kiếm quá dài"
    );
    ensure!(
        request.from >= 0 && request.to >= 0 && (request.to == 0 || request.to >= request.from),
        "Khoảng thời gian không hợp lệ"
    );
    ensure!(
        request.extension.len() <= 16
            && request.extension.bytes().all(|b| b.is_ascii_alphanumeric()),
        "Đuôi tệp không hợp lệ"
    );
    let needle = fold(request.query.trim());
    if request.category == Category::Contacts {
        return contacts(store, request, &needle);
    }
    while !crate::message_lifecycle::backfill(&store.conn)? {
        ensure!(!cancel(), "Đã hủy tìm kiếm");
    }
    while !crate::message_lifecycle::purge(&store.conn, crate::login::unix_now() * 1000)?.is_empty()
    {
        ensure!(!cancel(), "Đã hủy tìm kiếm");
    }
    ensure_ready(store, cancel)?;
    let pattern = format!(
        "%{}%",
        needle
            .replace('!', "!!")
            .replace('%', "!%")
            .replace('_', "!_")
    );
    let use_fts = needle.chars().count() >= 3;
    let fts = format!("\"{}\"", needle.replace('"', "\"\""));
    let kind = match request.category {
        Category::Files => "file",
        Category::Visual => "visual",
        Category::Links => "link",
        _ => "",
    };
    let cursor = request.cursor.clone().unwrap_or(Cursor {
        ts: i64::MAX,
        thread: "\u{10ffff}".into(),
        id: "\u{10ffff}".into(),
    });
    let fts_clause = if use_fts {
        "AND e.rowid IN (SELECT rowid FROM history_fts WHERE history_fts MATCH ?10)"
    } else {
        "AND (?10 IS NOT NULL)"
    };
    let thread_clause = if request.thread.is_some() {
        "e.thread_id=?2"
    } else {
        "?2 IS NULL"
    };
    let kind_clause = if kind.is_empty() {
        "?3=''"
    } else {
        "e.kind=?3"
    };
    let sender_clause = if request.sender.is_empty() {
        "?4=''"
    } else {
        "e.sender_id=?4"
    };
    let sql=format!("SELECT e.thread_id,e.msg_id,e.ts,substr(COALESCE(t.name,e.thread_id),1,256),substr(e.sender_name,1,256),e.kind,e.title,e.snippet,COALESCE(t.is_group,0) FROM search_entries e LEFT JOIN threads t ON t.thread_id=e.thread_id LEFT JOIN thread_preferences p ON p.thread_id=e.thread_id WHERE e.visible=1 AND (?1 OR COALESCE(p.is_hidden,0)=0) AND {thread_clause} AND {kind_clause} AND {sender_clause} AND e.ts>=?5 AND (?6=0 OR e.ts<=?6) AND (e.ts,e.thread_id,e.msg_id)<(?7,?8,?9) {fts_clause} AND e.search_text LIKE ?11 ESCAPE '!' AND (?12='' OR e.extension=?12) ORDER BY e.ts DESC,e.thread_id DESC,e.msg_id DESC LIMIT 51");
    let mut stmt = store.conn.prepare(&sql)?;
    let rows = stmt.query_map(
        params![
            request.hidden,
            request.thread,
            kind,
            request.sender,
            request.from,
            request.to,
            cursor.ts,
            cursor.thread,
            cursor.id,
            fts,
            pattern,
            request.extension.to_ascii_lowercase()
        ],
        |r| {
            Ok(Hit {
                thread: r.get(0)?,
                id: r.get(1)?,
                ts: r.get(2)?,
                thread_name: r.get(3)?,
                sender: r.get(4)?,
                kind: r.get(5)?,
                title: r.get(6)?,
                text: r.get(7)?,
                group: r.get(8)?,
                message: None,
            })
        },
    )?;
    let mut result = page(rows.collect::<rusqlite::Result<Vec<_>>>()?)?;
    let mut metadata=store.conn.prepare_cached("SELECT COALESCE(text,''),COALESCE(raw_json,''),COALESCE(msg_type,''),COALESCE(is_outgoing,0) FROM messages WHERE thread_id=?1 AND msg_id=?2 AND length(COALESCE(raw_json,''))<=1048576")?;
    for hit in &mut result.hits {
        if !matches!(hit.kind.as_str(), "visual" | "file" | "link") {
            continue;
        }
        use rusqlite::OptionalExtension;
        let row = metadata
            .query_row(params![hit.thread, hit.id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, bool>(3)?,
                ))
            })
            .optional()?;
        if let Some((text, raw, kind, is_outgoing)) = row {
            let mut message = ChatMessage {
                id: hit.id.clone(),
                ts: hit.ts,
                sender: hit.sender.clone(),
                text,
                is_outgoing,
                kind: crate::message_model::MessageKind::from_wire_name(&kind),
                ..Default::default()
            };
            if crate::tools::restore_import_metadata(&mut message, &raw).is_ok() {
                hit.message = Some(Box::new(message));
            }
        }
    }
    if request.category == Category::All
        && request.thread.is_none()
        && request.cursor.is_none()
        && !needle.is_empty()
        && request.sender.is_empty()
        && request.extension.is_empty()
        && request.from == 0
        && request.to == 0
    {
        let mut contact_hits = contacts(store, request, &needle)?.hits;
        contact_hits.truncate(10);
        contact_hits.append(&mut result.hits);
        result.hits = contact_hits;
    }
    Ok(result)
}
fn page(mut hits: Vec<Hit>) -> Result<Page> {
    let next = if hits.len() > 50 {
        hits.truncate(50);
        hits.last().map(|h| Cursor {
            ts: h.ts,
            thread: h.thread.clone(),
            id: h.id.clone(),
        })
    } else {
        None
    };
    Ok(Page { hits, next })
}
fn contacts(store: &MessageStore, request: &Request, needle: &str) -> Result<Page> {
    let pattern = format!(
        "%{}%",
        needle
            .replace('!', "!!")
            .replace('%', "!%")
            .replace('_', "!_")
    );
    let cursor = request.cursor.clone().unwrap_or(Cursor {
        ts: i64::MAX,
        thread: "\u{10ffff}".into(),
        id: String::new(),
    });
    let mut stmt=store.conn.prepare("SELECT t.thread_id,COALESCE(t.name,t.thread_id),COALESCE(t.last_ts,0),t.is_group FROM threads t LEFT JOIN thread_preferences p ON p.thread_id=t.thread_id WHERE (?1 OR COALESCE(p.is_hidden,0)=0) AND ctdo_search_fold(COALESCE(t.name,'')||' '||t.thread_id) LIKE ?2 ESCAPE '!' AND (COALESCE(t.last_ts,0),t.thread_id)<(?3,?4) ORDER BY COALESCE(t.last_ts,0) DESC,t.thread_id DESC LIMIT 51")?;
    let hits = stmt
        .query_map(
            params![request.hidden, pattern, cursor.ts, cursor.thread],
            |r| {
                Ok(Hit {
                    thread: r.get(0)?,
                    id: String::new(),
                    ts: r.get(2)?,
                    thread_name: r.get(1)?,
                    sender: String::new(),
                    kind: if r.get::<_, bool>(3)? {
                        "group"
                    } else {
                        "contact"
                    }
                    .into(),
                    title: r.get(1)?,
                    text: String::new(),
                    group: r.get(3)?,
                    message: None,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    page(hits)
}
/// List sender identities from the full local thread, not only resident messages.
pub fn senders(store: &MessageStore, thread: &str) -> Result<Vec<Sender>> {
    let mut stmt=store.conn.prepare("SELECT sender_id,MAX(sender_name) FROM search_entries WHERE thread_id=?1 AND sender_id!='' AND visible=1 GROUP BY sender_id ORDER BY MAX(sender_name),sender_id LIMIT 1000")?;
    let rows = stmt
        .query_map([thread], |r| {
            Ok(Sender {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> MessageStore {
        let s = MessageStore::open(":memory:").unwrap();
        s.conn.execute_batch("INSERT INTO threads (thread_id, name, is_group, last_ts) VALUES('t','Nhóm thử',1,100); INSERT INTO threads (thread_id, name, is_group, last_ts) VALUES('hidden','Private',0,90);INSERT INTO thread_preferences(thread_id,is_hidden) VALUES('hidden',1);").unwrap();
        s
    }
    fn insert(s: &MessageStore, id: &str, ts: i64, text: &str, kind: &str, raw: &str) {
        s.conn.execute("INSERT INTO messages(thread_id,msg_id,ts,text,msg_type,raw_json,sender_id,sender_name) VALUES('t',?1,?2,?3,?4,?5,'user-1','Nguyễn')",params![id,ts,text,kind,raw]).unwrap();
    }
    #[test]
    fn indexed_search_handles_literal_fts_and_decomposed_vietnamese() {
        let s = store();
        insert(
            &s,
            "a",
            1,
            "Nguye\u{0302}\u{0303}n 100%_ \" OR 😀",
            "webchat",
            "{}",
        );
        for q in ["nguyen", "100%_", "\" OR", "😀"] {
            assert_eq!(
                query(
                    &s,
                    &Request {
                        query: q.into(),
                        ..Default::default()
                    }
                )
                .unwrap()
                .hits
                .len(),
                1,
                "{q}"
            );
        }
        assert!(query(
            &s,
            &Request {
                query: "no match".into(),
                ..Default::default()
            }
        )
        .unwrap()
        .hits
        .is_empty());
    }
    #[test]
    fn index_tracks_identity_changes_delete_and_recall() {
        let s = store();
        insert(&s, "local-1", 1, "secret phrase", "webchat", "{}");
        s.conn
            .execute(
                "UPDATE messages SET msg_id='server-1' WHERE msg_id='local-1'",
                [],
            )
            .unwrap();
        let request = Request {
            query: "secret".into(),
            ..Default::default()
        };
        assert_eq!(query(&s, &request).unwrap().hits[0].id, "server-1");
        s.conn.execute("UPDATE messages SET text='[Tin nhắn đã thu hồi]',msg_type='chat.undo' WHERE msg_id='server-1'",[]).unwrap();
        assert!(query(&s, &Request::default()).unwrap().hits.is_empty());
        s.conn.execute("DELETE FROM messages", []).unwrap();
        assert!(query(&s, &request).unwrap().hits.is_empty());
        let n: i64 = s
            .conn
            .query_row("SELECT count(*) FROM search_entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
    #[test]
    fn library_filters_sender_time_and_keeps_media_without_thumbnail() {
        let s = store();
        insert(&s, "v", 100, "video caption", "chat.video.msg", "{}");
        insert(&s, "f", 101, "report.xlsx", "share.file", "{}");
        assert_eq!(
            query(
                &s,
                &Request {
                    category: Category::Visual,
                    ..Default::default()
                }
            )
            .unwrap()
            .hits
            .len(),
            1
        );
        assert_eq!(
            query(
                &s,
                &Request {
                    category: Category::Files,
                    sender: "user-1".into(),
                    from: 101,
                    to: 101,
                    ..Default::default()
                }
            )
            .unwrap()
            .hits
            .len(),
            1
        );
        assert!(query(
            &s,
            &Request {
                sender: "other".into(),
                ..Default::default()
            }
        )
        .unwrap()
        .hits
        .is_empty());
        assert_eq!(senders(&s, "t").unwrap()[0].id, "user-1");
    }
    #[test]
    fn keyset_pagination_has_no_duplicates_with_equal_timestamps() {
        let s = store();
        for i in 0..125 {
            insert(&s, &format!("msg-{i:04}"), 10, "test", "webchat", "{}");
        }
        let mut request = Request::default();
        let mut ids = std::collections::HashSet::new();
        loop {
            let page = query(&s, &request).unwrap();
            for hit in page.hits {
                assert!(ids.insert(hit.id));
            }
            if page.next.is_none() {
                break;
            }
            request.cursor = page.next;
        }
        assert_eq!(ids.len(), 125);
    }
    #[test]
    fn hidden_threads_and_call_rows_never_leak_into_default_search() {
        let s = store();
        s.conn
            .execute_batch(
                "INSERT INTO messages(thread_id,msg_id,text,ts) VALUES('hidden','x','secret',3)",
            )
            .unwrap();
        insert(&s, "call_x", 1, "call", "call", "{}");
        assert!(query(&s, &Request::default()).unwrap().hits.is_empty());
        assert_eq!(
            query(
                &s,
                &Request {
                    hidden: true,
                    ..Default::default()
                }
            )
            .unwrap()
            .hits
            .len(),
            1
        );
    }
}

#[cfg(test)]
mod benchmarks {
    use super::*;
    #[test]
    #[ignore = "synthetic 632k-row search benchmark"]
    fn indexed_search_632k_rows() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let begin = std::time::Instant::now();
        let tx = store.conn.transaction().unwrap();
        tx.execute("INSERT INTO threads (thread_id, name, is_group, last_ts) VALUES('t','Synthetic',1,632000)", [])
            .unwrap();
        {
            let mut st=tx.prepare("INSERT INTO messages(thread_id,msg_id,ts,text,msg_type,raw_json,sender_id,sender_name) VALUES('t',?1,?2,?3,'webchat','{}','user-1','Synthetic')").unwrap();
            for i in 0..632000 {
                let text = if i % 10000 == 0 {
                    "Nguyễn tài liệu quarterly report uncommon-marker"
                } else {
                    "Tin nhắn công việc thông thường và lịch họp"
                };
                st.execute(params![format!("msg-{i:08}"), i, text]).unwrap();
            }
        }
        tx.commit().unwrap();
        println!(
            "dataset_rows=632000 setup_ms={}",
            begin.elapsed().as_millis()
        );
        for (name, request) in [
            (
                "indexed_missing",
                Request {
                    query: "absent-token-xyz".into(),
                    category: Category::Messages,
                    ..Default::default()
                },
            ),
            (
                "indexed_rare",
                Request {
                    query: "uncommon-marker".into(),
                    category: Category::Messages,
                    ..Default::default()
                },
            ),
            (
                "thread_browse",
                Request {
                    thread: Some("t".into()),
                    ..Default::default()
                },
            ),
            (
                "library_missing",
                Request {
                    thread: Some("t".into()),
                    category: Category::Files,
                    ..Default::default()
                },
            ),
        ] {
            let mut times = Vec::new();
            for _ in 0..20 {
                let start = std::time::Instant::now();
                let _page = query(&store, &request).unwrap();
                times.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            times.sort_by(f64::total_cmp);
            println!("task={name} iterations=20 min_ms={:.3} avg_ms={:.3} p50_ms={:.3} p95_ms={:.3} p99_ms={:.3}",times[0],times.iter().sum::<f64>()/20.0,times[10],times[18],times[19]);
        }
        let start = std::time::Instant::now();
        let _:i64=store.conn.query_row("SELECT count(*) FROM messages WHERE ctdo_search_fold(text) LIKE '%absent-token-xyz%'",[],|r|r.get(0)).unwrap();
        println!(
            "task=legacy_fold_scan iterations=1 elapsed_ms={:.3}",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

#[cfg(test)]
mod sender_identity_tests {
    use super::*;
    #[test]
    fn prefers_source_uid_over_legacy_display_name_column() {
        let store = MessageStore::open(":memory:").unwrap();
        for uid in ["100", "200"] {
            let raw = serde_json::json!({"message_state":{"wire_ref":{"sender":uid}}}).to_string();
            store.conn.execute("INSERT INTO messages(thread_id,msg_id,text,ts,msg_type,raw_json,sender_id,sender_name) VALUES('t',?1,'hello',1,'webchat',?2,'Same name','Same name')",params![uid,raw]).unwrap();
        }
        let people = senders(&store, "t").unwrap();
        assert_eq!(people.len(), 2);
        let result = query(
            &store,
            &Request {
                thread: Some("t".into()),
                sender: "100".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].id, "100");
    }
}

#[cfg(test)]
mod backfill_tests {
    use super::*;
    #[test]
    fn existing_history_is_indexed_lazily_and_can_resume_after_cancellation() {
        let root = std::env::temp_dir().join(format!(
            "ctdo-search-upgrade-{}",
            crate::param_cipher::random_hex(8)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("history.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE messages(msg_id TEXT NOT NULL,thread_id TEXT NOT NULL,sender_id TEXT,sender_name TEXT,text TEXT,msg_type TEXT,ts INTEGER,is_outgoing INTEGER,raw_json TEXT,PRIMARY KEY(thread_id,msg_id));INSERT INTO messages VALUES('old','t','100','Name','legacy searchable', 'webchat',1,0,'{}');").unwrap();
        }
        let store = MessageStore::open(&path).unwrap();
        let count: i64 = store
            .conn
            .query_row("SELECT count(*) FROM search_entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        assert!(query_with_cancel(&store, &Request::default(), &|| true).is_err());
        assert_eq!(
            query(
                &store,
                &Request {
                    query: "legacy".into(),
                    ..Default::default()
                }
            )
            .unwrap()
            .hits[0]
                .id,
            "old"
        );
        let complete: bool = store
            .conn
            .query_row(
                "SELECT is_complete FROM search_index_state WHERE id=1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(complete);
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn library_query_uses_thread_kind_index() {
        let store = MessageStore::open(":memory:").unwrap();
        let mut stmt=store.conn.prepare("EXPLAIN QUERY PLAN SELECT msg_id FROM search_entries WHERE thread_id=?1 AND kind=?2 ORDER BY ts DESC,msg_id DESC LIMIT 50").unwrap();
        let plans = stmt
            .query_map(["t", "file"], |r| r.get::<_, String>(3))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(
            plans.iter().any(|p| p.contains("search_library")),
            "{plans:?}"
        );
    }
}

#[cfg(test)]
mod legacy_media_tests {
    use super::*;
    #[test]
    fn file_search_uses_original_metadata_name_and_can_open_the_result() {
        let store = MessageStore::open(":memory:").unwrap();
        let raw=serde_json::json!({"msgType":"share.file","uidFrom":"100","content":{"fileName":"Báo cáo ngân sách.xlsx","href":"https://dlfl.vn/example"}}).to_string();
        store.conn.execute("INSERT INTO messages(thread_id,msg_id,text,ts,msg_type,raw_json,sender_id,sender_name) VALUES('t','m','[Tệp]',1,'share.file',?1,'Same name','Same name')",[raw]).unwrap();
        let page = query(
            &store,
            &Request {
                category: Category::Files,
                query: "ngan sach".into(),
                sender: "100".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(page.hits.len(), 1);
        assert_eq!(page.hits[0].title, "Báo cáo ngân sách.xlsx");
        assert_eq!(
            crate::message_view::media(page.hits[0].message.as_deref().unwrap())
                .original
                .as_deref(),
            Some("https://dlfl.vn/example")
        );
    }
}
