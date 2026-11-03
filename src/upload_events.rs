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
    waiting: HashMap<String, tokio::sync::oneshot::Sender<String>>,
    done: HashMap<String, (String, Instant)>,
}

static REGISTRY: Mutex<Option<Registry>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Registry) -> R) -> R {
    let mut guard = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Registry::default))
}

fn id_text(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::to_owned)
        .or_else(|| v.as_u64().map(|n| n.to_string()))
        .or_else(|| v.as_i64().map(|n| n.to_string()))
        .filter(|s| !s.is_empty() && s.len() <= 64)
}

/// Extract `(fileId, url)` pairs from a decrypted control payload.
pub fn parse(payload: &Value) -> Vec<(String, String)> {
    let controls = payload
        .pointer("/data/controls")
        .or_else(|| payload.get("controls"))
        .and_then(Value::as_array);
    let mut out = Vec::new();
    for control in controls.into_iter().flatten() {
        let content = control.get("content").unwrap_or(control);
        if !matches!(
            content.get("act_type").and_then(Value::as_str),
            Some("file_done" | "video_done")
        ) {
            continue;
        }
        let data = match content.get("data") {
            Some(Value::String(raw)) => serde_json::from_str(raw).unwrap_or(Value::Null),
            Some(value) => value.clone(),
            None => Value::Null,
        };
        let file_id = content
            .get("fileId")
            .or_else(|| data.get("fileId"))
            .and_then(id_text);
        let url = data
            .get("url")
            .and_then(Value::as_str)
            .filter(|u| u.starts_with("https://") && u.len() <= 4096);
        if let (Some(file_id), Some(url)) = (file_id, url) {
            out.push((file_id, url.to_owned()));
        }
    }
    out
}

/// Deliver notifications from any control transport; never blocks.
pub fn deliver(payload: &Value) {
    let found = parse(payload);
    if found.is_empty() {
        return;
    }
    with(|r| {
        let now = Instant::now();
        r.done.retain(|_, (_, at)| now.duration_since(*at) < KEEP);
        for (file_id, url) in found {
            match r.waiting.remove(&file_id) {
                Some(waiter) => {
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

/// Wait for the processed CDN URL of an uploaded file, up to `timeout`.
pub async fn wait(file_id: &str, timeout: Duration) -> Option<String> {
    let receiver = with(|r| {
        if let Some((url, _)) = r.done.remove(file_id) {
            return Err(url);
        }
        if r.waiting.len() >= LIMIT {
            return Ok(None);
        }
        let (tx, rx) = tokio::sync::oneshot::channel();
        r.waiting.insert(file_id.to_owned(), tx);
        Ok(Some(rx))
    });
    let receiver = match receiver {
        Err(url) => return Some(url),
        Ok(None) => return None,
        Ok(Some(rx)) => rx,
    };
    let result = tokio::time::timeout(timeout, receiver).await;
    with(|r| {
        r.waiting.remove(file_id);
    });
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
        assert_eq!(parse(&payload), vec![("77".to_string(), "https://v.dlmd.me/x.mp4".to_string())]);
    }

    #[tokio::test]
    async fn waiter_receives_url_delivered_later() {
        let waiting = tokio::spawn(wait("file-later", Duration::from_secs(2)));
        tokio::time::sleep(Duration::from_millis(50)).await;
        deliver(&done("file-later", "https://f.dlfl.vn/a"));
        assert_eq!(waiting.await.unwrap().as_deref(), Some("https://f.dlfl.vn/a"));
    }

    #[tokio::test]
    async fn early_notification_is_kept_for_a_late_waiter() {
        deliver(&done("file-early", "https://f.dlfl.vn/b"));
        assert_eq!(
            wait("file-early", Duration::from_millis(10)).await.as_deref(),
            Some("https://f.dlfl.vn/b")
        );
    }

    #[tokio::test]
    async fn wait_times_out_without_notification() {
        assert!(wait("file-never", Duration::from_millis(20)).await.is_none());
    }
}
