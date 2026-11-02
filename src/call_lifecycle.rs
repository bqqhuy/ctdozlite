//! Receiver-side terminal transitions never start local media or create chat rows.
pub fn answered_elsewhere(
    call_id: i64,
    event_id: i64,
    is_caller: bool,
    connected: bool,
    is_group: bool,
    actor_is_self: bool,
    status: i64,
) -> bool {
    call_id != 0
        && event_id == call_id
        && !is_caller
        && !connected
        && status == 0
        && (!is_group || actor_is_self)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_pending_matching_receiver_is_dismissed() {
        assert!(answered_elsewhere(12, 12, false, false, false, false, 0));
        assert!(!answered_elsewhere(12, 13, false, false, false, false, 0));
        assert!(!answered_elsewhere(12, 12, false, true, false, false, 0));
        assert!(!answered_elsewhere(12, 12, true, false, false, false, 0));
        assert!(!answered_elsewhere(12, 12, false, false, true, false, 0));
        assert!(answered_elsewhere(12, 12, false, false, true, true, 0));
    }
}
