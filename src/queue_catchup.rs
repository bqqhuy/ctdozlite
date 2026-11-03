//! Bound automatic queue pagination without guessing or advancing server cursors.

use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

/// In-memory admission guard, owned by one HTTP receiver or WebSocket queue.
#[derive(Clone, Debug, Default)]
pub(crate) struct CatchupGuard {
    seen: HashSet<[u8; 32]>,
    recent: VecDeque<[u8; 32]>,
    blocked_until: Option<Instant>,
}

impl CatchupGuard {
    /// Whether another request may start; a stalled chain rests for thirty seconds.
    pub(crate) fn ready(&mut self, now: Instant) -> bool {
        if self.blocked_until.is_some_and(|until| now < until) {
            return false;
        }
        self.blocked_until = None;
        true
    }

    /// Admit advancing cursors without a page cap; retain a bounded recent cycle detector.
    pub(crate) fn observe(&mut self, cursor: Option<String>, more: bool, now: Instant) -> bool {
        if !more {
            self.seen.clear();
            self.recent.clear();
            self.blocked_until = None;
            return false;
        }
        if let Some(cursor) = cursor.filter(|c| !c.is_empty() && c.len() <= 256 * 1024) {
            let key = Self::fingerprint(&cursor);
            if !self.seen.contains(&key) {
                self.remember(key);
                self.blocked_until = None;
                return true;
            }
        }
        // Duplicate replies cannot keep postponing the already scheduled retry.
        if self.blocked_until.is_none_or(|at| at <= now) {
            self.blocked_until = Some(now + Duration::from_secs(30));
        }
        false
    }

    /// Register the current request's cursor without growing memory with the backlog.
    pub(crate) fn seed(&mut self, cursor: String) {
        self.remember(Self::fingerprint(&cursor));
    }

    fn fingerprint(cursor: &str) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        Sha256::digest(cursor.as_bytes()).into()
    }
    fn remember(&mut self, key: [u8; 32]) {
        if self.seen.insert(key) {
            self.recent.push_back(key);
            if self.recent.len() > 1024 {
                if let Some(old) = self.recent.pop_front() {
                    self.seen.remove(&old);
                }
            }
        }
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
    fn missing_cursors_pause_but_advancing_backlogs_keep_draining_with_bounded_memory() {
        let now = Instant::now();
        let mut missing = CatchupGuard::default();
        assert!(!missing.observe(None, true, now));
        assert!(!missing.ready(now));
        let mut advancing = CatchupGuard::default();
        for n in 0..10000 {
            assert!(advancing.observe(Some(n.to_string()), true, now));
        }
        assert!(advancing.seen.len() <= 1024);
        assert!(advancing.recent.len() <= 1024);
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
