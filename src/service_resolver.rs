//! Unified Service Resolver & Legacy Compatibility Fallback (Zalo Parity C04).
//!
//! Provides intelligent endpoint resolution, route version mapping, and transparent
//! fallback handling between primary and backup service map domains.

use std::collections::BTreeMap;

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
        let chat_primary = crate::endpoints::service("chat").unwrap_or("https://tt-chat-wpa.chat.zalo.me").to_string();
        let group_primary = crate::endpoints::service("group").unwrap_or("https://tt-group-wpa.chat.zalo.me").to_string();
        let profile_primary = crate::endpoints::service("profile").unwrap_or("https://tt-profile-wpa.chat.zalo.me").to_string();
        let file_primary = crate::endpoints::service("file").unwrap_or("https://tt-files-wpa.chat.zalo.me").to_string();

        resolver.register_service("chat", vec![
            chat_primary,
            "https://tt-chat3-wpa.api.zaloapp.com".into(),
        ]);
        resolver.register_service("group", vec![
            group_primary,
            "https://tt-group-wpa.zaloapp.com".into(),
        ]);
        resolver.register_service("profile", vec![
            profile_primary,
            "https://tt-profile-wpa.zaloapp.com".into(),
        ]);
        resolver.register_service("file", vec![
            file_primary,
            "https://tt-files-wpa.zaloapp.com".into(),
        ]);
        resolver
    }

    /// Register a service name with a priority list of host domains.
    pub fn register_service(&mut self, service_key: &str, hosts: Vec<String>) {
        if !hosts.is_empty() {
            self.service_endpoints.insert(service_key.to_string(), hosts);
            self.active_indices.insert(service_key.to_string(), 0);
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
        let host = self.resolve_host(service_key).unwrap_or("https://wpa.chat.zalo.me");
        let clean_path = if path.starts_with('/') { path } else { &format!("/{path}") };
        format!("{host}{clean_path}")
    }

    /// Rotate to the next fallback host when a server failure occurs.
    pub fn record_failure(&mut self, service_key: &str) -> bool {
        if let Some(hosts) = self.service_endpoints.get(service_key) {
            if hosts.len() > 1 {
                let current_idx = self.active_indices.entry(service_key.to_string()).or_insert(0);
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
