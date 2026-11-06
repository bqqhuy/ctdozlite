//! Evidence-preserving inbox metadata, independent from the friendship relation.
use crate::chat::FriendshipStatus;
use serde::{Deserialize, Serialize};
use serde_json::Value;
/// Missing metadata stays unknown; a loaded contact name is not friendship evidence.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Metadata {
    pub responded_by_me: Option<bool>,
    pub outside: Option<i64>,
    pub official_account: Option<bool>,
}
/// Classification candidate before applying the account's server display configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Unknown,
    Normal,
    StrangerCandidate,
}
fn flag(value: &Value) -> Option<bool> {
    value
        .as_bool()
        .or_else(|| match crate::api_response::integer(value)? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        })
}
impl Metadata {
    /// Apply explicit server fields only; partial updates never erase previously known fields.
    pub fn apply(&mut self, value: &Value) {
        if let Some(responded) = value.get("respondedByMe").and_then(flag) {
            self.responded_by_me = Some(responded);
        }
        if let Some(outside) = value
            .get("outside")
            .and_then(crate::api_response::integer)
            .filter(|state| (0..=2).contains(state))
        {
            self.outside = Some(outside);
        }
    }
    /// Preserve known server fields when a cache/history copy omits them.
    pub fn merge(&mut self, incoming: &Self, authoritative: bool) {
        if incoming.responded_by_me.is_some()
            && (authoritative || self.responded_by_me.is_none())
            && (self.responded_by_me != Some(true) || incoming.responded_by_me == Some(true))
        {
            self.responded_by_me = incoming.responded_by_me;
        }
        if incoming.outside.is_some() && (authoritative || self.outside.is_none()) {
            self.outside = incoming.outside;
        }
        if incoming.official_account.is_some() && (authoritative || self.official_account.is_none())
        {
            self.official_account = incoming.official_account;
        }
    }
    /// OA type belongs to a profile, never to a message's similarly named `type` field.
    pub fn apply_profile(&mut self, value: &Value) {
        if let Some(kind) = value
            .get("type")
            .and_then(crate::api_response::integer)
            .filter(|kind| *kind >= 0)
        {
            self.official_account = Some(kind >= 1);
        }
    }
    /// Find candidates without claiming that the server has placed them in the stranger box.
    /// Group, Cloud, OA, replied and pinned exceptions follow the PC classifier.
    pub fn classify(&self, friendship: FriendshipStatus, special: bool) -> Class {
        if special
            || friendship == FriendshipStatus::Friend
            || self.responded_by_me == Some(true)
            || self.outside.is_some_and(|outside| outside != 0)
            || self.official_account == Some(true)
        {
            return Class::Normal;
        }
        if friendship == FriendshipStatus::Unknown
            || self.responded_by_me.is_none()
            || self.outside.is_none()
            || self.official_account.is_none()
        {
            return Class::Unknown;
        }
        Class::StrangerCandidate
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_fields_never_classify_a_contact_as_stranger() {
        let mut metadata = Metadata::default();
        assert_eq!(
            metadata.classify(FriendshipStatus::Stranger, false),
            Class::Unknown
        );
        metadata.apply(&serde_json::json!({"respondedByMe":0,"outside":0}));
        metadata.apply_profile(&serde_json::json!({"type":0}));
        metadata.apply_profile(&serde_json::json!({"type":0}));
        assert_eq!(
            metadata.classify(FriendshipStatus::Unknown, false),
            Class::Unknown
        );
        assert_eq!(
            metadata.classify(FriendshipStatus::Stranger, false),
            Class::StrangerCandidate
        );
        assert_eq!(
            metadata.classify(FriendshipStatus::Stranger, true),
            Class::Normal
        );
        for data in [
            serde_json::json!({"respondedByMe":1}),
            serde_json::json!({"outside":2}),
            serde_json::json!({"type":1}),
        ] {
            let mut updated = metadata.clone();
            updated.apply(&data);
            updated.apply_profile(&data);
            assert_eq!(
                updated.classify(FriendshipStatus::Stranger, false),
                Class::Normal
            );
        }
        metadata.apply(&serde_json::json!({"respondedByMe":"invalid","outside":9}));
        assert_eq!(metadata.outside, Some(0));
        assert_eq!(metadata.responded_by_me, Some(false));
        let restored: Metadata =
            serde_json::from_str(&serde_json::to_string(&metadata).unwrap()).unwrap();
        assert_eq!(restored, metadata);
    }
}

/// Account-level inputs are runtime scoped and replaced only by complete successful snapshots.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Policy {
    pub show_online: Option<bool>,
    pub profile_types: std::collections::HashMap<String, bool>,
    pub friends: Option<std::collections::HashSet<String>>,
    pub business: Option<bool>,
    pub direct: Option<bool>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Unknown,
    Normal,
    StrangerBox,
}
impl Policy {
    pub fn friendship(&self, id: &str) -> FriendshipStatus {
        match &self.friends {
            Some(friends) if friends.contains(id) => FriendshipStatus::Friend,
            Some(_) => FriendshipStatus::Stranger,
            None => FriendshipStatus::Unknown,
        }
    }
    pub fn placement(&self, thread: &crate::chat::Thread, special: bool) -> Placement {
        self.placement_metadata(
            &thread.id,
            thread.is_group,
            &thread.presentation.inbox,
            special,
        )
    }
    pub fn placement_metadata(
        &self,
        id: &str,
        group: bool,
        metadata: &Metadata,
        special: bool,
    ) -> Placement {
        let class = metadata.classify(self.friendship(id), special || group);
        match class {
            Class::Normal => Placement::Normal,
            _ if self.business == Some(true) || self.direct == Some(true) => Placement::Normal,
            Class::StrangerCandidate
                if self.business == Some(false) && self.direct == Some(false) =>
            {
                Placement::StrangerBox
            }
            _ => Placement::Unknown,
        }
    }
    /// Sending typing is opt-in separately; unknown/stranger direct relationships get no automatic signal.
    pub fn permits_typing(&self, id: &str, group: bool) -> bool {
        group || self.friendship(id) == FriendshipStatus::Friend
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;
    use serde_json::json;
    fn stranger() -> crate::chat::Thread {
        let mut thread = crate::chat::Thread {
            id: "12".into(),
            ..Default::default()
        };
        thread
            .presentation
            .inbox
            .apply(&json!({"respondedByMe":0,"outside":0}));
        thread.presentation.inbox.apply_profile(&json!({"type":0}));
        thread
    }
    #[test]
    fn server_display_policy_is_separate_from_friendship_and_known_exceptions() {
        let mut policy = Policy {
            friends: Some(Default::default()),
            business: Some(false),
            direct: Some(false),
            ..Default::default()
        };
        let mut thread = stranger();
        assert_eq!(policy.placement(&thread, false), Placement::StrangerBox);
        assert_eq!(policy.placement(&thread, true), Placement::Normal);
        policy.direct = Some(true);
        assert_eq!(policy.placement(&thread, false), Placement::Normal);
        assert_eq!(policy.friendship("12"), FriendshipStatus::Stranger);
        policy.direct = Some(false);
        policy.business = Some(true);
        assert_eq!(policy.placement(&thread, false), Placement::Normal);
        policy.business = Some(false);
        thread.presentation.inbox.official_account = Some(true);
        assert_eq!(policy.placement(&thread, false), Placement::Normal);
        thread = stranger();
        policy.friends.as_mut().unwrap().insert("12".into());
        assert_eq!(policy.placement(&thread, false), Placement::Normal);
        assert!(policy.permits_typing("12", false));
        assert!(!policy.permits_typing("99", false));
        assert!(policy.permits_typing("99", true));
    }
    #[test]
    fn incomplete_metadata_never_moves_a_thread_into_the_stranger_box() {
        let thread = stranger();
        for policy in [
            Policy::default(),
            Policy {
                friends: Some(Default::default()),
                ..Default::default()
            },
        ] {
            assert_eq!(policy.placement(&thread, false), Placement::Unknown);
        }
        let policy = Policy {
            friends: Some(Default::default()),
            business: Some(false),
            direct: Some(false),
            ..Default::default()
        };
        let mut thread = stranger();
        thread.presentation.inbox.outside = None;
        assert_eq!(policy.placement(&thread, false), Placement::Unknown);
    }
    #[test]
    fn an_acknowledged_reply_survives_an_older_negative_preview() {
        let mut metadata = stranger().presentation.inbox;
        metadata.responded_by_me = Some(true);
        let old = stranger().presentation.inbox;
        metadata.merge(&old, true);
        assert_eq!(metadata.responded_by_me, Some(true));
        assert_eq!(
            metadata.classify(FriendshipStatus::Stranger, false),
            Class::Normal
        );
    }
}
