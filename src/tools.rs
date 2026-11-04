//! Account-scoped search, read-only history migration, scheduling and opt-in replies.
use crate::{
    chat::{ChatMessage, Thread},
    store::MessageStore,
};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default)]
    pub theme: serde_json::Value,
    #[serde(default)]
    pub stickers: Vec<StickerPack>,
    #[serde(default)]
    pub rules: Vec<ReplyRule>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StickerPack {
    pub cate_id: u64,
    pub name: String,
    pub sticker_ids: Vec<u64>,
    #[serde(default = "sticker_kind")]
    pub sticker_type: u32,
}
fn sticker_kind() -> u32 {
    7
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplyRule {
    pub id: String,
    pub keyword: String,
    pub reply_text: String,
    pub mode: String,
    pub scope: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub enabled_since: i64,
}
impl Preferences {
    /// Validate user-controlled resource sizes and supported match modes before storing.
    pub fn validate(&mut self, now: i64) -> Result<()> {
        ensure!(
            self.stickers.len() <= 64 && self.rules.len() <= 64,
            "Tối đa 64 bộ sticker / quy tắc"
        );
        ensure!(
            serde_json::to_vec(&self.theme)?.len() <= 64 * 1024,
            "Theme vượt giới hạn 64 KiB"
        );
        let mut packs = std::collections::HashSet::new();
        for pack in &self.stickers {
            ensure!(
                packs.insert(pack.cate_id)
                    && pack.cate_id > 0
                    && !pack.name.trim().is_empty()
                    && pack.name.len() <= 200
                    && pack.sticker_ids.len() <= 500
                    && pack.sticker_ids.iter().all(|id| *id > 0),
                "Bộ sticker không hợp lệ"
            );
        }
        ensure!(
            self.theme.is_null() || self.theme.is_object(),
            "Theme phải là đối tượng JSON"
        );
        if let Some(map) = self.theme.as_object() {
            for (key, value) in map {
                if key == "dark" || key == "light" {
                    ensure!(value.is_object(), "dark/light phải là bảng màu");
                    for color in value.as_object().into_iter().flat_map(|m| m.values()) {
                        validate_color(color)?;
                    }
                } else {
                    validate_color(value)?;
                }
            }
        }
        let mut ids = std::collections::HashSet::new();
        for rule in &mut self.rules {
            ensure!(
                !rule.id.is_empty()
                    && rule.id.len() <= 128
                    && ids.insert(rule.id.clone())
                    && !rule.keyword.trim().is_empty()
                    && rule.keyword.len() <= 512
                    && !rule.reply_text.trim().is_empty()
                    && rule.reply_text.len() <= 4096,
                "Quy tắc trả lời không hợp lệ hoặc trùng ID"
            );
            ensure!(
                matches!(rule.mode.as_str(), "contains" | "exact" | "prefix")
                    && matches!(rule.scope.as_str(), "all" | "direct" | "group"),
                "Kiểu khớp/phạm vi không hợp lệ"
            );
            if rule.enabled {
                rule.enabled_since = now;
            } else {
                rule.enabled_since = 0;
            }
        }
        Ok(())
    }
}
fn validate_color(value: &serde_json::Value) -> Result<()> {
    ensure!(
        value
            .as_str()
            .and_then(|s| s.strip_prefix('#'))
            .is_some_and(|s| s.len() == 6 && s.bytes().all(|b| b.is_ascii_hexdigit())),
        "Màu theme phải có dạng #RRGGBB"
    );
    Ok(())
}
/// Load bounded preferences from the current account store.
pub fn preferences(store: &MessageStore) -> Result<Preferences> {
    let raw: Option<String> = store
        .conn
        .query_row(
            "SELECT value FROM tool_config WHERE key='preferences'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    Ok(raw
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_default())
}
/// Save an explicitly edited account configuration atomically.
pub fn save_preferences(store: &MessageStore, mut config: Preferences) -> Result<Preferences> {
    config.validate(crate::login::unix_now() * 1000)?;
    let text = serde_json::to_string(&config)?;
    ensure!(text.len() <= 1024 * 1024, "Cấu hình vượt 1 MiB");
    let tx = store.conn.unchecked_transaction()?;
    tx.execute("INSERT INTO tool_config(key,value) VALUES('preferences',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[text])?;
    tx.execute("UPDATE scheduled_messages SET status='cancelled' WHERE status='pending' AND id LIKE 'auto:%'",[])?;
    tx.commit()?;
    Ok(config)
}
#[derive(Clone, Debug)]
pub struct SearchHit {
    pub thread_id: String,
    pub message_id: String,
    pub text: String,
    pub time: i64,
    pub thread_name: String,
}
/// Search all stored rows without hydrating histories into GUI memory.
pub fn search(store: &MessageStore, query: &str, hidden: bool) -> Result<Vec<SearchHit>> {
    ensure!(query.len() <= 512, "Từ khóa quá dài");
    if query.trim().len() < 2 {
        return Ok(Vec::new());
    }
    let needle = fold(query.trim());
    store.conn.create_scalar_function(
        "ctdo_fold",
        1,
        rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC
            | rusqlite::functions::FunctionFlags::SQLITE_UTF8,
        move |ctx| Ok(fold(&ctx.get::<String>(0)?)),
    )?;
    store.conn.create_scalar_function(
        "ctdo_visible",
        2,
        rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC
            | rusqlite::functions::FunctionFlags::SQLITE_UTF8,
        |ctx| {
            let id = ctx.get::<String>(0)?;
            if id.starts_with("call_") {
                return Ok(false);
            }
            let raw = ctx.get::<String>(1)?;
            let mut m = ChatMessage {
                id,
                ..Default::default()
            };
            if raw.len() <= 1024 * 1024 {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if let Some(state) = v.get("message_state") {
                        let _ = m.restore_state(state);
                    } else {
                        m.history_source = Some(v);
                    }
                }
            }
            Ok(!crate::message_view::is_call_log(&m))
        },
    )?;
    let pattern = format!(
        "%{}%",
        needle
            .replace('!', "!!")
            .replace('%', "!%")
            .replace('_', "!_")
    );
    let mut statement=store.conn.prepare("SELECT m.thread_id,m.msg_id,COALESCE(m.text,''),m.ts,COALESCE(t.name,m.thread_id) FROM messages m LEFT JOIN threads t ON t.thread_id=m.thread_id LEFT JOIN thread_preferences p ON p.thread_id=m.thread_id WHERE (?2 OR COALESCE(p.is_hidden,0)=0) AND m.msg_id NOT GLOB 'call_*' AND COALESCE(m.msg_type,'') NOT IN ('chat.call','call','voip') AND ctdo_visible(m.msg_id,COALESCE(m.raw_json,'')) AND ctdo_fold(COALESCE(m.text,'')) LIKE ?1 ESCAPE '!' ORDER BY m.ts DESC LIMIT 50")?;
    let rows = statement.query_map(params![pattern, hidden], |r| {
        Ok(SearchHit {
            thread_id: r.get(0)?,
            message_id: r.get(1)?,
            text: r.get(2)?,
            time: r.get(3)?,
            thread_name: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
fn fold(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scheduled {
    pub id: String,
    pub owner: String,
    pub thread_id: String,
    pub deliver_at: i64,
    pub message: ChatMessage,
    pub status: String,
    pub error: String,
}
/// Persist a scheduled send; its client identity is reused by all explicit retries.
pub fn schedule(
    store: &MessageStore,
    owner: &str,
    thread: &str,
    group: bool,
    text: &str,
    at: i64,
) -> Result<()> {
    let now = crate::login::unix_now();
    ensure!(
        !owner.is_empty()
            && !thread.is_empty()
            && !text.trim().is_empty()
            && text.len() <= 16 * 1024
            && at > now
            && at <= now + 366 * 86400,
        "Nội dung, hội thoại hoặc giờ hẹn không hợp lệ (tối đa 366 ngày)"
    );
    ensure!(
        store.conn.query_row(
            "SELECT count(*) FROM scheduled_messages WHERE status='pending'",
            [],
            |r| r.get::<_, i64>(0)
        )? < 1000,
        "Đã có 1000 tin đang hẹn"
    );
    let message = crate::outgoing::pending(text.into(), group, crate::outgoing::Payload::Text);
    insert_job(store, owner, thread, at, &message, None)
}
fn insert_job(
    store: &MessageStore,
    owner: &str,
    thread: &str,
    at: i64,
    message: &ChatMessage,
    id: Option<&str>,
) -> Result<()> {
    store.conn.execute("INSERT OR IGNORE INTO scheduled_messages(id,owner,thread_id,deliver_at,message_json,status,error) VALUES(?1,?2,?3,?4,?5,'pending','')",params![id.unwrap_or(&message.client_message_id),owner,thread,at,serde_json::to_string(message)?])?;
    Ok(())
}
/// Return a bounded management list belonging to the current account only.
pub fn scheduled(store: &MessageStore, owner: &str) -> Result<Vec<Scheduled>> {
    let mut st=store.conn.prepare("SELECT id,thread_id,deliver_at,message_json,status,error FROM scheduled_messages WHERE owner=?1 ORDER BY deliver_at DESC LIMIT 100")?;
    let raw = st.query_map([owner], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
        ))
    })?;
    raw.map(|r| {
        let (id, thread_id, deliver_at, message, status, error) = r?;
        Ok(Scheduled {
            id,
            owner: owner.into(),
            thread_id,
            deliver_at,
            message: serde_json::from_str(&message)?,
            status,
            error,
        })
    })
    .collect()
}
/// Claim at most one due job transactionally; uncertain deliveries are never auto-replayed.
pub fn claim(store: &MessageStore, owner: &str, now: i64) -> Result<Option<Scheduled>> {
    let tx = store.conn.unchecked_transaction()?;
    let paused: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM tool_config WHERE key='scheduler_paused' AND value='1')",
        [],
        |r| r.get(0),
    )?;
    if paused {
        tx.commit()?;
        return Ok(None);
    }
    let id:Option<String>=tx.query_row("SELECT id FROM scheduled_messages WHERE owner=?1 AND status='pending' AND deliver_at<=?2 ORDER BY deliver_at LIMIT 1",params![owner,now],|r|r.get(0)).optional()?;
    let Some(id) = id else {
        tx.commit()?;
        return Ok(None);
    };
    let job = scheduled_by_id(store, owner, &id)?.context("Tin hẹn giờ không còn tồn tại")?;
    if job.id.starts_with("auto:") {
        if job.deliver_at < now - 60 {
            tx.execute(
                "UPDATE scheduled_messages SET status='cancelled' WHERE id=?1",
                [&id],
            )?;
            tx.commit()?;
            return Ok(None);
        }
        let attempts:i64=tx.query_row("SELECT count(*) FROM scheduled_messages WHERE owner=?1 AND started_at>?2 AND id LIKE 'auto:%'",params![owner,now-60],|r|r.get(0))?;
        if attempts >= 5 {
            tx.commit()?;
            return Ok(None);
        }
    }
    if has_ack(store, &job)? {
        tx.execute(
            "UPDATE scheduled_messages SET status='sent',error='' WHERE id=?1",
            [&id],
        )?;
        tx.commit()?;
        return Ok(None);
    }
    tx.execute(
        "UPDATE scheduled_messages SET status='sending',started_at=?2 WHERE id=?1 AND status='pending'",
        params![id,now],
    )?;
    tx.commit()?;
    scheduled_by_id(store, owner, &id)
}
fn has_ack(store: &MessageStore, job: &Scheduled) -> Result<bool> {
    Ok(store.conn.query_row("SELECT EXISTS(SELECT 1 FROM messages WHERE thread_id=?1 AND is_outgoing=1 AND json_valid(raw_json) AND json_extract(raw_json,'$.message_state.client_message_id')=?2 AND json_extract(raw_json,'$.message_state.delivery') IN ('server_accepted','delivered','read'))",params![job.thread_id,job.message.client_message_id],|r|r.get(0))?)
}
/// Explicitly resume pending work after the user has inspected the failure.
pub fn resume(store: &MessageStore) -> Result<()> {
    store
        .conn
        .execute("DELETE FROM tool_config WHERE key='scheduler_paused'", [])?;
    Ok(())
}
fn pause(store: &MessageStore) -> Result<()> {
    store.conn.execute("INSERT INTO tool_config(key,value) VALUES('scheduler_paused','1') ON CONFLICT(key) DO UPDATE SET value='1'",[])?;
    Ok(())
}
fn scheduled_by_id(store: &MessageStore, owner: &str, id: &str) -> Result<Option<Scheduled>> {
    let raw:Option<(String,i64,String,String,String)>=store.conn.query_row("SELECT thread_id,deliver_at,message_json,status,error FROM scheduled_messages WHERE owner=?1 AND id=?2",params![owner,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
    raw.map(|(thread_id, deliver_at, message, status, error)| {
        Ok(Scheduled {
            id: id.into(),
            owner: owner.into(),
            thread_id,
            deliver_at,
            message: serde_json::from_str(&message)?,
            status,
            error,
        })
    })
    .transpose()
}
/// Mark abandoned in-flight jobs uncertain once on worker startup/account login.
pub fn recover(store: &MessageStore, owner: &str) -> Result<()> {
    let tx = store.conn.unchecked_transaction()?;
    if tx.execute("UPDATE scheduled_messages SET status='uncertain',error='Lần gửi bị gián đoạn; kiểm tra hội thoại trước khi gửi lại' WHERE owner=?1 AND status='sending'",[owner])?>0 {pause(store)?;}
    tx.commit()?;
    Ok(())
}
/// Finalize a job only under its account owner.
pub fn finish(
    store: &MessageStore,
    owner: &str,
    id: &str,
    status: &str,
    error: &str,
) -> Result<()> {
    store.conn.execute(
        "UPDATE scheduled_messages SET status=?3,error=?4 WHERE owner=?1 AND id=?2",
        params![owner, id, status, error],
    )?;
    Ok(())
}
/// Cancel, edit or explicitly retry a job; never race an already claimed send.
pub fn edit_job(store: &MessageStore, owner: &str, id: &str, at: Option<i64>) -> Result<()> {
    let tx = store.conn.unchecked_transaction()?;
    let Some(mut job) = scheduled_by_id(store, owner, id)? else {
        return Ok(());
    };
    if let Some(at) = at {
        ensure!(
            at > crate::login::unix_now() && at <= crate::login::unix_now() + 366 * 86400,
            "Giờ hẹn phải ở tương lai (tối đa 366 ngày)"
        );
        ensure!(
            !has_ack(store, &job)?,
            "Tin đã được máy chủ xác nhận; không gửi lại"
        );
        if job.status != "pending" {
            if let Some(ref mut payload) = job.message.outgoing_job {
                payload.attempt = payload.attempt.saturating_add(1);
            }
        }
        tx.execute("UPDATE scheduled_messages SET deliver_at=?3,message_json=?4,status='pending',error='' WHERE owner=?1 AND id=?2 AND status IN ('pending','failed','uncertain')",params![owner,id,at,serde_json::to_string(&job.message)?])?;
    } else {
        tx.execute("UPDATE scheduled_messages SET status='cancelled' WHERE owner=?1 AND id=?2 AND status IN ('pending','failed','uncertain')",params![owner,id])?;
    }
    tx.commit()?;
    Ok(())
}
/// Edit only an unsent first attempt; uncertain retries preserve the original body.
pub fn update_pending(
    store: &MessageStore,
    owner: &str,
    id: &str,
    text: &str,
    at: i64,
) -> Result<()> {
    let tx = store.conn.unchecked_transaction()?;
    let mut job = scheduled_by_id(store, owner, id)?.context("Không tìm thấy tin hẹn giờ")?;
    ensure!(
        job.status == "pending"
            && job
                .message
                .outgoing_job
                .as_ref()
                .is_some_and(|j| j.attempt == 1)
            && !has_ack(store, &job)?,
        "Chỉ sửa nội dung tin chưa từng gửi"
    );
    ensure!(
        !text.trim().is_empty()
            && text.len() <= 16 * 1024
            && at > crate::login::unix_now()
            && at <= crate::login::unix_now() + 366 * 86400,
        "Nội dung hoặc giờ hẹn không hợp lệ"
    );
    job.message.text = text.into();
    tx.execute("UPDATE scheduled_messages SET deliver_at=?3,message_json=?4 WHERE owner=?1 AND id=?2 AND status='pending'",params![owner,id,at,serde_json::to_string(&job.message)?])?;
    tx.commit()?;
    Ok(())
}
/// Read the scheduling pause state for the current account store.
pub fn paused(store: &MessageStore) -> Result<bool> {
    Ok(store.conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tool_config WHERE key='scheduler_paused' AND value='1')",
        [],
        |r| r.get(0),
    )?)
}
/// Load a bounded local context around one exact search result, without network I/O.
pub fn open_hit(store: &MessageStore, thread: &str, id: &str) -> Result<Thread> {
    ensure!(thread.len() <= 512 && id.len() <= 512, "Định danh quá dài");
    let ts: i64 = store
        .conn
        .query_row(
            "SELECT ts FROM messages WHERE thread_id=?1 AND msg_id=?2",
            params![thread, id],
            |r| r.get(0),
        )
        .context("Tin không còn trong lịch sử cục bộ")?;
    let (name, group): (String, bool) = store
        .conn
        .query_row(
            "SELECT COALESCE(name,thread_id),COALESCE(is_group,0) FROM threads WHERE thread_id=?1",
            [thread],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .context("Hội thoại chưa được nhận diện; chỉ có thể xem kết quả tìm kiếm")?;
    let mut messages = Vec::new();
    for sql in ["SELECT msg_id,COALESCE(sender_name,''),COALESCE(text,''),COALESCE(ts,0),COALESCE(is_outgoing,0),COALESCE(raw_json,'') FROM messages WHERE thread_id=?1 AND ts<=?2 ORDER BY ts DESC,msg_id DESC LIMIT 40","SELECT msg_id,COALESCE(sender_name,''),COALESCE(text,''),COALESCE(ts,0),COALESCE(is_outgoing,0),COALESCE(raw_json,'') FROM messages WHERE thread_id=?1 AND ts>?2 ORDER BY ts ASC,msg_id ASC LIMIT 40"] {
        let mut stmt=store.conn.prepare(sql)?;
        let rows=stmt.query_map(params![thread,ts],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?,r.get::<_,bool>(4)?,r.get::<_,String>(5)?)))?;
        for row in rows {let(id,sender,text,ts,is_outgoing,raw)=row?;let mut m=ChatMessage{id,sender,text,ts,is_outgoing,..Default::default()};restore_import_metadata(&mut m,&raw)?;messages.push(m);}
    }
    if !messages.iter().any(|m| m.id == id) {
        let(sender,text,ts,is_outgoing,raw):(String,String,i64,bool,String)=store.conn.query_row("SELECT COALESCE(sender_name,''),COALESCE(text,''),COALESCE(ts,0),COALESCE(is_outgoing,0),COALESCE(raw_json,'') FROM messages WHERE thread_id=?1 AND msg_id=?2",params![thread,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
        let mut m = ChatMessage {
            id: id.into(),
            sender,
            text,
            ts,
            is_outgoing,
            ..Default::default()
        };
        restore_import_metadata(&mut m, &raw)?;
        messages.push(m);
    }
    messages.sort_by(|a, b| a.ts.cmp(&b.ts).then_with(|| a.id.cmp(&b.id)));
    Ok(Thread {
        id: thread.into(),
        name,
        is_group: group,
        messages,
        ..Default::default()
    })
}
/// Find one adjacent visual attachment using the thread/time index, without hydrating history.
/// # Errors
/// Returns database failures or an unavailable anchor.
pub(crate) fn media_neighbor(
    store: &MessageStore,
    thread: &str,
    anchor: &str,
    next: bool,
) -> Result<Option<ChatMessage>> {
    ensure!(
        thread.len() <= 512 && anchor.len() <= 512,
        "Định danh quá dài"
    );
    let ts: i64 = store.conn.query_row(
        "SELECT ts FROM messages WHERE thread_id=?1 AND msg_id=?2",
        params![thread, anchor],
        |r| r.get(0),
    )?;
    store.conn.create_scalar_function(
        "ctdo_visual",
        3,
        rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC
            | rusqlite::functions::FunctionFlags::SQLITE_UTF8,
        |ctx| {
            let mut m = ChatMessage {
                text: ctx.get(0)?,
                ..Default::default()
            };
            let raw: String = ctx.get(1)?;
            let kind: String = ctx.get(2)?;
            if raw.len() > 1024 * 1024 {
                return Ok(false);
            }
            if restore_import_metadata(&mut m, &raw).is_err() {
                return Ok(false);
            }
            if m.kind == crate::message_model::MessageKind::Unknown {
                m.kind = crate::message_model::MessageKind::from_wire_name(&kind);
            }
            Ok(!matches!(
                m.kind,
                crate::message_model::MessageKind::Recall
                    | crate::message_model::MessageKind::Delete
            ) && matches!(
                crate::message_view::media(&m).kind,
                crate::message_view::MediaKind::Photo | crate::message_view::MediaKind::Video
            ))
        },
    )?;
    let sql = if next {
        "SELECT msg_id,COALESCE(sender_name,''),COALESCE(text,''),ts,COALESCE(is_outgoing,0),COALESCE(raw_json,''),COALESCE(msg_type,'') FROM messages WHERE thread_id=?1 AND (ts,msg_id)>(?2,?3) AND ctdo_visual(COALESCE(text,''),COALESCE(raw_json,''),COALESCE(msg_type,'')) ORDER BY ts ASC,msg_id ASC LIMIT 1"
    } else {
        "SELECT msg_id,COALESCE(sender_name,''),COALESCE(text,''),ts,COALESCE(is_outgoing,0),COALESCE(raw_json,''),COALESCE(msg_type,'') FROM messages WHERE thread_id=?1 AND (ts,msg_id)<(?2,?3) AND ctdo_visual(COALESCE(text,''),COALESCE(raw_json,''),COALESCE(msg_type,'')) ORDER BY ts DESC,msg_id DESC LIMIT 1"
    };
    let row = store
        .conn
        .query_row(sql, params![thread, ts, anchor], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, bool>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
            ))
        })
        .optional()?;
    row.map(|(id, sender, text, ts, is_outgoing, raw, kind)| {
        let mut m = ChatMessage {
            id,
            sender,
            text,
            ts,
            is_outgoing,
            ..Default::default()
        };
        restore_import_metadata(&mut m, &raw)?;
        if m.kind == crate::message_model::MessageKind::Unknown {
            m.kind = crate::message_model::MessageKind::from_wire_name(&kind);
        }
        Ok(m)
    })
    .transpose()
}
fn restore_import_metadata(message: &mut ChatMessage, raw: &str) -> Result<()> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        if let Some(state) = value.get("message_state") {
            message.restore_state(state)?;
        }
        let mut encoded = serde_json::to_value(&*message)?;
        for key in [
            "is_pinned",
            "is_starred",
            "link_url",
            "link_title",
            "link_desc",
            "link_thumb",
            "link_src",
            "reply_to_id",
            "reply_to_sender",
            "reply_to_text",
        ] {
            if let Some(v) = value.get(key) {
                encoded[key] = v.clone();
            }
        }
        for (source, target) in [
            ("stickerId", "sticker_id"),
            ("cateId", "sticker_cate_id"),
            ("type", "sticker_type"),
        ] {
            if let Some(v) = value.get(source) {
                encoded[target] = v.clone();
            }
        }
        *message = serde_json::from_value(encoded)?;
    }
    Ok(())
}
/// Deduplicate incoming live messages and queue at most one matching reply per message.
pub fn auto_reply(store: &MessageStore, owner: &str, threads: &[Thread]) -> Result<usize> {
    let config = preferences(store)?;
    if !config.rules.iter().any(|r| r.enabled) {
        return Ok(0);
    }
    let now = crate::login::unix_now();
    let mut count = 0;
    let tx = store.conn.unchecked_transaction()?;
    for thread in threads {
        for message in &thread.messages {
            if message.is_outgoing
                || message.id.is_empty()
                || message.kind == crate::message_model::MessageKind::System
                || !message.unread_policy.counts_unread()
            {
                continue;
            }
            let text = message.text.to_lowercase();
            let Some(rule) = config.rules.iter().find(|r| {
                r.enabled
                    && r.enabled_since > 0
                    && message.ts >= r.enabled_since
                    && message.ts <= now * 1000 + 60_000
                    && (r.scope == "all" || (r.scope == "group") == thread.is_group)
                    && {
                        let needle = r.keyword.to_lowercase();
                        match r.mode.as_str() {
                            "exact" => text == needle,
                            "prefix" => text.starts_with(&needle),
                            "contains" => text.contains(&needle),
                            _ => false,
                        }
                    }
            }) else {
                continue;
            };
            let fresh = tx.execute(
                "INSERT OR IGNORE INTO auto_reply_seen(thread_id,msg_id,seen_at) VALUES(?1,?2,?3)",
                params![thread.id, message.id, now],
            )? > 0;
            if !fresh {
                continue;
            }
            let recent:i64=tx.query_row("SELECT count(*) FROM scheduled_messages WHERE owner=?1 AND deliver_at>?2 AND id LIKE 'auto:%'",params![owner,now-60],|r|r.get(0))?;
            if recent >= 5 {
                continue;
            }
            let outgoing = crate::outgoing::pending(
                rule.reply_text.clone(),
                thread.is_group,
                crate::outgoing::Payload::Text,
            );
            let id = format!("auto:{}:{}", thread.id, message.id);
            insert_job(store, owner, &thread.id, now, &outgoing, Some(&id))?;
            count += 1;
        }
    }
    tx.commit()?;
    Ok(count)
}
/// Import app-compatible SQLite/JSON only after explicit ownership confirmation.
/// Sources are read-only; one outer transaction prevents partially accepted imports.
pub fn import_history(
    store: &mut MessageStore,
    source: &std::path::Path,
    owner: &str,
    confirmed: bool,
    active: &dyn Fn() -> bool,
) -> Result<usize> {
    ensure!(
        confirmed && !owner.is_empty(),
        "Cần xác nhận dữ liệu thuộc tài khoản đang đăng nhập"
    );
    let mut paths = Vec::new();
    if source.is_dir() {
        for entry in std::fs::read_dir(source)?.take(101) {
            let p = entry?.path();
            if p.is_file()
                && (matches!(
                    p.file_name().and_then(|s| s.to_str()),
                    Some("threads_cache.json" | "export.json")
                ) || matches!(
                    p.extension().and_then(|x| x.to_str()),
                    Some("db" | "sqlite" | "sqlite3")
                ))
            {
                paths.push(p);
            }
        }
        ensure!(paths.len() <= 100, "Tối đa 100 nguồn mỗi lần nhập");
    } else {
        paths.push(source.to_owned());
    }
    ensure!(
        !paths.is_empty(),
        "Không tìm thấy nguồn SQLite/JSON phù hợp"
    );
    let tx = store.conn.unchecked_transaction()?;
    let mut inserted = 0;
    for path in paths {
        ensure!(active(), "Đã dừng nhập vì tài khoản đã đăng xuất");
        let meta = path.symlink_metadata()?;
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "Nguồn phải là tập tin thường"
        );
        if matches!(
            path.extension().and_then(|x| x.to_str()),
            Some("db" | "sqlite" | "sqlite3")
        ) {
            if let Some(destination) = store
                .conn
                .path()
                .filter(|p| !p.is_empty() && *p != ":memory:")
            {
                ensure!(
                    path.canonicalize()? != std::path::Path::new(destination).canonicalize()?,
                    "Nguồn trùng database đang sử dụng"
                );
            }
            let src = rusqlite::Connection::open_with_flags(
                &path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            src.pragma_update(None, "query_only", true)?;
            let has_owner:bool=src.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='history_export')",[],|r|r.get(0))?;
            if has_owner {
                let source_owner: String =
                    src.query_row("SELECT owner FROM history_export LIMIT 1", [], |r| r.get(0))?;
                ensure!(source_owner == owner, "Nguồn lịch sử thuộc tài khoản khác");
            }

            let oversized:i64=src.query_row("SELECT EXISTS(SELECT 1 FROM messages WHERE length(COALESCE(text,''))>1048576 OR length(COALESCE(raw_json,''))>1048576)",[],|r|r.get(0))?;
            ensure!(oversized == 0, "Nguồn có tin vượt giới hạn nhập 1 MiB");
            let mut stmt=src.prepare("SELECT thread_id,msg_id,COALESCE(sender_name,''),COALESCE(text,''),ts,is_outgoing,COALESCE(raw_json,'') FROM messages ORDER BY thread_id,ts")?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, bool>(5)?,
                    r.get::<_, String>(6)?,
                ))
            })?;
            for row in rows {
                ensure!(active(), "Đã đăng xuất");
                let (thread_id, id, sender, text, ts, is_outgoing, raw) = row?;
                ensure!(
                    text.len() + raw.len() <= 1024 * 1024,
                    "Một tin vượt giới hạn nhập"
                );
                let mut m = ChatMessage {
                    id,
                    sender,
                    text,
                    ts,
                    is_outgoing,
                    ..Default::default()
                };
                restore_import_metadata(&mut m, &raw)?;
                let metadata: Option<(String, bool)> = src
                    .query_row(
                        "SELECT COALESCE(name,thread_id),COALESCE(is_group,0) FROM threads WHERE thread_id=?1",
                        [&thread_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                let (name, group) = metadata.context("Lịch sử nguồn thiếu thông tin hội thoại")?;
                store.upsert_thread(&crate::store::StoredThread {
                    thread_id: thread_id.clone(),
                    name,
                    is_group: group,
                    last_ts: ts,
                })?;
                inserted += import_message(store, &thread_id, m)?;
            }
        } else {
            ensure!(
                meta.len() <= 32 * 1024 * 1024,
                "Nguồn JSON tối đa 32 MiB; dùng SQLite cho lịch sử lớn"
            );
            let value: serde_json::Value =
                serde_json::from_slice(&crate::limits::read_file(&path, 32 * 1024 * 1024)?)?;
            if let Some(declared) = value.get("owner").and_then(|v| v.as_str()) {
                ensure!(declared == owner, "Nguồn thuộc tài khoản khác");
            }
            let data = value.get("threads").unwrap_or(&value).clone();
            let threads: Vec<Thread> = serde_json::from_value(data)
                .context("JSON phải là danh sách hội thoại tương thích")?;
            for thread in threads {
                store.upsert_thread(&crate::store::StoredThread {
                    thread_id: thread.id.clone(),
                    name: thread.name.clone(),
                    is_group: thread.is_group,
                    last_ts: thread.messages.iter().map(|m| m.ts).max().unwrap_or(0),
                })?;
                for message in thread.messages {
                    inserted += import_message(store, &thread.id, message)?;
                }
            }
        }
    }
    ensure!(active(), "Đã đăng xuất");
    tx.commit()?;
    Ok(inserted)
}
fn import_message(store: &MessageStore, thread: &str, mut message: ChatMessage) -> Result<usize> {
    ensure!(
        !thread.is_empty()
            && !message.id.is_empty()
            && thread.len() <= 512
            && message.id.len() <= 512,
        "Thiếu định danh lịch sử"
    );
    message.ts = crate::clock::normalize_ts_ms(message.ts);
    // External pending jobs cannot authorize a send or import local file retry payloads.
    message.outgoing_job = None;
    store.conn.execute(
        "INSERT OR IGNORE INTO threads(thread_id,name,is_group,last_ts) VALUES(?1,?1,0,?2)",
        params![thread, message.ts],
    )?;
    let mut sm = crate::store::StoredMessage::from(&message);
    sm.thread_id = thread.into();
    store.reconcile_send_identity(&message, &mut sm)?;
    Ok(usize::from(store.upsert_message(&sm)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> MessageStore {
        MessageStore::open(":memory:").unwrap()
    }
    fn thread(text: &str, id: &str) -> Thread {
        Thread {
            id: "thread".into(),
            name: "Hội thoại".into(),
            messages: vec![ChatMessage {
                id: id.into(),
                text: text.into(),
                sender: "Bạn".into(),
                ts: crate::login::unix_now() * 1000,
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    #[test]
    fn search_includes_unloaded_history_accents_and_literal_wildcards() {
        let mut s = store();
        crate::store::persist_threads(&mut s, &[thread("Tiếng Việt 100% an_toàn", "1")]).unwrap();
        assert_eq!(search(&s, "tieng viet", false).unwrap().len(), 1);
        assert_eq!(search(&s, "100%", false).unwrap().len(), 1);
        assert_eq!(search(&s, "an_toan", false).unwrap().len(), 1);
        assert!(search(&s, "100_", false).unwrap().is_empty());
        s.conn
            .execute(
                "INSERT INTO thread_preferences(thread_id,is_hidden) VALUES('thread',1)",
                [],
            )
            .unwrap();
        assert!(search(&s, "tieng", false).unwrap().is_empty());
        assert_eq!(search(&s, "tieng", true).unwrap().len(), 1);
    }
    #[test]
    fn scheduled_claim_is_owner_scoped_and_recovery_never_replays() {
        let s = store();
        let now = crate::login::unix_now();
        schedule(&s, "owner", "thread", false, "hello", now + 60).unwrap();
        assert!(claim(&s, "other", now + 100).unwrap().is_none());
        let job = claim(&s, "owner", now + 100).unwrap().unwrap();
        assert!(claim(&s, "owner", now + 100).unwrap().is_none());
        recover(&s, "owner").unwrap();
        let saved = scheduled(&s, "owner").unwrap();
        assert_eq!(saved[0].status, "uncertain");
        assert!(claim(&s, "owner", now + 100).unwrap().is_none());
        edit_job(&s, "other", &job.id, Some(now + 120)).unwrap();
        assert_eq!(scheduled(&s, "owner").unwrap()[0].status, "uncertain");
        edit_job(&s, "owner", &job.id, Some(now + 120)).unwrap();
        resume(&s).unwrap();
        let retry = claim(&s, "owner", now + 130).unwrap().unwrap();
        assert_eq!(
            job.message.client_message_id,
            retry.message.client_message_id
        );
    }
    #[test]
    fn configuration_rejects_invalid_theme_and_sticker_ids() {
        let s = store();
        let mut c = Preferences {
            theme: serde_json::json!({"dark":{"primary":"oops"}}),
            ..Default::default()
        };
        assert!(save_preferences(&s, c.clone()).is_err());
        c.theme = serde_json::json!({"dark":{"primary":"#123ABC"}});
        c.stickers = vec![StickerPack {
            cate_id: 1,
            name: "test".into(),
            sticker_ids: vec![0],
            sticker_type: 7,
        }];
        assert!(save_preferences(&s, c).is_err());
        assert!(preferences(&s).unwrap().theme.is_null());
    }
    #[test]
    fn autoreply_is_opt_in_deduplicated_and_does_not_process_history() {
        let mut s = store();
        let now = crate::login::unix_now() * 1000;
        assert_eq!(auto_reply(&s, "owner", &[thread("hello", "1")]).unwrap(), 0);
        let config = Preferences {
            rules: vec![ReplyRule {
                id: "r".into(),
                keyword: "hello".into(),
                reply_text: "reply".into(),
                mode: "exact".into(),
                scope: "direct".into(),
                enabled: true,
                enabled_since: 0,
            }],
            ..Default::default()
        };
        save_preferences(&s, config).unwrap();
        let incoming = thread("hello", "1");
        crate::store::persist_threads(&mut s, std::slice::from_ref(&incoming)).unwrap();
        assert_eq!(
            auto_reply(&s, "owner", std::slice::from_ref(&incoming)).unwrap(),
            1
        );
        assert_eq!(auto_reply(&s, "owner", &[incoming]).unwrap(), 0);
        let mut old = thread("hello", "old");
        old.messages[0].ts = now - 86400000;
        assert_eq!(auto_reply(&s, "owner", &[old]).unwrap(), 0);
        let mut own = thread("hello", "own");
        own.messages[0].is_outgoing = true;
        assert_eq!(auto_reply(&s, "owner", &[own]).unwrap(), 0);
        for i in 2..10 {
            auto_reply(&s, "owner", &[thread("hello", &i.to_string())]).unwrap();
        }
        assert_eq!(scheduled(&s, "owner").unwrap().len(), 5);
    }
    #[test]
    fn sqlite_import_is_read_only_idempotent_and_preserves_metadata() {
        let root = std::env::temp_dir().join(format!(
            "ctdo-import-{}",
            crate::param_cipher::random_hex(12)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("source.db");
        {
            let mut src = MessageStore::open(&path).unwrap();
            let mut t = thread("[Nhãn dán]", "server-1");
            t.messages[0].sticker_id = Some(123);
            t.messages[0].sticker_cate_id = Some(456);
            crate::store::persist_threads(&mut src, &[t]).unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        let mut dst = store();
        assert!(import_history(&mut dst, &path, "owner", false, &|| true).is_err());
        assert_eq!(
            import_history(&mut dst, &path, "owner", true, &|| true).unwrap(),
            1
        );
        assert_eq!(
            import_history(&mut dst, &path, "owner", true, &|| true).unwrap(),
            0
        );
        assert_eq!(before, std::fs::read(&path).unwrap());
        let mut threads = Vec::new();
        dst.hydrate_threads(&mut threads, 20).unwrap();
        assert_eq!(threads[0].name, "Hội thoại");
        assert_eq!(threads[0].messages[0].sticker_id, Some(123));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn import_rollback_preserves_destination_when_a_later_message_is_invalid() {
        let path = std::env::temp_dir().join(format!(
            "ctdo-import-{}.json",
            crate::param_cipher::random_hex(12)
        ));
        let mut t = thread("valid", "1");
        t.messages.push(ChatMessage {
            text: "invalid".into(),
            ..Default::default()
        });
        std::fs::write(&path, serde_json::to_vec(&vec![t]).unwrap()).unwrap();
        let mut dst = store();
        assert!(import_history(&mut dst, &path, "owner", true, &|| true).is_err());
        assert!(dst.messages_for("thread", 20).unwrap().is_empty());
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod delivery_and_navigation_tests {
    use super::*;
    #[test]
    fn acknowledged_scheduled_send_is_not_sent_again_after_manual_retry_or_echo() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let now = crate::login::unix_now();
        schedule(&store, "owner", "thread", false, "hello", now + 60).unwrap();
        let job = scheduled(&store, "owner").unwrap().remove(0);
        let mut ack = job.message.clone();
        ack.id = "server-ack".into();
        ack.delivery = crate::message_model::DeliveryState::ServerAccepted;
        ack.outgoing_job = None;
        crate::store::persist_threads(
            &mut store,
            &[Thread {
                id: "thread".into(),
                messages: vec![ack],
                ..Default::default()
            }],
        )
        .unwrap();
        assert!(edit_job(&store, "owner", &job.id, Some(now + 120)).is_err());
        assert!(claim(&store, "owner", now + 130).unwrap().is_none());
        assert_eq!(scheduled(&store, "owner").unwrap()[0].status, "sent");
    }
    #[test]
    fn pending_edit_cannot_change_an_uncertain_payload() {
        let store = MessageStore::open(":memory:").unwrap();
        let now = crate::login::unix_now();
        schedule(&store, "owner", "thread", false, "old", now + 60).unwrap();
        let job = scheduled(&store, "owner").unwrap().remove(0);
        update_pending(&store, "owner", &job.id, "new", now + 120).unwrap();
        assert_eq!(scheduled(&store, "owner").unwrap()[0].message.text, "new");
        claim(&store, "owner", now + 130).unwrap();
        recover(&store, "owner").unwrap();
        assert!(update_pending(&store, "owner", &job.id, "different", now + 200).is_err());
        edit_job(&store, "owner", &job.id, Some(now + 200)).unwrap();
        assert!(update_pending(&store, "owner", &job.id, "different", now + 200).is_err());
    }
    #[test]
    fn opening_an_old_hit_reads_bounded_context_and_uses_exact_thread_id() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let t = Thread {
            id: "thread".into(),
            name: "Tên nhóm".into(),
            is_group: true,
            messages: (0..500)
                .map(|i| ChatMessage {
                    id: i.to_string(),
                    text: format!("row {i}"),
                    ts: i,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        crate::store::persist_threads(&mut store, &[t]).unwrap();
        let context = open_hit(&store, "thread", "5").unwrap();
        assert!(context.messages.len() <= 81);
        assert!(context.messages.iter().any(|m| m.id == "5"));
        assert_eq!(context.name, "Tên nhóm");
        assert!(context.is_group);
        assert!(open_hit(&store, "other", "5").is_err());
    }
}

/// Export history only to a new compatible SQLite file, excluding automation/configuration.
/// # Errors
/// Returns cancellation, destination-exists and SQLite errors without replacing a destination.
pub fn export_history(
    store: &MessageStore,
    destination: &std::path::Path,
    owner: &str,
    active: &dyn Fn() -> bool,
) -> Result<usize> {
    use std::io::Write;
    ensure!(!owner.is_empty() && active(), "Đã đăng xuất");
    ensure!(
        !destination.exists(),
        "Tệp đích đã tồn tại; hãy chọn tên mới"
    );
    let stage = destination.with_file_name(format!(
        ".ctdo-history-{}.db",
        crate::chat::next_client_message_id()
    ));
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(&stage)?.flush()?;
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _cleanup = Cleanup(stage.clone());
    let mut target = rusqlite::Connection::open(&stage)?;
    target.execute_batch("CREATE TABLE threads(thread_id TEXT PRIMARY KEY,name TEXT,is_group INTEGER,last_ts INTEGER);CREATE TABLE messages(msg_id TEXT,thread_id TEXT,sender_id TEXT,sender_name TEXT,text TEXT,msg_type TEXT,ts INTEGER,is_outgoing INTEGER,raw_json TEXT,PRIMARY KEY(thread_id,msg_id));CREATE TABLE history_export(owner TEXT NOT NULL,version INTEGER NOT NULL);")?;
    let tx = target.transaction()?;
    tx.execute("INSERT INTO history_export VALUES(?1,1)", [owner])?;
    let snapshot = store.conn.unchecked_transaction()?;
    for table in ["threads", "messages"] {
        let (select, insert) = if table == "threads" {
            (
                "SELECT thread_id,name,is_group,last_ts FROM threads",
                "INSERT INTO threads VALUES(?1,?2,?3,?4)",
            )
        } else {
            ("SELECT msg_id,thread_id,sender_id,sender_name,text,msg_type,ts,is_outgoing,raw_json FROM messages","INSERT INTO messages VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)")
        };
        let mut read = snapshot.prepare(select)?;
        let count = read.column_count();
        let mut rows = read.query([])?;
        let mut write = tx.prepare(insert)?;
        while let Some(row) = rows.next()? {
            ensure!(active(), "Đã đăng xuất; dừng xuất lịch sử");
            let values = (0..count)
                .map(|i| row.get::<_, rusqlite::types::Value>(i))
                .collect::<rusqlite::Result<Vec<_>>>()?;
            write.execute(rusqlite::params_from_iter(values))?;
        }
    }
    let n = tx.query_row("SELECT COUNT(*) FROM messages", [], |r| {
        r.get::<_, usize>(0)
    })?;
    tx.commit()?;
    snapshot.commit()?;
    drop(target);
    ensure!(active(), "Đã đăng xuất; dừng xuất lịch sử");
    // A hard link publishes without replacing an existing user file on either platform.
    #[cfg(not(windows))]
    std::fs::hard_link(&stage, destination)
        .context("Không thể xuất tệp; hãy chọn thư mục hoặc tên khác")?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        extern "system" {
            fn MoveFileW(from: *const u16, to: *const u16) -> i32;
        }
        let from: Vec<u16> = stage.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        ensure!(
            !from[..from.len() - 1].contains(&0) && !to[..to.len() - 1].contains(&0),
            "Tên tệp không hợp lệ"
        );
        // SAFETY: both paths are NUL-terminated and remain alive through a non-replacing move.
        ensure!(
            unsafe { MoveFileW(from.as_ptr(), to.as_ptr()) } != 0,
            "Không thể xuất tệp; đích đã tồn tại hoặc ổ đĩa không cho phép ghi"
        );
    }

    Ok(n)
}

#[cfg(test)]
mod export_tests {
    use super::*;
    #[test]
    fn exported_history_round_trips_without_automation_and_checks_owner() {
        let source = MessageStore::open(":memory:").unwrap();
        source.conn.execute_batch("INSERT INTO threads VALUES('t','Thread',0,100);INSERT INTO messages(thread_id,msg_id,text,ts,is_outgoing,raw_json) VALUES('t','m','hello',100,0,'{}');INSERT INTO tool_config VALUES('private-config','must-not-export');").unwrap();
        let path = std::env::temp_dir().join(format!(
            "history-export-{}.db",
            crate::chat::next_client_message_id()
        ));
        assert_eq!(
            export_history(&source, &path, "owner-a", &|| true).unwrap(),
            1
        );
        assert!(export_history(&source, &path, "owner-a", &|| true).is_err());
        let mut destination = MessageStore::open(":memory:").unwrap();
        assert!(import_history(&mut destination, &path, "owner-b", true, &|| true).is_err());
        assert_eq!(
            import_history(&mut destination, &path, "owner-a", true, &|| true).unwrap(),
            1
        );
        assert_eq!(
            import_history(&mut destination, &path, "owner-a", true, &|| true).unwrap(),
            0
        );
        assert_eq!(
            destination
                .conn
                .query_row("SELECT COUNT(*) FROM tool_config", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod deleted_import_tests {
    use super::*;
    #[test]
    fn importing_an_old_backup_cannot_restore_a_deleted_message() {
        let root = std::env::temp_dir().join(format!(
            "ctdo-deleted-import-{}",
            crate::param_cipher::random_hex(12)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("old.db");
        let message = ChatMessage {
            id: "server-1".into(),
            text: "Tin đã xóa".into(),
            ts: 100,
            ..Default::default()
        };
        {
            let mut source = MessageStore::open(&path).unwrap();
            crate::store::persist_threads(
                &mut source,
                &[Thread {
                    id: "thread-a".into(),
                    messages: vec![message.clone()],
                    ..Default::default()
                }],
            )
            .unwrap();
        }
        let mut destination = MessageStore::open(":memory:").unwrap();
        destination
            .delete_chat_message("thread-a", &message)
            .unwrap();
        assert_eq!(
            import_history(&mut destination, &path, "owner-a", true, &|| true).unwrap(),
            0
        );
        assert!(destination.messages_for("thread-a", 20).unwrap().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod gallery_tests {
    use super::*;
    #[test]
    fn gallery_uses_time_and_id_order_and_never_crosses_threads() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let photo = |id: &str, ts: i64| ChatMessage {
            id: id.into(),
            ts,
            kind: crate::message_model::MessageKind::Photo,
            link_url: Some("https://example.invalid/photo.jpg".into()),
            ..Default::default()
        };
        let thread = Thread {
            id: "thread-a".into(),
            name: "A".into(),
            messages: vec![
                photo("001", 10),
                ChatMessage {
                    id: "002".into(),
                    ts: 10,
                    text: "plain".into(),
                    ..Default::default()
                },
                photo("003", 10),
                photo("004", 20),
            ],
            ..Default::default()
        };
        crate::store::persist_threads(
            &mut store,
            &[
                thread,
                Thread {
                    id: "thread-b".into(),
                    messages: vec![photo("other", 11)],
                    ..Default::default()
                },
            ],
        )
        .unwrap();
        assert_eq!(
            media_neighbor(&store, "thread-a", "001", true)
                .unwrap()
                .unwrap()
                .id,
            "003"
        );
        assert_eq!(
            media_neighbor(&store, "thread-a", "004", false)
                .unwrap()
                .unwrap()
                .id,
            "003"
        );
        assert!(media_neighbor(&store, "thread-a", "001", false)
            .unwrap()
            .is_none());
        assert!(media_neighbor(&store, "thread-a", "missing", true).is_err());
        store.delete_message("thread-a", "003", 10).unwrap();
        assert_eq!(
            media_neighbor(&store, "thread-a", "001", true)
                .unwrap()
                .unwrap()
                .id,
            "004"
        );
        let plan:String=store.conn.query_row("EXPLAIN QUERY PLAN SELECT msg_id FROM messages WHERE thread_id=?1 AND (ts,msg_id)>(?2,?3) ORDER BY ts,msg_id LIMIT 1",params!["thread-a",10,"001"],|r|r.get(3)).unwrap();
        assert!(plan.contains("INDEX"), "{plan}");
    }
}

#[cfg(test)]
mod media_gallery_bench {
    use super::*;
    #[test]
    #[ignore = "Run explicitly to measure indexed navigation against full hydration"]
    fn indexed_navigation_measurement() {
        let mut store = MessageStore::open(":memory:").unwrap();
        let messages = (0..500)
            .map(|i| ChatMessage {
                id: format!("{i:06}"),
                ts: i,
                kind: if i % 10 == 0 {
                    crate::message_model::MessageKind::Photo
                } else {
                    crate::message_model::MessageKind::Text
                },
                text: "fixture".into(),
                link_url: (i % 10 == 0).then(|| "https://example.invalid/photo.jpg".into()),
                ..Default::default()
            })
            .collect();
        crate::store::persist_threads(
            &mut store,
            &[Thread {
                id: "bench".into(),
                messages,
                ..Default::default()
            }],
        )
        .unwrap();
        for indexed in [false, true] {
            let mut times = Vec::new();
            let budget = std::time::Instant::now();
            for _ in 0..30 {
                let start = std::time::Instant::now();
                if indexed {
                    assert!(media_neighbor(&store, "bench", "000200", true)
                        .unwrap()
                        .is_some());
                } else {
                    let mut threads = vec![Thread {
                        id: "bench".into(),
                        ..Default::default()
                    }];
                    store.hydrate_selected_threads(&mut threads, 500).unwrap();
                    assert_eq!(threads[0].messages.len(), 500);
                }
                times.push(start.elapsed().as_secs_f64() * 1000.0);
                if budget.elapsed() > std::time::Duration::from_secs(2) {
                    break;
                }
            }
            times.sort_by(f64::total_cmp);
            let count = times.len();
            println!(
                "{}",
                serde_json::json!({"task":if indexed{"gallery_indexed_neighbor"}else{"full_thread_hydration_reference"},"rows":500,"iterations":count,"min_ms":times[0],"avg_ms":times.iter().sum::<f64>()/count as f64,"p50_ms":times[count/2],"p95_ms":times[(count*95/100).min(count-1)],"p99_ms":times[count-1]})
            );
        }
    }
}
