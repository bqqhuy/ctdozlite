//! Group Poll and Voting (Zalo Parity A19).
//!
//! Provides data structures and API helpers for creating, voting, and inspecting
//! group polls (`/api/group/poll/create`, `/api/group/poll/vote`, `/api/group/poll/detail`).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::session::Session;

/// An option in a poll.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PollOption {
    pub id: i64,
    pub text: String,
    pub vote_count: u32,
    pub is_voted_by_me: bool,
}

/// A poll in a group chat.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PollItem {
    pub id: i64,
    pub grid: String,
    pub question: String,
    pub creator_id: String,
    pub options: Vec<PollOption>,
    pub multi_choice: bool,
    pub is_closed: bool,
    pub total_votes: u32,
}

impl PollItem {
    /// Calculate percentage for an option based on total votes.
    pub fn option_percentage(&self, option_id: i64) -> f32 {
        if self.total_votes == 0 {
            return 0.0;
        }
        let count = self.options.iter().find(|o| o.id == option_id).map(|o| o.vote_count).unwrap_or(0);
        (count as f32 / self.total_votes as f32) * 100.0
    }
}

/// Parse a poll item from Zalo API response or message wire payload.
pub fn parse_poll_item(val: &Value) -> Option<PollItem> {
    let root = val.get("data").unwrap_or(val);
    let id = root.get("id").or_else(|| root.get("poll_id")).and_then(crate::api_response::integer)?;
    let grid = root.get("grid").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let question = root.get("question").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let creator_id = root.get("creator_id").or_else(|| root.get("owner_id")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let multi_choice = root.get("multi_choice").or_else(|| root.get("is_multi_choice")).and_then(|v| v.as_bool()).unwrap_or(false);
    let is_closed = root.get("is_closed").or_else(|| root.get("closed")).and_then(|v| v.as_bool()).unwrap_or(false);

    let mut total_votes = 0u32;
    let mut options = Vec::new();
    if let Some(arr) = root.get("options").and_then(|v| v.as_array()) {
        for (idx, opt) in arr.iter().enumerate() {
            let opt_id = opt.get("id").or_else(|| opt.get("option_id")).and_then(crate::api_response::integer).unwrap_or(idx as i64 + 1);
            let text = opt.get("content").or_else(|| opt.get("text")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let vote_count = opt.get("count").or_else(|| opt.get("vote_count")).and_then(crate::api_response::integer).unwrap_or(0) as u32;
            let is_voted_by_me = opt.get("is_voted").or_else(|| opt.get("voted")).and_then(|v| v.as_bool()).unwrap_or(false);
            total_votes += vote_count;
            options.push(PollOption {
                id: opt_id,
                text,
                vote_count,
                is_voted_by_me,
            });
        }
    }

    Some(PollItem {
        id,
        grid,
        question,
        creator_id,
        options,
        multi_choice,
        is_closed,
        total_votes,
    })
}

/// Build create poll payload (`/api/group/poll/create`).
pub fn build_create_poll_params(
    grid: &str,
    question: &str,
    options: &[String],
    multi_choice: bool,
    allow_add_opt: bool,
    hide_voters: bool,
) -> Value {
    let opts: Vec<Value> = options.iter().map(|o| json!({ "content": o })).collect();
    json!({
        "grid": grid,
        "question": question,
        "options": opts,
        "multi_choice": multi_choice,
        "allow_add_opt": allow_add_opt,
        "hide_voters": hide_voters,
    })
}

/// Call `/api/group/poll/create`.
pub async fn create_group_poll(
    s: &Session,
    grid: &str,
    question: &str,
    options: &[String],
    multi_choice: bool,
    allow_add_opt: bool,
    hide_voters: bool,
) -> Result<Value> {
    let url = format!("{}/api/group/poll/create", s.domains.group);
    let payload = build_create_poll_params(grid, question, options, multi_choice, allow_add_opt, hide_voters);
    let params = s.encoded_params(&payload)
        .context("Mã hoá tham số tạo bình chọn thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    crate::chat::confirmed_mutation(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Call `/api/group/poll/vote`.
pub async fn vote_group_poll(
    s: &Session,
    poll_id: i64,
    option_ids: &[i64],
) -> Result<Value> {
    let url = format!("{}/api/group/poll/vote", s.domains.group);
    let payload = json!({
        "poll_id": poll_id,
        "option_ids": option_ids,
    });
    let params = s.encoded_params(&payload)
        .context("Mã hoá tham số bỏ phiếu thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    crate::chat::confirmed_mutation(s.maybe_decrypt(resp, s.session_key_bytes()).await)
}

/// Call `/api/group/poll/detail`.
pub async fn get_group_poll_detail(
    s: &Session,
    poll_id: i64,
) -> Result<Option<PollItem>> {
    let url = format!("{}/api/group/poll/detail", s.domains.group);
    let payload = json!({ "poll_id": poll_id });
    let params = s.encoded_params(&payload)
        .context("Mã hoá tham số xem bình chọn thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let decrypted = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    Ok(parse_poll_item(&decrypted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_poll_item() {
        let val = json!({
            "error_code": 0,
            "data": {
                "id": 998877,
                "grid": "g123",
                "question": "Release v0.009-arm khi nào?",
                "creator_id": "u1",
                "multi_choice": false,
                "is_closed": false,
                "options": [
                    { "id": 1, "content": "Tối nay", "count": 8, "is_voted": true },
                    { "id": 2, "content": "Ngày mai", "count": 2, "is_voted": false }
                ]
            }
        });

        let poll = parse_poll_item(&val).expect("Failed to parse poll");
        assert_eq!(poll.id, 998877);
        assert_eq!(poll.question, "Release v0.009-arm khi nào?");
        assert_eq!(poll.options.len(), 2);
        assert_eq!(poll.total_votes, 10);
        assert_eq!(poll.option_percentage(1), 80.0);
        assert_eq!(poll.option_percentage(2), 20.0);
    }
}
