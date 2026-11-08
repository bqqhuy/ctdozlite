//! DNS-over-HTTPS resolver for CDN media resilience (WP15-3, v0.015.2, v0.016 Task 3).
//!
//! When media download fails with DNS/connection errors, fallback to DoH to resolve
//! CDN hostnames and retry with IP addresses directly. Supports multiple DoH endpoints
//! with TTL-aware caching and negative cache.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use anyhow::{anyhow, Result};

pub const GOOGLE_DOH_ENDPOINT: &str = "https://dns.google/resolve";
pub const CLOUDFLARE_DOH_ENDPOINT: &str = "https://cloudflare-dns.com/dns-query";

/// DoH endpoints tried in order of preference
pub const DOH_ENDPOINTS: &[&str] = &[
    GOOGLE_DOH_ENDPOINT,
    CLOUDFLARE_DOH_ENDPOINT,
    "https://1.1.1.1/dns-query",
];

/// DNS response schema (simplified; only used fields)
#[derive(serde::Deserialize, Debug)]
pub struct DnsResponse {
    #[serde(default, rename = "Answer")]
    pub answer: Vec<DnsAnswer>,
}

#[derive(serde::Deserialize, Debug)]
pub struct DnsAnswer {
    #[serde(default)]
    pub data: String,
    #[serde(default, rename = "TTL")]
    pub ttl: u32,
}

/// DNS-over-HTTPS resolver with TTL-aware cache
pub struct DohResolver {
    /// Cached resolutions: (IpAddrs, expiry_time)
    cache: Mutex<HashMap<String, (Vec<IpAddr>, Instant)>>,
    /// Negative cache (failed lookups): host -> expiry_time
    negative: Mutex<HashMap<String, Instant>>,
    /// HTTP client (ephemeral, no proxy, 5s timeout)
    client: reqwest::Client,
}

impl DohResolver {
    /// Create a new DoH resolver
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .no_proxy()
            .build()?;

        Ok(Self {
            cache: Mutex::new(HashMap::new()),
            negative: Mutex::new(HashMap::new()),
            client,
        })
    }

    /// Resolve hostname to IP addresses via DoH with TTL
    /// Returns (Vec<IpAddr>, TTL_seconds)
    pub async fn resolve_with_ttl(&self, host: &str) -> Result<(Vec<IpAddr>, u32)> {
        // Check positive cache
        {
            let cache = self.cache.lock().unwrap();
            if let Some((addrs, expiry)) = cache.get(host) {
                if Instant::now() < *expiry {
                    let remaining_ttl = (*expiry - Instant::now()).as_secs() as u32;
                    return Ok((addrs.clone(), remaining_ttl));
                }
            }
        }

        // Check negative cache
        {
            let negative = self.negative.lock().unwrap();
            if let Some(expiry) = negative.get(host) {
                if Instant::now() < *expiry {
                    return Err(anyhow!("Negative cache hit for {}", host));
                }
            }
        }

        // Try each DoH endpoint
        for endpoint in DOH_ENDPOINTS {
            if let Ok((addrs, ttl)) = self.query_endpoint_with_ttl(endpoint, host).await {
                if !addrs.is_empty() {
                    let cache_ttl = ttl.clamp(60, 600);
                    let expiry = Instant::now() + Duration::from_secs(cache_ttl as u64);
                    self.cache
                        .lock()
                        .unwrap()
                        .insert(host.to_string(), (addrs.clone(), expiry));
                    return Ok((addrs, cache_ttl));
                }
            }
        }

        // All endpoints failed -> add to negative cache (10min)
        self.negative.lock().unwrap().insert(
            host.to_string(),
            Instant::now() + Duration::from_secs(600),
        );
        Err(anyhow!("DoH resolution failed for {}", host))
    }

    /// Resolve hostname to IP addresses via DoH
    pub async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>> {
        let (addrs, _) = self.resolve_with_ttl(host).await?;
        Ok(addrs)
    }

    /// Query a single DoH endpoint, returning (IpAddrs, TTL)
    /// Audit A23: Google DoH JSON API takes one RR type parameter at a time.
    /// Query A and AAAA separately and merge results.
    async fn query_endpoint_with_ttl(
        &self,
        endpoint: &str,
        host: &str,
    ) -> Result<(Vec<IpAddr>, u32)> {
        let mut all_addrs = Vec::new();
        let mut max_ttl = 300u32;

        // Query A records (IPv4)
        if let Ok((a_addrs, a_ttl)) = self.query_type(endpoint, host, "A").await {
            all_addrs.extend(a_addrs);
            max_ttl = max_ttl.max(a_ttl);
        }

        // Query AAAA records (IPv6)
        if let Ok((aaaa_addrs, aaaa_ttl)) = self.query_type(endpoint, host, "AAAA").await {
            all_addrs.extend(aaaa_addrs);
            max_ttl = max_ttl.max(aaaa_ttl);
        }

        if all_addrs.is_empty() {
            return Err(anyhow!("No A/AAAA records in response"));
        }

        Ok((all_addrs, max_ttl))
    }

    /// Query a single RR type (A or AAAA) from a DoH endpoint.
    async fn query_type(
        &self,
        endpoint: &str,
        host: &str,
        rr_type: &str,
    ) -> Result<(Vec<IpAddr>, u32)> {
        let url = format!("{}?name={}&type={}", endpoint, host, rr_type);
        let resp = self
            .client
            .get(&url)
            .header("Accept", "application/dns-json")
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("DoH request failed with status {}", resp.status()));
        }
        let dns_resp: DnsResponse = resp.json().await?;

        let allow_loopback = cfg!(test) || std::env::var_os("CTDOZ_ALLOW_TEST_LOOPBACK").is_some();
        let addrs: Vec<IpAddr> = dns_resp
            .answer
            .iter()
            .filter_map(|a| a.data.parse::<IpAddr>().ok())
            .filter(|ip| allow_loopback || !crate::endpoints::is_private_or_loopback_ip(*ip))
            .collect();

        // Empty response for this type is not an error (may not have AAAA, etc.)
        let ttl = dns_resp
            .answer
            .iter()
            .map(|a| a.ttl)
            .max()
            .unwrap_or(300);

        Ok((addrs, ttl))
    }

    /// Invalidate cache entry for a host
    pub fn invalidate(&self, host: &str) {
        self.cache.lock().unwrap().remove(host);
        self.negative.lock().unwrap().remove(host);
    }
}

/// Convenience async function to resolve a domain to IP addresses via the shared DoH resolver (v0.016 Task 3).
pub async fn resolve_ip(host: &str) -> Result<Vec<IpAddr>> {
    static GLOBAL_DOH: std::sync::OnceLock<DohResolver> = std::sync::OnceLock::new();
    let resolver = GLOBAL_DOH.get_or_init(|| {
        DohResolver::new().expect("Failed to initialize DohResolver")
    });
    resolver.resolve(host).await
}

/// Resolve a domain with system DNS first, falling back to DoH if system DNS fails (v0.016 Task 3).
pub async fn resolve_with_system_fallback(host: &str) -> Result<Vec<IpAddr>> {
    if let Ok(addrs) = tokio::net::lookup_host(format!("{host}:443")).await {
        let ips: Vec<IpAddr> = addrs.map(|a| a.ip()).collect();
        if !ips.is_empty() {
            return Ok(ips);
        }
    }
    resolve_ip(host).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_invalidate() {
        let resolver = DohResolver::new().unwrap();
        resolver
            .cache
            .lock()
            .unwrap()
            .insert("test.com".to_string(), (vec![], Instant::now()));
        assert!(resolver.cache.lock().unwrap().contains_key("test.com"));

        resolver.invalidate("test.com");
        assert!(!resolver.cache.lock().unwrap().contains_key("test.com"));
    }

    #[test]
    fn test_dns_json_deserialization() {
        let json_data = r#"{
            "Status": 0,
            "Answer": [
                { "name": "zdn.vn.", "type": 5, "TTL": 120, "data": "cdn.zdn.vn." },
                { "name": "cdn.zdn.vn.", "type": 1, "TTL": 300, "data": "123.30.235.10" },
                { "name": "cdn.zdn.vn.", "type": 28, "TTL": 600, "data": "2405:4800:102:2::10" }
            ]
        }"#;
        let resp: DnsResponse = serde_json::from_str(json_data).unwrap();
        assert_eq!(resp.answer.len(), 3);

        let ips: Vec<IpAddr> = resp
            .answer
            .iter()
            .filter_map(|a| a.data.parse::<IpAddr>().ok())
            .collect();
        assert_eq!(ips.len(), 2);
        assert_eq!(ips[0], "123.30.235.10".parse::<IpAddr>().unwrap());
        assert_eq!(ips[1], "2405:4800:102:2::10".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_doh_endpoints_presence() {
        assert!(DOH_ENDPOINTS.contains(&GOOGLE_DOH_ENDPOINT));
        assert!(DOH_ENDPOINTS.contains(&CLOUDFLARE_DOH_ENDPOINT));
    }
}
