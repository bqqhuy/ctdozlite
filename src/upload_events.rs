//! Correlates `asyncfile` uploads with the server's `file_done` / `video_done` controls.
//!
//! Zalo PC (`_processFileAsyncDoneEvent`) waits for `{fileId, data:{url}}` on the control
//! channel before calling `asyncfile/msg`. Notifications may arrive before the uploader starts
//! waiting, so unclaimed ones are kept briefly. No second socket is opened.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long an unclaimed notification is kept for a late waiter.
const KEEP: Duration = Duration::from_secs(120);
/// Bound on remembered notifications and waiters.
const LIMIT: usize = 256;

#[derive(Default)]
struct Registry {
    waiting: HashMap<String, (u64, tokio::sync::oneshot::Sender<String>)>,
    done: HashMap<String, (String, Instant)>,
}

static REGISTRY: Mutex<Option<Registry>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Registry) -> R) -> R {
    let mut guard = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Registry::default))
}

/// Server processing sentinels are not upload identities.
pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.trim() == id && id != "-1" && id != "0"
}

fn id_text(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::to_owned)
        .or_else(|| v.as_u64().map(|n| n.to_string()))
        .or_else(|| v.as_i64().map(|n| n.to_string()))
        .filter(|s| valid_id(s))
}

/// Extract `(fileId, url)` pairs from a decrypted control payload.
pub fn parse(payload: &Value) -> Vec<(String, String)> {
    // Bound traversal and embedded JSON independently of the socket frame limit.
    fn visit(value: &Value, depth: usize, budget: &mut usize, out: &mut Vec<(String, String)>) {
        if depth > 8 || *budget == 0 || out.len() >= LIMIT {
            return;
        }
        *budget -= 1;
        if let Some(items) = value.as_array() {
            for item in items {
                if *budget == 0 || out.len() >= LIMIT {
                    break;
                }
                visit(item, depth + 1, budget, out);
            }
            return;
        }
        let content = value.get("content").unwrap_or(value);
        if matches!(
            content.get("act_type").and_then(Value::as_str),
            Some("file_done" | "video_done")
        ) {
            let data = match content.get("data") {
                Some(Value::String(raw)) => {
                    if raw.len() <= 16 * 1024 {
                        serde_json::from_str(raw).unwrap_or(Value::Null)
                    } else {
                        Value::Null
                    }
                }
                Some(value) => value.clone(),
                None => Value::Null,
            };
            let id = [content.get("fileId"), data.get("fileId")]
                .into_iter()
                .flatten()
                .find_map(id_text);
            let url = ["url", "fileUrl"]
                .into_iter()
                .filter_map(|key| data.get(key).and_then(Value::as_str))
                .find(|raw| {
                    raw.len() <= 4096
                        && reqwest::Url::parse(raw).is_ok_and(|u| {
                            u.scheme() == "https"
                                && u.host_str().is_some()
                                && u.username().is_empty()
                                && u.password().is_none()
                        })
                });
            if let (Some(id), Some(url)) = (id, url) {
                out.push((id, url.to_owned()));
            }
            return;
        }
        for key in ["data", "controls"] {
            if let Some(child) = value.get(key) {
                visit(child, depth + 1, budget, out);
            }
        }
    }
    let mut out = Vec::new();
    visit(payload, 0, &mut 1024, &mut out);
    out
}

/// Deliver notifications from any control transport; never blocks.
pub fn deliver(account: &str, payload: &Value) {
    let found = parse(payload);
    if found.is_empty() {
        return;
    }
    with(|r| {
        let now = Instant::now();
        r.done.retain(|_, (_, at)| now.duration_since(*at) < KEEP);
        for (file_id, url) in found {
            let file_id = scoped_key(account, &file_id);
            match r.waiting.remove(&file_id) {
                Some((_, waiter)) => {
                    let _ = waiter.send(url);
                }
                None if r.done.len() < LIMIT => {
                    r.done.insert(file_id, (url, now));
                }
                None => {}
            }
        }
    });
}

fn scoped_key(account: &str, file_id: &str) -> String {
    format!("{}:{account}{file_id}", account.len())
}

struct Waiter(String, u64);
impl Drop for Waiter {
    fn drop(&mut self) {
        // Also release cancelled futures; no later waiter can be removed by this one.
        with(|r| {
            if r.waiting.get(&self.0).is_some_and(|(id, _)| *id == self.1) {
                r.waiting.remove(&self.0);
            }
        });
    }
}

/// Wait for one account's processed upload URL; cancellation releases its registration.
pub async fn wait(account: &str, file_id: &str, timeout: Duration) -> Option<String> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let generation = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let file_id = scoped_key(account, file_id);
    let receiver = with(|r| {
        if let Some((url, at)) = r.done.remove(&file_id) {
            if at.elapsed() < KEEP {
                return Err(url);
            }
        }
        if r.waiting.len() >= LIMIT || r.waiting.contains_key(&file_id) {
            return Ok(None);
        }
        let (tx, rx) = tokio::sync::oneshot::channel();
        r.waiting.insert(file_id.clone(), (generation, tx));
        Ok(Some(rx))
    });
    let receiver = match receiver {
        Err(url) => return Some(url),
        Ok(None) => return None,
        Ok(Some(rx)) => rx,
    };
    let _waiter = Waiter(file_id, generation);
    let result = tokio::time::timeout(timeout, receiver).await;
    result.ok().and_then(Result::ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn done(id: &str, url: &str) -> Value {
        json!({"data":{"controls":[{"content":{"act_type":"file_done","fileId":id,"data":{"url":url}}}]}})
    }

    #[test]
    fn parse_accepts_file_and_video_done_and_ignores_other_controls() {
        let payload = json!({"controls":[
            {"content":{"act_type":"video_done","fileId":77,"data":"{\"url\":\"https://v.dlmd.me/x.mp4\"}"}},
            {"content":{"act_type":"voip","fileId":1,"data":{"url":"https://x"}}},
            {"content":{"act_type":"file_done","fileId":"9","data":{"url":"http://insecure"}}}
        ]});
        assert_eq!(
            parse(&payload),
            vec![("77".to_string(), "https://v.dlmd.me/x.mp4".to_string())]
        );
    }

    #[test]
    fn accepts_direct_batched_and_file_url_alias_with_bounds() {
        let control =
            json!({"act_type":"file_done","fileId":42,"data":{"fileUrl":"https://f.dlfl.vn/file"}});
        assert_eq!(parse(&control).len(), 1);
        let mut mixed = control.clone();
        mixed["data"]["url"] = Value::Null;
        assert_eq!(parse(&mixed).len(), 1);
        mixed["data"]["url"] = json!("");
        assert_eq!(parse(&mixed).len(), 1);
        assert_eq!(
            parse(&json!([{"data":{"controls":[control.clone()]}}])).len(),
            1
        );
        for id in ["-1", "0", ""] {
            let mut invalid = control.clone();
            invalid["fileId"] = json!(id);
            assert!(parse(&invalid).is_empty());
        }
        let mut invalid = control.clone();
        invalid["data"]["fileUrl"] = json!("https://user:secret@example.com/file");
        assert!(parse(&invalid).is_empty());
        assert_eq!(parse(&Value::Array(vec![control; 2048])).len(), LIMIT);
    }

    #[tokio::test]
    async fn waiter_receives_url_delivered_later() {
        let waiting = tokio::spawn(wait("test-account", "file-later", Duration::from_secs(2)));
        tokio::time::sleep(Duration::from_millis(50)).await;
        deliver("test-account", &done("file-later", "https://f.dlfl.vn/a"));
        assert_eq!(
            waiting.await.unwrap().as_deref(),
            Some("https://f.dlfl.vn/a")
        );
    }

    #[tokio::test]
    async fn early_notification_is_kept_for_a_late_waiter() {
        deliver("test-account", &done("file-early", "https://f.dlfl.vn/b"));
        assert_eq!(
            wait("test-account", "file-early", Duration::from_millis(10))
                .await
                .as_deref(),
            Some("https://f.dlfl.vn/b")
        );
    }

    #[tokio::test]
    async fn wait_times_out_without_notification() {
        assert!(
            wait("test-account", "file-never", Duration::from_millis(20))
                .await
                .is_none()
        );
    }
    #[tokio::test]
    async fn notifications_are_isolated_by_account_and_expired_entries_are_ignored() {
        deliver("account-a", &done("scoped-file", "https://f.dlfl.vn/a"));
        assert!(wait("account-b", "scoped-file", Duration::from_millis(1))
            .await
            .is_none());
        assert_eq!(
            wait("account-a", "scoped-file", Duration::from_millis(1))
                .await
                .as_deref(),
            Some("https://f.dlfl.vn/a")
        );
        with(|r| {
            r.done.insert(
                scoped_key("expired", "file"),
                (
                    "https://f.dlfl.vn/old".into(),
                    Instant::now() - KEEP - Duration::from_secs(1),
                ),
            );
        });
        assert!(wait("expired", "file", Duration::from_millis(1))
            .await
            .is_none());
    }

    #[tokio::test]
    async fn duplicate_waiter_does_not_replace_first_and_cancellation_cleans_up() {
        let first = tokio::spawn(wait("waiters", "same-file", Duration::from_secs(10)));
        tokio::task::yield_now().await;
        assert!(wait("waiters", "same-file", Duration::from_millis(1))
            .await
            .is_none());
        first.abort();
        let _ = first.await;
        assert!(!with(|r| r
            .waiting
            .contains_key(&scoped_key("waiters", "same-file"))));
        let next = tokio::spawn(wait("waiters", "same-file", Duration::from_secs(1)));
        tokio::task::yield_now().await;
        deliver("waiters", &done("same-file", "https://f.dlfl.vn/new"));
        assert_eq!(
            next.await.unwrap().as_deref(),
            Some("https://f.dlfl.vn/new")
        );
    }
}
