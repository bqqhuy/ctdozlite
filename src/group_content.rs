//! Separate plain-text group notes and GROUP_POLL operations from timed reminders.
use crate::{group_state, session::Session};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

/// Parsed server poll; unknown fields remain available for forward compatibility.
#[derive(Clone, Debug)]
pub struct Poll {
    pub id: String,
    pub question: String,
    pub creator: String,
    pub closed: bool,
    pub multi: bool,
    pub anonymous: bool,
    pub add_options: bool,
    pub options: Vec<OptionItem>,
    pub raw: Value,
}
/// An option identity is supplied by the server, never inferred from its label.
#[derive(Clone, Debug)]
pub struct OptionItem {
    pub id: i64,
    pub text: String,
    pub votes: Option<u64>,
    pub selected: bool,
}
/// Group content intents; non-idempotent mutations are not retried automatically.
#[derive(Clone, Debug)]
pub enum Action {
    ListNotes {
        page: u32,
        last: String,
        last_type: i64,
    },
    SaveNote {
        id: Option<String>,
        text: String,
        creator: String,
    },
    RemoveNote {
        id: String,
        creator: String,
    },
    ListPolls {
        page: u32,
        last: String,
        last_type: i64,
    },
    PollDetail {
        id: String,
    },
    CreatePoll {
        question: String,
        options: Vec<String>,
        multi: bool,
        anonymous: bool,
        allow_add: bool,
    },
    Vote {
        id: String,
        options: Vec<i64>,
    },
    AddOption {
        id: String,
        text: String,
    },
    EndPoll {
        id: String,
    },
}
impl Action {
    pub fn mutation(&self) -> bool {
        !matches!(
            self,
            Self::ListNotes { .. } | Self::ListPolls { .. } | Self::PollDetail { .. }
        )
    }
}
/// One reply keeps notes and polls distinct even when both are discovered through the board.
#[derive(Clone, Debug, Default)]
pub struct Reply {
    pub next: Option<(u32, String, i64)>,
    pub append: bool,
    pub notes: Option<Vec<crate::board::Item>>,
    pub polls: Option<Vec<Poll>>,
    pub detail: Option<Poll>,
    pub changed: bool,
}
fn id(value: Option<&Value>) -> Option<String> {
    value.and_then(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_i64().map(|value| value.to_string()))
    })
}
fn bool_value(value: Option<&Value>) -> bool {
    value.is_some_and(|value| value.as_bool().unwrap_or_else(|| value.as_i64() == Some(1)))
}
fn object(value: &Value) -> Result<Value> {
    if let Some(text) = value.as_str() {
        ensure!(text.len() <= 256 * 1024, "Dữ liệu bảng tin quá lớn");
        Ok(serde_json::from_str(text)?)
    } else {
        Ok(value.clone())
    }
}
/// Parse field identities used by the PC poll renderer, including anonymous vote counts.
pub fn parse_poll(value: Value, my_uid: &str) -> Result<Poll> {
    let value = object(&value)?;
    let raw = if value.get("boardType").is_some() {
        object(value.get("data").context("Thiếu dữ liệu poll")?)?
    } else {
        value
    };
    let identifier = id(raw.get("poll_id"))
        .filter(|id| !id.is_empty() && id.len() <= 128)
        .context("Thiếu poll_id")?;
    let question = raw
        .get("question")
        .and_then(Value::as_str)
        .context("Thiếu câu hỏi bình chọn")?
        .to_owned();
    ensure!(question.len() <= 4000, "Câu hỏi bình chọn quá dài");
    let anonymous = bool_value(raw.get("is_anonymous"));
    let options = raw
        .get("options")
        .and_then(Value::as_array)
        .context("Thiếu lựa chọn bình chọn")?;
    ensure!(options.len() <= 100, "Quá nhiều lựa chọn bình chọn");
    let mut parsed = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for option in options {
        let identifier = option
            .get("option_id")
            .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
            .context("Thiếu option_id")?;
        ensure!(seen.insert(identifier), "Lựa chọn bình chọn bị trùng ID");
        let voters = option.get("voters").and_then(Value::as_array);
        let votes = if anonymous {
            option.get("votes").and_then(Value::as_u64)
        } else {
            voters.map(|values| values.len() as u64)
        };
        parsed.push(OptionItem {
            id: identifier,
            text: option
                .get("content")
                .and_then(Value::as_str)
                .context("Thiếu nội dung lựa chọn")?
                .to_owned(),
            votes,
            selected: voters
                .is_some_and(|values| values.iter().any(|id| id.as_str() == Some(my_uid))),
        });
    }
    Ok(Poll {
        id: identifier,
        question,
        creator: id(raw.get("creator")).unwrap_or_default(),
        closed: bool_value(raw.get("closed")),
        multi: bool_value(raw.get("allow_multi_choices")),
        anonymous,
        add_options: bool_value(raw.get("allow_add_new_option")),
        options: parsed,
        raw,
    })
}
async fn call(
    s: &Session,
    host: &str,
    path: &str,
    body: Value,
    get: bool,
    mutation: bool,
) -> Result<Value> {
    ensure!(
        !host.is_empty(),
        "Server chưa cấp dịch vụ bảng tin/bình chọn; đăng nhập lại để cập nhật service map"
    );
    let params = s.encoded_params(&body)?;
    let url = format!("{host}{path}?{}", s.common_params());
    let raw = if get {
        s.get(&format!("{url}&params={params}")).await?
    } else {
        s.post_form(&url, format!("params={params}")).await?
    };
    let raw = s.maybe_decrypt(raw, s.session_key_bytes()).await;
    if mutation {
        ensure!(
            raw.get("error_code").is_some(),
            "Thiếu xác nhận server; tải lại trước khi thử tiếp"
        );
    }
    ensure!(s.is_active(), "Phiên làm việc đã đóng");
    crate::board::data(raw)
}
fn clean(group: &str) -> Result<String> {
    Ok(crate::server_pins::identity(group, true)?[1..].to_owned())
}
async fn create_allowed(s: &Session, group: &str, poll: bool) -> Result<()> {
    let state = group_state::snapshot(s, group, true).await?;
    ensure!(!state.removed, "Bạn không còn trong nhóm");
    let flag = state
        .info
        .pointer(if poll {
            "/setting/lockCreatePoll"
        } else {
            "/setting/lockCreatePost"
        })
        .context("Chưa xác minh quyền tạo nội dung nhóm")?;
    let flag = flag
        .as_i64()
        .or_else(|| flag.as_bool().map(i64::from))
        .or_else(|| flag.as_str()?.parse().ok())
        .context("Quyền tạo nội dung không hợp lệ")?;
    ensure!(
        flag == 0 || state.manager(s.account_key()),
        "Nhóm hạn chế tạo nội dung này"
    );
    Ok(())
}
async fn detail(s: &Session, id: &str) -> Result<Poll> {
    ensure!(
        !id.is_empty() && id.len() <= 128,
        "ID bình chọn không hợp lệ"
    );
    parse_poll(
        call(
            s,
            &s.domains.group_poll,
            "/api/poll/detail",
            json!({"poll_id":id,"imei":s.imei}),
            true,
            false,
        )
        .await?,
        s.account_key(),
    )
}
/// Execute one group content intent against services returned at login.
/// # Errors
/// Fails for unknown permissions, malformed replies or missing server acknowledgement.
pub async fn execute(s: &Session, group: &str, action: &Action) -> Result<Reply> {
    ensure!(
        s.is_active() && !s.account_key().is_empty(),
        "Chưa đăng nhập"
    );
    let grid = clean(group)?;
    let mut reply = Reply {
        changed: action.mutation(),
        ..Default::default()
    };
    match action {
        Action::ListNotes {
            page,
            last,
            last_type,
        }
        | Action::ListPolls {
            page,
            last,
            last_type,
        } => {
            let raw=call(s,&s.domains.group_board,"/api/board/list",json!({"group_id":grid,"board_type":if matches!(action,Action::ListPolls{..}){3}else{1},"page":page,"count":50,"last_id":last,"last_type":last_type,"imei":s.imei}),true,false).await?;
            let items = raw
                .get("items")
                .and_then(Value::as_array)
                .or_else(|| raw.as_array())
                .context("Danh sách nội dung nhóm chưa hỗ trợ định dạng này")?;
            ensure!(items.len() <= 1000, "Danh sách nội dung nhóm quá lớn");
            if items.len() >= 50 {
                if let Some(item) = items.last() {
                    let data = object(item.get("data").unwrap_or(item))?;
                    let identifier = id(data.get("id").or_else(|| data.get("poll_id")))
                        .context("Thiếu cursor trang tiếp theo")?;
                    let kind = item
                        .get("boardType")
                        .and_then(Value::as_i64)
                        .context("Thiếu loại cursor bảng tin")?;
                    ensure!(identifier != *last, "Cursor bảng tin không tiến");
                    reply.next = Some((page + 1, identifier, kind));
                }
            }
            if matches!(action, Action::ListNotes { .. }) {
                let mut notes = Vec::new();
                for value in items {
                    let raw = if value.get("boardType").is_some() {
                        object(value.get("data").context("Thiếu nội dung bảng tin")?)?
                    } else {
                        value.clone()
                    };
                    if raw.get("type").and_then(Value::as_i64) == Some(0) {
                        let note = crate::board::parse_item(value.clone())?;
                        if note.start <= 0 {
                            notes.push(note);
                        }
                    }
                }
                reply.notes = Some(notes);
            } else {
                let mut polls = Vec::new();
                for value in items {
                    let raw = if value.get("boardType").is_some() {
                        object(value.get("data").context("Thiếu nội dung bảng tin")?)?
                    } else {
                        value.clone()
                    };
                    if raw.get("poll_id").is_some() {
                        polls.push(parse_poll(raw, s.account_key())?);
                    }
                }
                reply.polls = Some(polls);
            }
        }
        Action::SaveNote { id, text, creator } => {
            ensure!(
                !text.trim().is_empty() && text.len() <= 4000,
                "Ghi chú phải có từ 1 đến 4000 byte"
            );
            create_allowed(s, group, false).await?;
            if id.is_some() {
                let state = group_state::snapshot(s, group, false).await?;
                ensure!(
                    creator == s.account_key() || state.manager(s.account_key()),
                    "Không có quyền sửa ghi chú này"
                );
            }
            let mut body = json!({"grid":grid,"type":0,"color":-16777216i64,"emoji":"","startTime":-1,"duration":-1,"params":json!({"title":text.trim()}).to_string(),"needPin":false,"repeat":0,"imei":s.imei});
            if let Some(id) = id {
                ensure!(!id.is_empty() && id.len() <= 128, "ID ghi chú không hợp lệ");
                body["topicId"] = json!(id);
            }
            call(
                s,
                &s.domains.group_board,
                if id.is_some() {
                    "/api/board/topic/update"
                } else {
                    "/api/board/topic/create"
                },
                body,
                false,
                true,
            )
            .await?;
        }
        Action::RemoveNote { id, creator } => {
            ensure!(!id.is_empty() && id.len() <= 128, "ID ghi chú không hợp lệ");
            let state = group_state::snapshot(s, group, true).await?;
            ensure!(
                !state.removed && (creator == s.account_key() || state.manager(s.account_key())),
                "Không có quyền xóa ghi chú"
            );
            call(
                s,
                &s.domains.group_board,
                "/api/board/topic/remove",
                json!({"grid":grid,"topicId":id,"imei":s.imei}),
                false,
                true,
            )
            .await?;
        }
        Action::PollDetail { id } => reply.detail = Some(detail(s, id).await?),
        Action::CreatePoll {
            question,
            options,
            multi,
            anonymous,
            allow_add,
        } => {
            create_allowed(s, group, true).await?;
            ensure!(
                !question.trim().is_empty()
                    && question.len() <= 4000
                    && options.len() >= 2
                    && options.len() <= 20,
                "Câu hỏi hoặc số lựa chọn không hợp lệ"
            );
            let mut unique = std::collections::HashSet::new();
            for option in options {
                ensure!(
                    !option.trim().is_empty()
                        && option.len() <= 1000
                        && unique.insert(option.trim()),
                    "Lựa chọn rỗng, trùng hoặc quá dài"
                );
            }
            let raw=call(s,&s.domains.group_poll,"/api/poll/create",json!({"group_id":grid,"question":question.trim(),"options":options,"allow_multi_choices":multi,"allow_add_new_option":allow_add,"is_anonymous":anonymous,"is_hide_vote_preview":false,"poll_type":0,"expired_time":0,"pinAct":0,"src":1,"imei":s.imei}),false,true).await?;
            if let Some(id) = id(raw.get("poll_id")) {
                reply.detail = Some(detail(s, &id).await?);
            }
        }
        Action::Vote { id, options } => {
            let poll = detail(s, id).await?;
            ensure!(!poll.closed, "Bình chọn đã kết thúc");
            ensure!(
                poll.multi || options.len() <= 1,
                "Bình chọn chỉ cho phép một lựa chọn"
            );
            let mut unique = std::collections::HashSet::new();
            for option in options {
                ensure!(
                    poll.options.iter().any(|item| item.id == *option) && unique.insert(option),
                    "Lựa chọn không tồn tại hoặc bị trùng"
                );
            }
            call(
                s,
                &s.domains.group_poll,
                "/api/poll/vote",
                json!({"poll_id":id,"option_ids":options,"imei":s.imei}),
                true,
                true,
            )
            .await?;
            reply.detail = Some(detail(s, id).await?);
        }
        Action::AddOption { id, text } => {
            let poll = detail(s, id).await?;
            ensure!(
                !poll.closed && poll.add_options && !text.trim().is_empty() && text.len() <= 1000,
                "Không thể thêm lựa chọn"
            );
            let voted = poll
                .options
                .iter()
                .filter(|item| item.selected)
                .map(|item| item.id)
                .collect::<Vec<_>>();
            call(
                s,
                &s.domains.group_poll,
                "/api/poll/option/add",
                json!({"poll_id":id,"new_options":[text.trim()],"voted_option_ids":voted}),
                true,
                true,
            )
            .await?;
            reply.detail = Some(detail(s, id).await?);
        }
        Action::EndPoll { id } => {
            let poll = detail(s, id).await?;
            let state = group_state::snapshot(s, group, true).await?;
            ensure!(
                !state.removed
                    && (poll.creator == s.account_key() || state.manager(s.account_key())),
                "Không có quyền kết thúc bình chọn"
            );
            call(
                s,
                &s.domains.group_poll,
                "/api/poll/end",
                json!({"poll_id":id,"imei":s.imei}),
                false,
                true,
            )
            .await?;
            reply.detail = Some(detail(s, id).await?);
        }
    }
    Ok(reply)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn anonymous_poll_does_not_invent_voter_identities() {
        let poll=parse_poll(json!({"poll_id":12,"question":"Q","creator":"1","closed":0,"is_anonymous":true,"allow_multi_choices":false,"options":[{"option_id":7,"content":"A","votes":9}]}),"1").unwrap();
        assert!(poll.anonymous);
        assert_eq!(poll.options[0].votes, Some(9));
        assert!(!poll.options[0].selected);
    }
}
