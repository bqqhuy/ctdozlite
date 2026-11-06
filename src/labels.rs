//! Versioned conversation labels from the official legacy adapter; no blind whole-list retries.
use crate::session::Session;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
/// Display fields plus the untouched server row to preserve attributes outside the supported editor.
#[derive(Clone, Debug)]
pub struct Label {
    pub id: i64,
    pub text: String,
    pub raw: Value,
    pub conversations: Vec<String>,
    members: std::collections::HashSet<String>,
}
impl Label {
    /// Constant-time membership lookup for sidebar rendering.
    pub fn contains(&self, identity: &str) -> bool {
        self.members.contains(identity)
    }
}
/// A complete server label snapshot and its optimistic concurrency version.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub version: i64,
    pub labels: Vec<Label>,
}
impl Snapshot {
    /// Read documented array/object/string labelData forms without treating missing data as empty.
    /// # Errors
    /// Rejects invalid IDs, oversized lists or malformed membership fields.
    pub fn parse(data: Value) -> Result<Self> {
        let version = data
            .get("version")
            .and_then(crate::api_response::integer)
            .filter(|value| *value >= 0)
            .context("Thiếu phiên bản phân loại")?;
        let mut raw = data
            .get("labelData")
            .cloned()
            .context("Thiếu danh sách phân loại")?;
        if let Some(text) = raw.as_str() {
            ensure!(text.len() <= 4 * 1024 * 1024, "Danh sách phân loại quá lớn");
            raw = serde_json::from_str(text)?;
        }
        let rows = match raw {
            Value::Array(values) => values,
            Value::Object(values) => values.into_values().collect(),
            _ => anyhow::bail!("Danh sách phân loại sai định dạng"),
        };
        ensure!(rows.len() <= 1000, "Quá nhiều phân loại");
        let mut labels = Vec::new();
        let mut ids = std::collections::HashSet::new();
        let mut total = 0;
        for row in rows {
            let id = row
                .get("id")
                .and_then(crate::api_response::integer)
                .filter(|id| *id > 0)
                .context("ID phân loại không hợp lệ")?;
            ensure!(ids.insert(id), "ID phân loại bị trùng");
            let text = row
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Phân loại #{id}"));
            ensure!(text.len() <= 1024, "Tên phân loại quá dài");
            let members = row
                .get("conversations")
                .and_then(Value::as_array)
                .context("Thiếu hội thoại trong phân loại")?;
            total += members.len();
            ensure!(total <= 100000, "Danh sách hội thoại phân loại quá lớn");
            let conversations: Vec<String> = members
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .filter(|id| !id.is_empty() && id.len() <= 64)
                        .map(str::to_owned)
                        .context("ID hội thoại phân loại không hợp lệ")
                })
                .collect::<Result<Vec<_>>>()?;
            labels.push(Label {
                id,
                text,
                raw: row,
                members: conversations.iter().cloned().collect(),
                conversations,
            });
        }
        labels.sort_by_key(|label| {
            (
                label
                    .raw
                    .get("offset")
                    .and_then(crate::api_response::integer)
                    .unwrap_or(0),
                label.id,
            )
        });
        Ok(Self { version, labels })
    }
}
/// Explicit edits apply to a freshly loaded snapshot, retaining unrelated labels and fields.
#[derive(Clone, Debug)]
pub enum Action {
    Read,
    Create {
        name: String,
    },
    Rename {
        id: i64,
        name: String,
    },
    Delete {
        id: i64,
    },
    Assign {
        target: String,
        group: bool,
        label: Option<i64>,
    },
}
fn name(value: &str) -> Result<&str> {
    let value = value.trim();
    ensure!(
        !value.is_empty() && value.chars().count() <= 50 && !value.chars().any(char::is_control),
        "Tên phân loại phải có từ 1 đến 50 ký tự"
    );
    Ok(value)
}
/// Build the PC `labelData` JSON string and version after changing only the chosen intent.
/// # Errors
/// Rejects stale/unknown labels, duplicates and invalid recipient identities before mutation.
pub fn update_params(session: &Session, snapshot: &Snapshot, action: &Action) -> Result<Value> {
    let mut rows: Vec<Value> = snapshot
        .labels
        .iter()
        .map(|label| label.raw.clone())
        .collect();
    match action {
        Action::Read => anyhow::bail!("Đọc phân loại không cần tham số cập nhật"),
        Action::Create { name: input } => {
            let text = name(input)?;
            ensure!(
                !snapshot.labels.iter().any(|label| label.text == text),
                "Tên phân loại đã tồn tại"
            );
            ensure!(
                rows.len() < 100,
                "Đã đạt giới hạn 100 phân loại của bộ chỉnh sửa"
            );
            let id = snapshot
                .labels
                .iter()
                .map(|label| label.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .context("Hết ID phân loại")?;
            let color = snapshot
                .labels
                .iter()
                .find_map(|label| label.raw.get("color").cloned())
                .unwrap_or(json!("#0068ff"));
            rows.push(
                json!({"id":id,"text":text,"color":color,"offset":rows.len(),"conversations":[]}),
            );
        }
        Action::Rename { id, name: input } => {
            let text = name(input)?;
            ensure!(
                !snapshot
                    .labels
                    .iter()
                    .any(|label| label.id != *id && label.text == text),
                "Tên phân loại đã tồn tại"
            );
            let row = rows
                .iter_mut()
                .find(|row| row.get("id").and_then(crate::api_response::integer) == Some(*id))
                .context("Phân loại đã thay đổi; tải lại danh sách")?;
            row["text"] = json!(text);
            if let Some(object) = row.as_object_mut() {
                object.remove("textKey");
            }
        }
        Action::Delete { id } => {
            ensure!(
                snapshot.labels.iter().any(|label| label.id == *id),
                "Phân loại không còn tồn tại"
            );
            rows.retain(|row| row.get("id").and_then(crate::api_response::integer) != Some(*id));
        }
        Action::Assign {
            target,
            group,
            label,
        } => {
            let identity = crate::server_pins::identity(target, *group)?;
            if let Some(id) = label {
                ensure!(
                    snapshot.labels.iter().any(|label| label.id == *id),
                    "Phân loại không còn tồn tại"
                );
            }
            for row in &mut rows {
                let id = row.get("id").and_then(crate::api_response::integer);
                let members = row
                    .get_mut("conversations")
                    .and_then(Value::as_array_mut)
                    .context("Danh sách hội thoại không hợp lệ")?;
                members.retain(|value| value.as_str() != Some(identity.as_str()));
                if id == *label {
                    members.push(json!(identity));
                }
            }
        }
    }
    Ok(
        json!({"labelData":serde_json::to_string(&rows)?,"version":snapshot.version,"imei":session.imei}),
    )
}
async fn call(s: &Session, path: &str, body: Value, get: bool) -> Result<Value> {
    ensure!(
        s.is_active() && !s.account_key().is_empty(),
        "Phiên tài khoản đã thay đổi"
    );
    let host = crate::endpoints::service("label").context("Thiếu dịch vụ phân loại")?;
    let params = s.encoded_params(&body)?;
    let url = format!("{host}{path}?{}", s.common_params());
    let raw = if get {
        s.get(&format!("{url}&params={params}")).await?
    } else {
        s.post_form(&url, format!("params={params}")).await?
    };
    let raw = s.maybe_decrypt(raw, s.session_key_bytes()).await;
    ensure!(
        raw.get("error_code").is_some(),
        "Thiếu xác nhận phân loại từ máy chủ"
    );
    crate::api_response::checked_data(raw)
}
async fn read(s: &Session) -> Result<Snapshot> {
    Snapshot::parse(call(s, "/api/convlabel/get", json!({"imei":s.imei}), true).await?)
}
/// Confirm the requested operation against server readback, without claiming an outer ACK applied it.
pub fn applied(snapshot: &Snapshot, action: &Action) -> bool {
    match action {
        Action::Read => true,
        Action::Create { name } => snapshot
            .labels
            .iter()
            .any(|label| label.text == name.trim()),
        Action::Rename { id, name } => snapshot
            .labels
            .iter()
            .any(|label| label.id == *id && label.text == name.trim()),
        Action::Delete { id } => !snapshot.labels.iter().any(|label| label.id == *id),
        Action::Assign {
            target,
            group,
            label,
        } => crate::server_pins::identity(target, *group)
            .ok()
            .is_some_and(|identity| {
                let containing: Vec<_> = snapshot
                    .labels
                    .iter()
                    .filter(|label| label.contains(&identity))
                    .map(|label| label.id)
                    .collect();
                match label {
                    Some(id) => containing == [*id],
                    None => containing.is_empty(),
                }
            }),
    }
}

/// Execute once using a fresh version and reconcile from the server. Conflicts are surfaced, not replayed.
/// # Errors
/// Returns validation, server conflict/refusal, missing capability and uncertain transport outcomes.
pub async fn execute(s: &Session, action: &Action) -> Result<Snapshot> {
    let before = read(s).await?;
    if matches!(action, Action::Read) {
        return Ok(before);
    }
    call(
        s,
        "/api/convlabel/update",
        update_params(s, &before, action)?,
        false,
    )
    .await?;
    let after = read(s).await?;
    ensure!(
        applied(&after, action),
        "Chưa xác nhận máy chủ đã áp dụng phân loại; tải lại trước khi thử tiếp"
    );
    Ok(after)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot() -> Snapshot {
        Snapshot::parse(json!({"version":8,"labelData":"[{\"id\":1,\"text\":\"Một\",\"color\":\"#0068ff\",\"future\":42,\"conversations\":[\"123\",\"g123\"]},{\"id\":2,\"text\":\"Hai\",\"conversations\":[]}]"})).unwrap()
    }
    #[test]
    fn assigning_a_group_preserves_the_direct_peer_and_unrelated_server_fields() {
        let s = Session::new(false, false, false).unwrap();
        let body = update_params(
            &s,
            &snapshot(),
            &Action::Assign {
                target: "123".into(),
                group: true,
                label: Some(2),
            },
        )
        .unwrap();
        let rows: Vec<Value> = serde_json::from_str(body["labelData"].as_str().unwrap()).unwrap();
        assert_eq!(body["version"], 8);
        assert_eq!(rows[0]["conversations"], json!(["123"]));
        assert_eq!(rows[1]["conversations"], json!(["g123"]));
        assert_eq!(rows[0]["future"], 42);
        assert!(update_params(
            &s,
            &snapshot(),
            &Action::Rename {
                id: 1,
                name: "Hai".into()
            }
        )
        .is_err());
        assert!(Snapshot::parse(json!({"version":8})).is_err());
    }
}
