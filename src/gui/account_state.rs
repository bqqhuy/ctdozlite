//! Account-scoped inbox, alias and private-chat presentation helpers.
use super::*;
impl ZaloGuiApp {
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
            if let Some(alias) = self.server_aliases.get(&thread.id) {
                if thread.presentation.server_alias.as_deref() != Some(thread.name.as_str()) {
                    thread.presentation.base_name = Some(thread.name.clone());
                }
                thread.name = alias.clone();
                thread.presentation.server_alias = Some(alias.clone());
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
