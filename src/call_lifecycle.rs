//! Receiver-side terminal transitions never start local media or create chat rows.
/// Local rows deduplicate terminal updates by call identity, not wall-clock seconds.
pub fn local_record_id(call_id: i64) -> String {
    if call_id > 0 {
        format!("call_{call_id}")
    } else {
        format!(
            "call_local_{}_{}",
            crate::login::unix_now(),
            crate::search::generation()
        )
    }
}
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

/// Zalo PC drops call controls older than one minute before they can recreate UI state.
/// Accept seconds or milliseconds because both shapes occur in replayed payloads.
pub fn is_fresh_control_timestamp(event_ts: i64, now_ms: i64) -> bool {
    if event_ts <= 0 || now_ms <= 0 {
        return true;
    }
    let event_ms = if event_ts < 10_000_000_000 {
        event_ts.saturating_mul(1000)
    } else {
        event_ts
    };
    now_ms.saturating_sub(event_ms) <= 60_000
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn call_record_identity_deduplicates_replays_without_colliding_same_second() {
        assert_eq!(local_record_id(42), local_record_id(42));
        assert_ne!(local_record_id(42), local_record_id(43));
        assert_ne!(local_record_id(0), local_record_id(0));
    }
    #[test]
    fn only_the_pending_matching_receiver_with_explicit_self_actor_is_dismissed() {
        assert!(answered_elsewhere(12, 12, false, false, true, 0));
        assert!(!answered_elsewhere(12, 13, false, false, true, 0));
        assert!(!answered_elsewhere(12, 12, false, true, true, 0));
        assert!(!answered_elsewhere(12, 12, true, false, true, 0));
        assert!(!answered_elsewhere(12, 12, false, false, false, 0));
        assert!(!answered_elsewhere(12, 12, false, false, true, -1));
    }

    #[test]
    fn stale_call_controls_match_pc_one_minute_window() {
        let now = 1_800_000_000_000i64;
        assert!(is_fresh_control_timestamp(now - 60_000, now));
        assert!(!is_fresh_control_timestamp(now - 60_001, now));
        assert!(is_fresh_control_timestamp((now / 1000) - 60, now));
        assert!(is_fresh_control_timestamp(0, now));
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
    fn terminal_control_suppresses_replayed_request_on_both_transports() {
        let now = std::time::Instant::now();
        let mut calls = TerminalCalls::default();
        assert!(!calls.contains("account-a", 42, now));
        calls.remember("account-a", 42, now);
        // WS and HTTP long-poll share this engine-owned set, so duplicate delivery is idempotent.
        assert!(calls.contains("account-a", 42, now));
        calls.remember("account-a", 42, now);
        assert!(calls.contains("account-a", 42, now));
        assert!(!calls.contains("account-b", 42, now));
    }
    #[test]
    fn missing_or_invalid_status_is_not_success() {
        assert_eq!(integer(&serde_json::Value::Null), None);
        assert_eq!(integer(&serde_json::json!("invalid")), None);
        assert_eq!(integer(&serde_json::json!("0")), Some(0));
    }
}
