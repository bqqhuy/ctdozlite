//! Connection-owned pull scheduling for every server-advertised queue.
use super::continuation_cursor;
use crate::{queue_catchup::CatchupGuard, transfer_diagnostics as diag};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

type Key = (u16, u8);
const WAIT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u8 = 3;
/// Pull commands whose pages may continue a server queue (the pre-r2 fixed list).
const KNOWN_PULL_COMMANDS: [u16; 9] = [510, 511, 513, 514, 515, 517, 518, 603, 604];
/// Same bound as the auth-advertised queue list.
const MAX_QUEUES: usize = 256;

pub(crate) struct Pull {
    pub key: Key,
    pub last: Value,
    pub ids: Value,
}
struct Queue {
    guard: CatchupGuard,
    last: Value,
    ids: Value,
    request: String,
    in_flight: bool,
    deadline: Option<Instant>,
    retries: u8,
}
/// One instance per socket epoch; dropping the connection drops every scheduled retry.
#[derive(Default)]
pub(crate) struct PullQueues {
    queues: HashMap<Key, Queue>,
    epoch: u64,
}
/// Validate whether the server payload contains an error code, error status, or missing/null data (A17).
pub(crate) fn is_queue_error_payload(payload: &Value) -> bool {
    let check_code = |v: &Value| -> bool {
        match v {
            Value::Number(n) => n.as_i64().is_some_and(|c| c != 0),
            Value::String(s) => s != "0" && !s.is_empty(),
            Value::Bool(b) => *b,
            _ => false,
        }
    };

    if let Some(c) = payload.get("error_code") {
        if check_code(c) {
            return true;
        }
    }
    if let Some(c) = payload.pointer("/data/error_code") {
        if check_code(c) {
            return true;
        }
    }
    if let Some(err) = payload.get("error") {
        if check_code(err) {
            return true;
        }
    }
    if let Some(err) = payload.pointer("/data/error") {
        if check_code(err) {
            return true;
        }
    }

    // Missing or null data when no cursor fields exist at top level
    let data_is_present = payload.get("data").is_some_and(|d| !d.is_null());
    let has_top_cursor = payload.get("lastActionId").is_some() || payload.get("queueStatus").is_some();
    if !data_is_present && !has_top_cursor {
        return true;
    }

    false
}

impl PullQueues {
    /// Create an empty scheduler for one authenticated socket generation.
    pub fn new(epoch: u64) -> Self {
        Self {
            epoch,
            ..Default::default()
        }
    }
    /// Register an initial pull only after its write succeeds; `start` is the saved cursor
    /// the pull was sent with, if any.
    pub fn register(
        &mut self,
        key: Key,
        request: String,
        now: Instant,
        start: Option<(Value, Value)>,
    ) {
        let (last, ids) = start.unwrap_or_else(|| (json!(1), json!([])));
        let mut guard = CatchupGuard::default();
        if let Some(cursor) = continuation_cursor(Some(&last), &ids) {
            guard.seed(cursor);
        }
        self.queues.insert(
            key,
            Queue {
                guard,
                last,
                ids,
                request,
                in_flight: true,
                deadline: Some(now + WAIT),
                retries: 0,
            },
        );
        diag::queue(self.epoch, key, diag::QueueDecision::Initial, 0);
    }
    /// Process only replies for an advertised queue, retaining the exact server cursor.
    pub fn page(&mut self, key: Key, payload: &Value, now: Instant) -> Option<Pull> {
        if !self.queues.contains_key(&key) {
            self.discover(key, payload)?;
        }
        let queue = self.queues.get_mut(&key)?;
        let reply = payload
            .pointer("/data/reqId")
            .or_else(|| payload.get("reqId"))
            .and_then(Value::as_str);
        if queue.in_flight
            && reply.is_some_and(|id| !queue.request.is_empty() && id != queue.request)
        {
            diag::queue(self.epoch, key, diag::QueueDecision::Stale, queue.retries);
            return None;
        }
        queue.in_flight = false;
        if is_queue_error_payload(payload) {
            // A17: Server error must NOT be marked completed. Retain existing cursor, keep backoff, and schedule retry if under limit.
            if queue.retries < MAX_RETRIES {
                queue.deadline = Some(queue.deadline.map_or(now + WAIT, |at| at.min(now + WAIT)));
                diag::queue(self.epoch, key, diag::QueueDecision::Missing, queue.retries);
            } else {
                queue.deadline = None;
                diag::queue(self.epoch, key, diag::QueueDecision::Exhausted, queue.retries);
            }
            return None;
        }
        let more = payload.pointer("/data/more").and_then(Value::as_i64) == Some(1);
        let qkey = format!("{}_{}", key.0, key.1);
        let last = payload
            .pointer("/data/lastActionId")
            .or_else(|| payload.pointer(&format!("/data/queueStatus/{qkey}/lastId")))
            .cloned();
        let ids = payload
            .pointer(&format!("/data/queueStatus/{qkey}/ids"))
            .cloned()
            .unwrap_or_else(|| json!([]));
        let cursor = continuation_cursor(last.as_ref(), &ids);
        let missing = cursor.is_none();
        if queue.guard.observe(cursor, more, now) {
            if let Some(last) = last {
                queue.last = last;
                queue.ids = ids;
            }
            queue.retries = 0;
            queue.deadline = None;
            diag::queue(self.epoch, key, diag::QueueDecision::Continue, 0);
            Some(Pull {
                key,
                last: queue.last.clone(),
                ids: queue.ids.clone(),
            })
        } else if !more {
            queue.deadline = None;
            queue.retries = 0;
            diag::queue(self.epoch, key, diag::QueueDecision::Complete, 0);
            None
        } else {
            // Missing/invalid cursors never replace the last valid request cursor.
            if queue.retries < MAX_RETRIES {
                queue.deadline = Some(queue.deadline.map_or(now + WAIT, |at| at.min(now + WAIT)));
                diag::queue(
                    self.epoch,
                    key,
                    if missing {
                        diag::QueueDecision::Missing
                    } else {
                        diag::QueueDecision::Repeated
                    },
                    queue.retries,
                );
            } else {
                queue.deadline = None;
                diag::queue(
                    self.epoch,
                    key,
                    diag::QueueDecision::Exhausted,
                    queue.retries,
                );
            }
            None
        }
    }
    /// Register a known pull queue the auth reply did not list, from a valid page.
    ///
    /// Only error-free pages with `more=1` and a valid cursor qualify; pulling unknown
    /// queues speculatively has closed sockets with `3002 NOT_FOUND_WORKER`.
    fn discover(&mut self, key: Key, payload: &Value) -> Option<()> {
        let more = payload.pointer("/data/more").and_then(Value::as_i64) == Some(1);
        if !more {
            return None;
        }
        let failed = is_queue_error_payload(payload);
        let qkey = format!("{}_{}", key.0, key.1);
        let last = payload
            .pointer("/data/lastActionId")
            .or_else(|| payload.pointer(&format!("/data/queueStatus/{qkey}/lastId")))
            .cloned();
        let ids = payload
            .pointer(&format!("/data/queueStatus/{qkey}/ids"))
            .cloned()
            .unwrap_or_else(|| json!([]));
        let eligible = KNOWN_PULL_COMMANDS.contains(&key.0)
            && !failed
            && self.queues.len() < MAX_QUEUES
            && continuation_cursor(last.as_ref(), &ids).is_some();
        if !eligible {
            diag::queue(self.epoch, key, diag::QueueDecision::Unadvertised, 0);
            return None;
        }
        self.queues.insert(
            key,
            Queue {
                guard: CatchupGuard::default(),
                last: last.unwrap_or_else(|| json!(1)),
                ids,
                request: String::new(),
                in_flight: false,
                deadline: None,
                retries: 0,
            },
        );
        diag::queue(self.epoch, key, diag::QueueDecision::DiscoveredFromPage, 0);
        Some(())
    }
    /// Mark a successful write. No second request is scheduled before its reply or timeout.
    pub fn sent(&mut self, key: Key, request: String, now: Instant) {
        if let Some(queue) = self.queues.get_mut(&key) {
            queue.request = request;
            queue.in_flight = true;
            queue.deadline = Some(now + WAIT);
        }
    }
    /// Resume idempotent reads even if no push arrives; work per scheduler tick is bounded.
    pub fn due(&mut self, now: Instant) -> Vec<Pull> {
        let mut out = Vec::new();
        for (&key, queue) in &mut self.queues {
            if out.len() >= 8 {
                break;
            }
            if queue.deadline.is_none_or(|at| at > now) {
                continue;
            }
            if queue.retries >= MAX_RETRIES {
                queue.deadline = None;
                queue.in_flight = false;
                diag::queue(
                    self.epoch,
                    key,
                    diag::QueueDecision::Exhausted,
                    queue.retries,
                );
                continue;
            }
            queue.in_flight = false;
            queue.deadline = None;
            queue.retries += 1;
            diag::queue(self.epoch, key, diag::QueueDecision::Resume, queue.retries);
            out.push(Pull {
                key,
                last: queue.last.clone(),
                ids: queue.ids.clone(),
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page(last: u64, more: bool) -> Value {
        json!({"data":{"lastActionId":last,"more":if more{1}else{0}}})
    }
    #[test]
    fn more_than_200_advancing_control_pages_and_server_defined_queues_keep_draining() {
        let now = Instant::now();
        let mut queues = PullQueues::new(7);
        for key in [(603, 1), (604, 0), (510, 0), (511, 1), (610, 1)] {
            queues.register(key, "initial".into(), now, None);
            for n in 2..402 {
                let pull = queues.page(key, &page(n, true), now).unwrap();
                assert_eq!(pull.last, json!(n));
                queues.sent(key, format!("req-{n}"), now);
                assert!(queues.due(now + Duration::from_secs(1)).is_empty());
            }
            assert!(queues.page(key, &page(402, false), now).is_none());
        }
        assert!(queues.due(now + Duration::from_secs(300)).is_empty());
    }
    #[test]
    fn repeated_cursor_resumes_without_any_new_push_and_exhausts_finite_retries() {
        let now = Instant::now();
        let mut queues = PullQueues::new(1);
        let key = (603, 1);
        queues.register(key, "initial".into(), now, None);
        let pull = queues.page(key, &page(2, true), now).unwrap();
        assert_eq!(pull.last, json!(2));
        queues.sent(key, "r1".into(), now);
        assert!(queues.page(key, &page(2, true), now).is_none());
        assert!(queues.due(now + Duration::from_secs(29)).is_empty());
        for n in 1..=3 {
            let time = now + WAIT * n;
            let due = queues.due(time);
            assert_eq!(due.len(), 1);
            assert_eq!(due[0].last, json!(2));
            queues.sent(key, format!("retry-{n}"), time);
            assert!(queues.page(key, &page(2, true), time).is_none());
        }
        assert!(queues.due(now + Duration::from_secs(300)).is_empty());
    }
    #[test]
    fn in_flight_timeout_retries_once_per_deadline_and_late_reply_cannot_duplicate_pull() {
        let now = Instant::now();
        let mut queues = PullQueues::new(1);
        let key = (603, 1);
        queues.register(key, "old".into(), now, None);
        assert!(queues.due(now + WAIT - Duration::from_millis(1)).is_empty());
        assert_eq!(queues.due(now + WAIT).len(), 1);
        queues.sent(key, "new".into(), now + WAIT);
        let mut stale = page(2, true);
        stale["data"]["reqId"] = json!("old");
        assert!(queues.page(key, &stale, now + WAIT).is_none());
        assert!(queues.due(now + WAIT + Duration::from_secs(1)).is_empty());
    }
    #[test]
    fn completed_queue_and_reconnected_epoch_have_no_old_retry_timers() {
        let now = Instant::now();
        let key = (603, 1);
        let mut old = PullQueues::new(1);
        old.register(key, "old".into(), now, None);
        assert!(old.page(key, &page(1, true), now).is_none());
        let mut next = PullQueues::new(2);
        assert!(next.due(now + Duration::from_secs(60)).is_empty());
        // A valid page on the new connection is discovered fresh; no old timer carries over.
        assert!(next.page(key, &page(2, true), now).is_some());
        assert!(next.due(now + Duration::from_secs(1)).is_empty());
        assert!(old.page(key, &page(1, false), now).is_none());
        assert!(old.due(now + Duration::from_secs(60)).is_empty());
    }
    #[test]
    fn missing_cursor_retries_exact_previous_cursor_without_inventing_progress() {
        let now = Instant::now();
        let key = (603, 1);
        let mut queues = PullQueues::new(1);
        queues.register(key, "first".into(), now, None);
        assert!(queues.page(key, &json!({"data":{"more":1}}), now).is_none());
        let retry = queues.due(now + WAIT);
        assert_eq!(retry.len(), 1);
        assert_eq!(retry[0].last, json!(1));
        assert_eq!(retry[0].ids, json!([]));
    }
    #[test]
    fn progress_after_pause_resets_retry_budget_and_cancels_old_deadline() {
        let now = Instant::now();
        let key = (603, 1);
        let mut queues = PullQueues::new(1);
        queues.register(key, "first".into(), now, None);
        queues.page(key, &page(1, true), now);
        assert_eq!(queues.due(now + WAIT).len(), 1);
        queues.sent(key, "retry".into(), now + WAIT);
        let pull = queues.page(key, &page(2, true), now + WAIT).unwrap();
        assert_eq!(pull.last, json!(2));
        queues.sent(key, "progress".into(), now + WAIT);
        assert_eq!(queues.queues[&key].retries, 0);
        assert!(queues.page(key, &page(2, false), now + WAIT).is_none());
        assert!(queues.due(now + WAIT + WAIT).is_empty());
    }

    #[test]
    fn late_known_queue_is_discovered_from_a_valid_page_and_continues_from_its_cursor() {
        let now = Instant::now();
        let mut queues = PullQueues::new(3);
        let pull = queues.page((510, 1), &page(40, true), now).unwrap();
        assert_eq!(pull.last, json!(40));
        queues.sent((510, 1), "req-1".into(), now);
        assert!(queues.page((510, 1), &page(41, true), now).is_some());
    }

    #[test]
    fn undiscoverable_pages_are_not_pulled() {
        let now = Instant::now();
        let mut queues = PullQueues::new(3);
        assert!(queues.page((999, 0), &page(5, true), now).is_none());
        assert!(queues.page((510, 2), &page(5, false), now).is_none());
        let failed = json!({"error_code":-1,"data":{"lastActionId":5,"more":1}});
        assert!(queues.page((511, 2), &failed, now).is_none());
        let missing = json!({"data":{"more":1}});
        assert!(queues.page((511, 3), &missing, now).is_none());
        assert!(queues.queues.is_empty());
    }

    #[test]
    fn reconnect_starts_with_no_discovered_queues() {
        let now = Instant::now();
        let mut first = PullQueues::new(1);
        assert!(first.page((511, 1), &page(9, true), now).is_some());
        let second = PullQueues::new(2);
        assert!(second.queues.is_empty());
    }

    #[test]
    fn queue_error_not_marked_completed_and_backoff_preserved() {
        let now = Instant::now();
        let key = (603, 1);
        let mut queues = PullQueues::new(1);
        queues.register(key, "req1".into(), now, Some((json!(42), json!(["41"]))));

        // 1. Numeric error code
        let err_num = json!({"error_code": 500, "error_message": "Internal Server Error"});
        assert!(queues.page(key, &err_num, now).is_none());
        assert_eq!(queues.queues[&key].last, json!(42));
        assert_eq!(queues.queues[&key].ids, json!(["41"]));
        // Due after WAIT because it was NOT completed!
        let due = queues.due(now + WAIT);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].last, json!(42));
        assert_eq!(queues.queues[&key].retries, 1);
        queues.sent(key, "retry1".into(), now + WAIT);

        // 2. String error code
        let err_str = json!({"error_code": "-1", "data": null});
        assert!(queues.page(key, &err_str, now + WAIT).is_none());
        assert_eq!(queues.queues[&key].retries, 1); // Not reset to 0!
        let due2 = queues.due(now + WAIT * 2);
        assert_eq!(due2.len(), 1);
        assert_eq!(queues.queues[&key].retries, 2);
        queues.sent(key, "retry2".into(), now + WAIT * 2);

        // 3. Nested error under data
        let err_nested = json!({"data": {"error_code": 1002, "more": 0}});
        assert!(queues.page(key, &err_nested, now + WAIT * 2).is_none());
        assert_eq!(queues.queues[&key].retries, 2); // Still preserved

        // 4. Truly successful page resets retries to 0
        let success = json!({"data": {"lastActionId": 43, "more": 1}});
        let pull = queues.page(key, &success, now + WAIT * 3).unwrap();
        assert_eq!(pull.last, json!(43));
        assert_eq!(queues.queues[&key].retries, 0); // Reset to 0 only on true success!
    }
}
