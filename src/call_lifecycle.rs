//! Receiver-side terminal transitions never start local media or create chat rows.
pub fn answered_elsewhere(
    call_id: i64,
    event_id: i64,
    is_caller: bool,
    connected: bool,
    actor_is_self: bool,
    status: i64,
) -> bool {
    call_id > 0 && event_id == call_id && !is_caller && !connected && status == 0 && actor_is_self
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_pending_matching_receiver_with_explicit_self_actor_is_dismissed() {
        assert!(answered_elsewhere(12, 12, false, false, true, 0));
        assert!(!answered_elsewhere(12, 13, false, false, true, 0));
        assert!(!answered_elsewhere(12, 12, false, true, true, 0));
        assert!(!answered_elsewhere(12, 12, true, false, true, 0));
        assert!(!answered_elsewhere(12, 12, false, false, false, 0));
        assert!(!answered_elsewhere(12, 12, false, false, true, -1));
    }
}

/// Bounded terminal history owned by one call engine; keys include the account.
#[derive(Default)]
pub struct TerminalCalls {
    entries: std::collections::VecDeque<(String, i64, std::time::Instant)>,
}
impl TerminalCalls {
    /// Remember a terminal call without retaining signalling payloads.
    pub fn remember(&mut self, owner: &str, id: i64, now: std::time::Instant) {
        if owner.is_empty() || id <= 0 {
            return;
        }
        self.entries.retain(|(account, call, at)| {
            now.saturating_duration_since(*at) < std::time::Duration::from_secs(300)
                && !(account == owner && *call == id)
        });
        self.entries.push_back((owner.to_owned(), id, now));
        while self.entries.len() > 128 {
            self.entries.pop_front();
        }
    }
    /// Ignore replay only for the matching account within the replay window.
    pub fn contains(&self, owner: &str, id: i64, now: std::time::Instant) -> bool {
        self.entries.iter().any(|(account, call, at)| {
            account == owner
                && *call == id
                && now.saturating_duration_since(*at) < std::time::Duration::from_secs(300)
        })
    }
}

// Preserve the existing helper surface while keeping generic protocol parsing out of VoIP.
pub use crate::api_response::integer;

#[cfg(test)]
mod replay_tests {
    use super::*;
    #[test]
    fn terminal_history_is_account_scoped_bounded_and_expires() {
        let now = std::time::Instant::now();
        let mut calls = TerminalCalls::default();
        calls.remember("account-a", 12, now);
        assert!(calls.contains("account-a", 12, now));
        assert!(!calls.contains("account-b", 12, now));
        assert!(!calls.contains("account-a", 13, now));
        assert!(!calls.contains("account-a", 12, now + std::time::Duration::from_secs(301)));
        for id in 13..=150 {
            calls.remember("account-a", id, now);
        }
        assert!(!calls.contains("account-a", 12, now));
        assert_eq!(calls.entries.len(), 128);
    }
    #[test]
    fn missing_or_invalid_status_is_not_success() {
        assert_eq!(integer(&serde_json::Value::Null), None);
        assert_eq!(integer(&serde_json::json!("invalid")), None);
        assert_eq!(integer(&serde_json::json!("0")), Some(0));
    }
}
