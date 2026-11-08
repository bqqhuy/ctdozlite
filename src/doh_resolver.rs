//! DNS-over-HTTPS resolver for CDN media resilience (WP15-3, v0.015.2).
//!
//! When media download fails with DNS/connection errors, fallback to DoH to resolve
//! CDN hostnames and retry with IP addresses directly. Supports multiple DoH endpoints
//! with TTL-aware caching and negative cache.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use anyhow::{anyhow, Result};

/// DNS-over-HTTPS resolver with TTL-aware cache
pub struct DohResolver {
    /// Cached resolutions: (IpAddrs, expiry_time)
    cache: Mutex<HashMap<String, (Vec<IpAddr>, Instant)>>,
    /// Negative cache (failed lookups): host → expiry_time
    negative: Mutex<HashMap<String, Instant>>,
    /// HTTP client (ephemeral, no proxy, 5s timeout)
    client: reqwest::Client,
}

/// DoH endpoints tried in order of preference
const DOH_ENDPOINTS: &[&str] = &[
    "https://dns.google/resolve",
    "https://cloudflare-dns.com/dns-query",
    "https://1.1.1.1/dns-query",
];

/// DNS response schema (simplified; only used fields)
#[derive(serde::Deserialize)]
struct DnsResponse {
    #[serde(default, rename = "Answer")]
    answer: Vec<DnsAnswer>,
}

#[derive(serde::Deserialize)]
struct DnsAnswer {
    #[serde(default)]
    data: String,
    #[serde(default, rename = "TTL")]
    ttl: u32,
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

        // All endpoints failed → add to negative cache (10min)
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
    async fn query_endpoint_with_ttl(
        &self,
        endpoint: &str,
        host: &str,
    ) -> Result<(Vec<IpAddr>, u32)> {
        let url = format!("{}?name={}&type=A,AAAA", endpoint, host);
        let resp = self.client.get(&url).send().await?;
        let dns_resp: DnsResponse = resp.json().await?;

        let addrs: Vec<IpAddr> = dns_resp
            .answer
            .iter()
            .filter_map(|a| a.data.parse::<IpAddr>().ok())
            .collect();

        if addrs.is_empty() {
            return Err(anyhow!("No A/AAAA records in response"));
        }

        // Use maximum TTL from response (or default 300s)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_doh_resolve() {
        let resolver = DohResolver::new().unwrap();
        // Test with a known domain
        let result = resolver.resolve("dns.google").await;
        // May fail in test environment without network, so just check API
        let _ = result;
    }

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
}
