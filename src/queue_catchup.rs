//! Bound automatic queue pagination without guessing or advancing server cursors.

use std::collections::HashSet;
use std::time::{Duration, Instant};

/// In-memory admission guard, owned by one HTTP receiver or WebSocket queue.
#[derive(Clone, Debug, Default)]
pub(crate) struct CatchupGuard {
    seen: HashSet<String>,
    blocked_until: Option<Instant>,
}

impl CatchupGuard {
    /// Whether another request may start; a stalled chain rests for thirty seconds.
    pub(crate) fn ready(&mut self, now: Instant) -> bool {
        if self.blocked_until.is_some_and(|until| now < until) {
            return false;
        }
        if self.blocked_until.take().is_some() {
            self.seen.clear();
        }
        true
    }

    /// Admit a continuation only once per cursor, with a bounded chain of 64 pages.
    /// A changed payload or request ID is not evidence of cursor progress.
    pub(crate) fn observe(&mut self, cursor: Option<String>, more: bool, now: Instant) -> bool {
        if !self.ready(now) {
            return false;
        }
        if !more {
            self.seen.clear();
            return false;
        }
        if self.seen.len() >= 64
            || cursor
                .as_ref()
                .is_none_or(|c| c.is_empty() || self.seen.contains(c))
        {
            self.blocked_until = Some(now + Duration::from_secs(30));
            return false;
        }
        if let Some(cursor) = cursor {
            self.seen.insert(cursor);
        }
        true
    }

    /// Register the request's current cursor so an unchanged first response cannot loop.
    pub(crate) fn seed(&mut self, cursor: String) {
        self.seen.insert(cursor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_and_cyclic_cursors_stop_until_cooldown() {
        let now = Instant::now();
        let mut guard = CatchupGuard::default();
        guard.seed("1".into());
        assert!(guard.observe(Some("2".into()), true, now));
        assert!(!guard.observe(Some("1".into()), true, now));
        assert!(!guard.ready(now + Duration::from_secs(29)));
        assert!(guard.ready(now + Duration::from_secs(30)));
        assert!(guard.observe(Some("3".into()), true, now + Duration::from_secs(30)));
    }
    #[test]
    fn missing_cursors_and_unbounded_advancement_are_bounded() {
        let now = Instant::now();
        let mut missing = CatchupGuard::default();
        assert!(!missing.observe(None, true, now));
        assert!(!missing.ready(now));
        let mut advancing = CatchupGuard::default();
        for n in 0..64 {
            assert!(advancing.observe(Some(n.to_string()), true, now));
        }
        assert!(!advancing.observe(Some("64".into()), true, now));
    }
    #[test]
    fn completed_chain_resets_without_blocking_a_new_delivery() {
        let now = Instant::now();
        let mut guard = CatchupGuard::default();
        assert!(guard.observe(Some("2".into()), true, now));
        assert!(!guard.observe(Some("2".into()), false, now));
        assert!(guard.ready(now));
        assert!(guard.observe(Some("2".into()), true, now));
    }
}
