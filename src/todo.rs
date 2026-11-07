//! Group To-Do and Task Assignment (Zalo Parity A18).
//!
//! Provides data structures and API helpers for creating, updating, and querying
//! tasks/todos in groups (`/api/group/todo/create`, `/api/group/todo/update`, `/api/group/todo/list`).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::session::Session;

/// Status of a group to-do item.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TodoStatus {
    Pending = 0,
    Done = 1,
}

/// A to-do item in a group.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TodoItem {
    pub id: String,
    pub grid: String,
    pub content: String,
    pub creator_id: String,
    pub assignee_id: String,
    pub deadline_ts: i64,
    pub status: TodoStatus,
    pub created_ts: i64,
}

impl TodoItem {
    pub fn is_done(&self) -> bool {
        self.status == TodoStatus::Done
    }

    /// Check whether the deadline has passed.
    pub fn is_overdue(&self, now_ts: i64) -> bool {
        !self.is_done() && self.deadline_ts > 0 && self.deadline_ts < now_ts
    }
}

/// Parse a list of to-do items from Zalo API response.
pub fn parse_todo_list(val: &Value) -> Vec<TodoItem> {
    let items = val
        .get("data")
        .and_then(|d| d.get("items").or_else(|| d.get("todos")))
        .or_else(|| val.get("items"))
        .and_then(|v| v.as_array());

    let Some(arr) = items else {
        return Vec::new();
    };

    arr.iter()
        .filter_map(|it| {
            let id = it.get("id").or_else(|| it.get("todo_id"))?.as_str()?.to_string();
            let grid = it.get("grid").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let content = it.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let creator_id = it.get("creator_id").or_else(|| it.get("owner_id")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let assignee_id = it.get("assignee_id").or_else(|| it.get("assignee")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let deadline_ts = it.get("deadline").or_else(|| it.get("deadline_ts")).and_then(crate::api_response::integer).unwrap_or(0);
            let status_num = it.get("status").and_then(crate::api_response::integer).unwrap_or(0);
            let status = if status_num == 1 { TodoStatus::Done } else { TodoStatus::Pending };
            let created_ts = it.get("created_ts").or_else(|| it.get("create_time")).and_then(crate::api_response::integer).unwrap_or(0);

            Some(TodoItem {
                id,
                grid,
                content,
                creator_id,
                assignee_id,
                deadline_ts,
                status,
                created_ts,
            })
        })
        .collect()
}

/// Build create to-do request payload (`/api/group/todo/create`).
pub fn build_create_todo_params(
    grid: &str,
    content: &str,
    assignee_id: &str,
    deadline_ts: i64,
) -> Value {
    json!({
        "grid": grid,
        "content": content,
        "assignee": assignee_id,
        "deadline": deadline_ts,
    })
}

/// Build update to-do request payload (`/api/group/todo/update`).
pub fn build_update_todo_params(
    todo_id: &str,
    grid: &str,
    status: TodoStatus,
    assignee_id: Option<&str>,
    deadline_ts: Option<i64>,
) -> Value {
    let mut payload = json!({
        "id": todo_id,
        "grid": grid,
        "status": status as i32,
    });
    if let Some(assignee) = assignee_id {
        payload["assignee"] = json!(assignee);
    }
    if let Some(deadline) = deadline_ts {
        payload["deadline"] = json!(deadline);
    }
    payload
}

/// Call `/api/group/todo/create`.
pub async fn create_group_todo(
    s: &Session,
    grid: &str,
    content: &str,
    assignee_id: &str,
    deadline_ts: i64,
) -> Result<Value> {
    let url = s.resolve_service_url("group", "/api/group/todo/create");
    let payload = build_create_todo_params(grid, content, assignee_id, deadline_ts);
    let params = s.encoded_params(&payload)
        .context("Mã hoá tham số tạo to-do thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    crate::chat::confirmed_mutation(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Call `/api/group/todo/update`.
pub async fn update_group_todo(
    s: &Session,
    todo_id: &str,
    grid: &str,
    status: TodoStatus,
    assignee_id: Option<&str>,
    deadline_ts: Option<i64>,
) -> Result<Value> {
    let url = s.resolve_service_url("group", "/api/group/todo/update");
    let payload = build_update_todo_params(todo_id, grid, status, assignee_id, deadline_ts);
    let params = s.encoded_params(&payload)
        .context("Mã hoá tham số cập nhật to-do thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    crate::chat::confirmed_mutation(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Call `/api/group/todo/list`.
pub async fn list_group_todos(
    s: &Session,
    grid: &str,
) -> Result<Vec<TodoItem>> {
    let url = s.resolve_service_url("group", "/api/group/todo/list");
    let payload = json!({ "grid": grid });
    let params = s.encoded_params(&payload)
        .context("Mã hoá tham số lấy danh sách to-do thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let decrypted = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    Ok(parse_todo_list(&decrypted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_todo_list() {
        let val = json!({
            "error_code": 0,
            "data": {
                "items": [
                    {
                        "id": "todo_123",
                        "grid": "g999",
                        "content": "Hoàn thiện spec R12 và R13",
                        "creator_id": "u1",
                        "assignee_id": "u2",
                        "deadline": 1793990000,
                        "status": 0,
                        "created_ts": 1793980000
                    },
                    {
                        "id": "todo_124",
                        "grid": "g999",
                        "content": "Chạy smoke test Windows ARM64",
                        "creator_id": "u1",
                        "assignee_id": "u1",
                        "deadline": 1793985000,
                        "status": 1,
                        "created_ts": 1793970000
                    }
                ]
            }
        });

        let todos = parse_todo_list(&val);
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].id, "todo_123");
        assert_eq!(todos[0].content, "Hoàn thiện spec R12 và R13");
        assert_eq!(todos[0].status, TodoStatus::Pending);
        assert!(!todos[0].is_done());

        assert_eq!(todos[1].id, "todo_124");
        assert_eq!(todos[1].status, TodoStatus::Done);
        assert!(todos[1].is_done());
    }

    #[test]
    fn test_todo_overdue() {
        let item = TodoItem {
            id: "t1".into(),
            grid: "g1".into(),
            content: "Nhiệm vụ quá hạn".into(),
            creator_id: "u1".into(),
            assignee_id: "u2".into(),
            deadline_ts: 1000,
            status: TodoStatus::Pending,
            created_ts: 500,
        };
        assert!(item.is_overdue(1500));
        assert!(!item.is_overdue(800));

        let done_item = TodoItem {
            status: TodoStatus::Done,
            ..item
        };
        assert!(!done_item.is_overdue(1500));
    }
}
