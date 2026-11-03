//! Conservative admission and explicit-rejection adaptation for phone transfers.
use super::plan::Query;
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(super) const PAYLOAD_BUDGET: usize = 64 * 1024;
#[derive(Debug)]
pub(super) struct RateLimited(pub u64);
impl std::fmt::Display for RateLimited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Máy chủ giới hạn tần suất; đã dừng. Thử lại sau {} giây",
            self.0
        )
    }
}
impl std::error::Error for RateLimited {}
#[derive(Debug)]
pub(super) struct LimitRejection(pub Option<usize>);
impl std::fmt::Display for LimitRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Điện thoại từ chối: yêu cầu có quá nhiều hội thoại")
    }
}
impl std::error::Error for LimitRejection {}

/// Unknown errors never authorize a replay. No numeric business code is guessed.
pub(super) fn rejection(value: &Value, accepted: bool) -> anyhow::Error {
    let code = value
        .get("error_code")
        .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
        .unwrap_or(-1);
    let data = value.get("data").unwrap_or(value);
    let reason = data
        .get("reason")
        .or_else(|| value.get("error_message"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    if code == 429 || reason.contains("rate limit") || reason.contains("too many requests") {
        let delay = data.get("retryAfter").and_then(Value::as_u64);
        return RateLimited(delay.unwrap_or(300).clamp(5, 86400)).into();
    }
    if matches!(code, 600 | -102 | 114 | 18022 | 18060) {
        return anyhow::anyhow!(
            "Lỗi xác thực/giao thức đồng bộ (mã {code}); đã dừng, không tự gửi lại"
        );
    }
    if !accepted
        && matches!(
            reason.as_str(),
            "too_many_partitions"
                | "too many partitions"
                | "too_many_conversations"
                | "too many conversations"
                | "payload_too_large"
        )
    {
        let maximum = data
            .get("maxPartitions")
            .or_else(|| data.get("maxConversations"))
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n > 0);
        return LimitRejection(maximum).into();
    }
    anyhow::anyhow!(
        "Đồng bộ bị từ chối hoặc chưa rõ kết quả (mã {code}); đã dừng, không tự gửi lại"
    )
}
#[derive(Default, Serialize, Deserialize)]
pub(super) struct Limits {
    pub count: Option<usize>,
    pub observed_at: u64,
    #[serde(default)]
    pub cooldown_until: u64,
}
impl Limits {
    pub fn load(path: &std::path::Path) -> Self {
        crate::limits::read_file(path, 4096)
            .ok()
            .and_then(|b| serde_json::from_slice::<Self>(&b).ok())
            .filter(|p| p.count.is_none_or(|n| n > 0 && n <= 100_000))
            .unwrap_or_default()
    }
}
pub(super) fn split(
    jobs: Vec<Query>,
    count: Option<usize>,
) -> Result<std::collections::VecDeque<Vec<Query>>> {
    let mut batches = std::collections::VecDeque::new();
    let mut current = Vec::new();
    let mut size = 2048usize;
    for query in jobs {
        let bytes = serde_json::to_vec(&serde_json::json!({"partition":query.partition,"from":query.from,"to":query.to,"limit":2147483647}))?.len() + 1;
        ensure!(
            bytes + 2048 <= PAYLOAD_BUDGET,
            "Định danh hội thoại vượt giới hạn yêu cầu cục bộ"
        );
        if !current.is_empty()
            && (size + bytes > PAYLOAD_BUDGET || current.len() >= count.unwrap_or(usize::MAX))
        {
            batches.push_back(std::mem::take(&mut current));
            size = 2048;
        }
        size += bytes;
        current.push(query);
    }
    if !current.is_empty() {
        batches.push_back(current);
    }
    Ok(batches)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_explicit_unaccepted_size_rejection_can_split() {
        for value in [
            serde_json::json!({"error_code":429,"data":{"reason":"too many requests"}}),
            serde_json::json!({"error_code":114}),
            serde_json::json!({"error_code":1,"data":{"reason":"unknown"}}),
        ] {
            assert!(rejection(&value, false)
                .downcast_ref::<LimitRejection>()
                .is_none());
        }
        let value = serde_json::json!({"error_code":1,"data":{"reason":"too_many_partitions","maxPartitions":10}});
        assert_eq!(
            rejection(&value, false)
                .downcast_ref::<LimitRejection>()
                .unwrap()
                .0,
            Some(10)
        );
        assert!(rejection(&value, true)
            .downcast_ref::<LimitRejection>()
            .is_none());
    }
    #[test]
    fn all_partitions_fit_one_request_until_a_real_bound_applies() {
        let jobs: Vec<_> = (0..300)
            .map(|i| Query {
                partition: format!("group/{i}"),
                from: 0,
                to: 100,
            })
            .collect();
        assert_eq!(split(jobs.clone(), None).unwrap().len(), 1);
        assert_eq!(split(jobs, Some(30)).unwrap().len(), 10);
    }
}
