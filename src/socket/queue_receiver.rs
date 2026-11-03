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
impl PullQueues {
    /// Create an empty scheduler for one authenticated socket generation.
    pub fn new(epoch: u64) -> Self {
        Self {
            epoch,
            ..Default::default()
        }
    }
    /// Register an initial pull only after its write succeeds.
    pub fn register(&mut self, key: Key, request: String, now: Instant) {
        let last = json!(1);
        let ids = json!([]);
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
            queues.register(key, "initial".into(), now);
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
        queues.register(key, "initial".into(), now);
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
        queues.register(key, "old".into(), now);
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
        old.register(key, "old".into(), now);
        assert!(old.page(key, &page(1, true), now).is_none());
        let mut next = PullQueues::new(2);
        assert!(next.due(now + Duration::from_secs(60)).is_empty());
        assert!(next.page(key, &page(2, true), now).is_none());
        assert!(old.page(key, &page(1, false), now).is_none());
        assert!(old.due(now + Duration::from_secs(60)).is_empty());
    }
    #[test]
    fn missing_cursor_retries_exact_previous_cursor_without_inventing_progress() {
        let now = Instant::now();
        let key = (603, 1);
        let mut queues = PullQueues::new(1);
        queues.register(key, "first".into(), now);
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
        queues.register(key, "first".into(), now);
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
}
