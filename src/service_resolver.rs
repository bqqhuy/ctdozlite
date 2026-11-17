//! Unified Service Resolver & Legacy Compatibility Fallback (Zalo Parity C04).
//!
//! Provides intelligent endpoint resolution, route version mapping, and transparent
//! fallback handling between primary and backup service map domains.

use std::collections::BTreeMap;

/// Service identity kept in diagnostics without retaining request paths or query data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceKey {
    Chat,
    Friend,
    Conversation,
    Profile,
    Group,
    File,
    Reaction,
    VoiceCall,
    Sticker,
    Other,
}

impl ServiceKey {
    pub(crate) fn from_key(key: &str) -> Self {
        match key {
            "chat" => Self::Chat,
            "friend" => Self::Friend,
            "conversation" => Self::Conversation,
            "profile" => Self::Profile,
            "group" => Self::Group,
            "file" => Self::File,
            "reaction" => Self::Reaction,
            "voice_call" => Self::VoiceCall,
            "sticker" => Self::Sticker,
            _ => Self::Other,
        }
    }
}

/// One bounded HTTP attempt summary. It deliberately contains neither URL paths nor IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestAttemptTiming {
    pub phase: TimingPhase,
    pub service: ServiceKey,
    pub candidate_index: Option<u8>,
    pub elapsed_ms: u32,
    pub result: AttemptResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingPhase {
    HttpRequest,
    LinkPreview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptResult {
    Success,
    HttpFailure,
    ConnectFailure,
    Timeout,
    TransportFailure,
    PreviewReady,
    PreviewCacheHit,
    PreviewFallback,
}

/// Service endpoint resolver with dynamic fallback support.
#[derive(Debug, Clone, Default)]
pub struct ServiceResolver {
    service_endpoints: BTreeMap<String, Vec<String>>,
    active_indices: BTreeMap<String, usize>,
}

impl ServiceResolver {
    pub fn new() -> Self {
        let mut resolver = Self::default();
        // Populate canonical endpoints from endpoints.rs as primary, with safe fallback backups
        let chat_primary = crate::endpoints::service("chat")
            .unwrap_or(crate::endpoints::CHAT)
            .to_string();
        let group_primary = crate::endpoints::service("group")
            .unwrap_or(crate::endpoints::GROUP)
            .to_string();
        let profile_primary = crate::endpoints::service("profile")
            .unwrap_or(crate::endpoints::PROFILE)
            .to_string();
        let file_primary = crate::endpoints::service("file")
            .unwrap_or(crate::endpoints::FILE)
            .to_string();

        resolver.register_service(
            "chat",
            vec![chat_primary, crate::endpoints::CHAT_LEGACY.into()],
        );
        resolver.register_service(
            "group",
            vec![group_primary, crate::endpoints::GROUP_LEGACY.into()],
        );
        resolver.register_service(
            "profile",
            vec![profile_primary, crate::endpoints::PROFILE_LEGACY.into()],
        );
        resolver.register_service(
            "file",
            vec![file_primary, crate::endpoints::FILE_LEGACY.into()],
        );
        resolver
    }

    /// Register a service name with a priority list of host domains.
    pub fn register_service(&mut self, service_key: &str, hosts: Vec<String>) {
        if !hosts.is_empty() {
            self.service_endpoints
                .insert(service_key.to_string(), hosts);
            self.active_indices.insert(service_key.to_string(), 0);
        }
    }

    /// Replace the resolver snapshot from the sanitized session map. Discovered
    /// candidates retain their server order; canonical endpoints are last-resort.
    pub(crate) fn sync_from_domains(&mut self, domains: &crate::session::Domains) {
        self.set_from_domains(domains, false);
    }

    /// Rebuild a resolver snapshot after restoring a session/account.
    pub(crate) fn reset_from_domains(&mut self, domains: &crate::session::Domains) {
        self.set_from_domains(domains, true);
    }

    fn set_from_domains(&mut self, domains: &crate::session::Domains, reset: bool) {
        for key in [
            "chat",
            "friend",
            "conversation",
            "profile",
            "group",
            "file",
            "reaction",
            "voice_call",
            "sticker",
            "alias",
            "label",
        ] {
            let Some(canonical) = crate::endpoints::service(key) else {
                continue;
            };
            let mut candidates = domains
                .service_fallbacks
                .get(canonical)
                .into_iter()
                .flatten()
                .filter(|host| {
                    crate::session::trusted_service_url(host)
                        && !((key == "chat"
                            && (host.contains("-msg-chat.") || host.contains("tt-chat2-wpa")))
                            || (key == "conversation" && host.contains("-ctl-chat.")))
                })
                .cloned()
                .collect::<Vec<_>>();
            candidates.truncate(crate::session::MAX_SERVICE_CANDIDATES.saturating_sub(1));
            if !candidates.iter().any(|host| host == canonical) {
                candidates.push(canonical.to_owned());
            }
            candidates.truncate(crate::session::MAX_SERVICE_CANDIDATES);
            if reset || self.service_endpoints.get(key) != Some(&candidates) {
                self.register_service(key, candidates);
            }
        }
    }

    pub(crate) fn candidates_for_url(&self, url: &str) -> Option<(String, Vec<String>)> {
        for (key, hosts) in &self.service_endpoints {
            let canonical = crate::endpoints::service(key)?;
            let Some(path) = url.strip_prefix(canonical) else {
                continue;
            };
            if !path.starts_with('/') {
                continue;
            }
            let active = self.active_indices.get(key).copied().unwrap_or(0) % hosts.len();
            let ordered = (0..hosts.len())
                .map(|offset| &hosts[(active + offset) % hosts.len()])
                .map(|host| format!("{host}{path}"))
                .collect();
            return Some((key.clone(), ordered));
        }
        None
    }

    pub(crate) fn record_success_for_url(&mut self, service_key: &str, url: &str) {
        let Some(hosts) = self.service_endpoints.get(service_key) else {
            return;
        };
        if let Some(index) = hosts.iter().position(|host| url.starts_with(host)) {
            self.active_indices.insert(service_key.to_owned(), index);
        }
    }

    /// Resolve the current active host for a service key.
    pub fn resolve_host<'a>(&'a self, service_key: &str) -> Option<&'a str> {
        let hosts = self.service_endpoints.get(service_key)?;
        let idx = self.active_indices.get(service_key).copied().unwrap_or(0);
        hosts.get(idx).map(|s| s.as_str())
    }

    /// Resolve full URL combining current active host and path.
    pub fn resolve_url(&self, service_key: &str, path: &str) -> String {
        let host = self
            .resolve_host(service_key)
            .unwrap_or(crate::endpoints::WPA_CHAT);
        let clean_path = if path.starts_with('/') {
            path
        } else {
            &format!("/{path}")
        };
        format!("{host}{clean_path}")
    }

    /// Rotate to the next fallback host when a server failure occurs.
    pub fn record_failure(&mut self, service_key: &str) -> bool {
        if let Some(hosts) = self.service_endpoints.get(service_key) {
            if hosts.len() > 1 {
                let current_idx = self
                    .active_indices
                    .entry(service_key.to_string())
                    .or_insert(0);
                *current_idx = (*current_idx + 1) % hosts.len();
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_resolver_fallback() {
        let mut resolver = ServiceResolver::new();
        assert_eq!(
            resolver.resolve_url("chat", "/api/message/send"),
            "https://tt-chat-wpa.chat.zalo.me/api/message/send"
        );

        // Record failure -> rotates to secondary fallback host
        assert!(resolver.record_failure("chat"));
        assert_eq!(
            resolver.resolve_url("chat", "/api/message/send"),
            "https://tt-chat3-wpa.api.zaloapp.com/api/message/send"
        );

        // Unknown service falls back to default base host
        assert_eq!(
            resolver.resolve_url("unknown_service", "/api/ping"),
            "https://wpa.chat.zalo.me/api/ping"
        );
    }
}
