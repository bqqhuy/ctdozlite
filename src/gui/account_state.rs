//! Account-scoped inbox, alias and private-chat presentation helpers.
use super::*;

/// Resolve a contact display name, prioritizing a non-empty saved phonebook alias in `aliases`.
pub(super) fn resolve_contact_name_from<'a>(
    aliases: &'a HashMap<String, String>,
    uid: &str,
    fallback_name: &'a str,
) -> &'a str {
    let uid = uid.trim();
    if !uid.is_empty() {
        if let Some(alias) = aliases.get(uid) {
            let trimmed = alias.trim();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }
    fallback_name
}

/// Resolve a thread's display name, prioritizing `aliases` for 1-1 chats while preserving group names.
pub(super) fn resolve_thread_name_from<'a>(
    aliases: &'a HashMap<String, String>,
    thread: &'a Thread,
) -> &'a str {
    if thread.is_group {
        thread.name.as_str()
    } else {
        resolve_contact_name_from(aliases, &thread.id, &thread.name)
    }
}

/// Resolve a message sender's display name, prioritizing `aliases` over Zalo display names.
pub(super) fn resolve_message_sender_name_from<'a>(
    aliases: &'a HashMap<String, String>,
    threads: &'a [Thread],
    group_members: &'a HashMap<String, (Vec<crate::chat::SidebarMember>, Option<u64>)>,
    msg: &'a ChatMessage,
    thread_id: &str,
    is_group: bool,
    fallback_name: &'a str,
) -> &'a str {
    let sender_uid = crate::chat::sidebar_sender_id(msg);
    let numeric_msg_sender = (!msg.sender.is_empty()
        && msg.sender != "0"
        && msg.sender.bytes().all(|b| b.is_ascii_digit()))
    .then_some(msg.sender.as_str());
    let lookup_uid = sender_uid
        .as_deref()
        .or(numeric_msg_sender)
        .or((!is_group && !thread_id.is_empty()).then_some(thread_id));

    if let Some(uid) = lookup_uid {
        if let Some(alias) = aliases.get(uid) {
            let trimmed = alias.trim();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }

    let sender_trimmed = msg.sender.trim();
    if !sender_trimmed.is_empty()
        && sender_trimmed != "0"
        && !crate::chat::is_generic_thread_name(sender_trimmed)
    {
        return sender_trimmed;
    }

    if let Some(uid) = lookup_uid {
        if let Some(thread) = threads.iter().find(|t| !t.is_group && t.id == uid) {
            if !thread.name.trim().is_empty() && !crate::chat::is_generic_thread_name(&thread.name)
            {
                return thread.name.as_str();
            }
        }
        if is_group {
            if let Some((members, _)) = group_members.get(thread_id) {
                if let Some(member) = members
                    .iter()
                    .find(|m| m.id == uid && !m.name.trim().is_empty())
                {
                    return member.name.as_str();
                }
            }
        }
    }

    if !sender_trimmed.is_empty() && sender_trimmed != "0" {
        return sender_trimmed;
    }
    fallback_name
}

/// Resolve the quoted sender label of `msg` from loaded messages, quote UID fields, or `aliases`.
pub(super) fn resolve_quote_sender_name_from<'a>(
    aliases: &'a HashMap<String, String>,
    threads: &'a [Thread],
    group_members: &'a HashMap<String, (Vec<crate::chat::SidebarMember>, Option<u64>)>,
    thread: &'a Thread,
    msg: &'a ChatMessage,
    thread_display_name: &'a str,
) -> Option<&'a str> {
    if let Some(reference) = crate::message_navigation::message_reply_reference(msg) {
        if let Some(index) =
            crate::message_navigation::unique_reference_index(&thread.messages, &reference)
        {
            let quoted = &thread.messages[index];
            return Some(if quoted.is_outgoing {
                "Bạn"
            } else {
                resolve_message_sender_name_from(
                    aliases,
                    threads,
                    group_members,
                    quoted,
                    &thread.id,
                    thread.is_group,
                    thread_display_name,
                )
            });
        }
    }
    if let Some(quote_obj) = msg.wire_ref.fields.get("quote") {
        for key in ["ownerId", "fromUid", "uidFrom", "uid"] {
            if let Some(v) = quote_obj.get(key) {
                let found = match v {
                    serde_json::Value::String(s) => {
                        let uid = s.trim();
                        (!uid.is_empty() && uid != "0")
                            .then(|| aliases.get(uid))
                            .flatten()
                    }
                    serde_json::Value::Number(n) if n.is_u64() => {
                        let uid = n.to_string();
                        (uid != "0").then(|| aliases.get(&uid)).flatten()
                    }
                    _ => None,
                };
                if let Some(alias) = found {
                    let trimmed = alias.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed);
                    }
                }
            }
        }
    }
    if let Some(rs) = msg.reply_to_sender.as_deref() {
        let trimmed = rs.trim();
        if !trimmed.is_empty() && trimmed != "0" {
            if let Some(alias) = aliases.get(trimmed) {
                let a = alias.trim();
                if !a.is_empty() {
                    return Some(a);
                }
            }
        }
    }
    None
}

impl ZaloGuiApp {
    /// Resolve a contact's display name, prioritizing `server_aliases` over `fallback_name`.
    pub(super) fn resolve_contact_name<'a>(&'a self, uid: &str, fallback_name: &'a str) -> &'a str {
        resolve_contact_name_from(&self.server_aliases, uid, fallback_name)
    }

    /// Resolve a thread's display name, prioritizing `server_aliases` for 1-1 conversations.
    pub(super) fn resolve_thread_name<'a>(&'a self, thread: &'a Thread) -> &'a str {
        resolve_thread_name_from(&self.server_aliases, thread)
    }

    /// Resolve a message sender's display name, prioritizing `server_aliases`.
    pub(super) fn resolve_message_sender_name<'a>(
        &'a self,
        msg: &'a ChatMessage,
        thread_id: &str,
        is_group: bool,
        fallback_name: &'a str,
    ) -> &'a str {
        resolve_message_sender_name_from(
            &self.server_aliases,
            &self.threads,
            &self.group_members,
            msg,
            thread_id,
            is_group,
            fallback_name,
        )
    }

    pub(super) fn inbox_placement(&self, thread: &Thread) -> crate::inbox::Placement {
        self.inbox_placement_metadata(&thread.id, thread.is_group, &thread.presentation.inbox)
    }
    pub(super) fn inbox_placement_metadata(
        &self,
        id: &str,
        group: bool,
        metadata: &crate::inbox::Metadata,
    ) -> crate::inbox::Placement {
        let special = self.friend_status_map.get(id)
            == Some(&crate::chat::FriendshipStatus::Friend)
            || group
            || self.server_pins.rank(id, group).is_some()
            || (!self.own_cloud_id.is_empty() && id == self.own_cloud_id);
        self.inbox_policy
            .placement_metadata(id, group, metadata, special)
    }
    pub(super) fn apply_server_aliases(&mut self) {
        if !self.server_aliases_ready {
            return;
        }
        for thread in &mut self.threads {
            if thread.is_group {
                continue;
            }
            if let Some(alias) = self
                .server_aliases
                .get(&thread.id)
                .map(|a| a.trim())
                .filter(|a| !a.is_empty())
            {
                if thread.presentation.server_alias.as_deref() != Some(thread.name.as_str()) {
                    thread.presentation.base_name = Some(thread.name.clone());
                }
                thread.name = alias.to_string();
                thread.presentation.server_alias = Some(alias.to_string());
            } else if thread.presentation.server_alias.take().is_some() {
                if let Some(base) = thread.presentation.base_name.take() {
                    thread.name = base;
                }
            }
        }
    }
    pub(super) fn private_access(&self, id: &str, group: bool) -> bool {
        self.hidden_panel.view.allows(id, group)
    }
}
