//! DNS-over-HTTPS fallback resolver for CDN media domains.
//!
//! When system DNS fails to resolve `*.zadn.vn` / `*.zdn.vn` domains (common in
//! enterprise proxies and some ISPs), this module performs an HTTPS DNS lookup
//! against one of four public DoH endpoints and caches the answer with the
//! server-supplied TTL (clamped to [60, 600] seconds).
//!
//! Design mirrors Olaz `DoHResolver.swift` (GPL-3.0, reference only; no code copied):
//! * Ephemeral HTTP client — no proxy, no cookie jar, 5-second connect + 8-second
//!   read timeout, no redirect following.
//! * TTL-aware positive cache and 300-second negative cache (NXDOMAIN / error).
//! * In-flight request dedup: only one DoH lookup per hostname at a time.
//! * Four endpoints tried in order; first success wins.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::sync::watch;

const TTL_MIN: u64 = 60;
const TTL_MAX: u64 = 600;
/// Used when the DoH response omits the TTL field.
const TTL_DEFAULT: u64 = 60;
const NEGATIVE_TTL: u64 = 300;
/// Upper bound on cached hostnames.
const CACHE_MAX_ENTRIES: usize = 256;

// DoH endpoints in priority order (matching Olaz / canonical preference).
const ENDPOINTS: &[&str] = &[
    "https://dns.google/resolve",
    "https://cloudflare-dns.com/dns-query",
    "https://8.8.8.8/resolve",
    "https://1.1.1.1/dns-query",
];

#[derive(Clone)]
struct CacheEntry {
    addrs: Vec<IpAddr>,
    expires: Instant,
}

/// Result payload published to in-flight waiters: `None` = not yet resolved.
type ResolveResult = Option<Result<(Vec<IpAddr>, u64), String>>;

/// Shared DoH resolver.  Create once via [`DohResolver::new`] and share the
/// value where needed; the inner state is reference-counted.
#[derive(Clone)]
pub struct DohResolver {
    inner: Arc<Inner>,
}

struct Inner {
    positive: Mutex<HashMap<String, CacheEntry>>,
    negative: Mutex<HashMap<String, Instant>>,
    // Per-hostname in-flight watches.  A waiter receives the result through the
    // watch and the owner clears the slot after publishing.  The owner id
    // guards against a stale owner evicting a newer lookup for the same host.
    inflight: Mutex<HashMap<String, (u64, watch::Receiver<ResolveResult>)>>,
    // Distinguishes owners of an in-flight lookup.
    inflight_owner: AtomicU64,
    client: reqwest::Client,
}

impl DohResolver {
    /// Build a resolver with an ephemeral, proxy-free HTTP client.
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .user_agent("ctdozlite/doh")
            .build()
            .context("Không tạo được DoH client")?;
        Ok(Self {
            inner: Arc::new(Inner {
                positive: Mutex::new(HashMap::new()),
                negative: Mutex::new(HashMap::new()),
                inflight: Mutex::new(HashMap::new()),
                inflight_owner: AtomicU64::new(0),
                client,
            }),
        })
    }

    /// Resolve `host` (A record) returning a non-empty list of IPv4/IPv6 addresses.
    pub async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>> {
        self.resolve_with_ttl(host).await.map(|(addrs, _)| addrs)
    }

    /// Resolve `host` and also report the TTL of the answer, so the caller can
    /// bound how long it pins the returned addresses.
    pub async fn resolve_with_ttl(&self, host: &str) -> Result<(Vec<IpAddr>, u64)> {
        let now = Instant::now();

        // Drop stale entries so the cache stays bounded over a long session.
        self.purge_expired();

        // Positive cache hit.
        if let Some(entry) = self.inner.positive.lock().unwrap().get(host) {
            if entry.expires > now {
                let ttl = entry.expires.saturating_duration_since(now).as_secs();
                return Ok((entry.addrs.clone(), ttl));
            }
        }

        // Negative cache hit.
        if let Some(&exp) = self.inner.negative.lock().unwrap().get(host) {
            if exp > now {
                anyhow::bail!("DoH: NXDOMAIN cached for {host}");
            }
        }

        // Claim ownership of the lookup, or join an existing one.  The check
        // and the insert happen under a single lock acquisition: splitting them
        // would let two tasks both believe they own the lookup and both hit the
        // network.
        let owner_id = self.inner.inflight_owner.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = watch::channel(None::<Result<(Vec<IpAddr>, u64), String>>);
        let existing_rx = {
            let mut inflight = self.inner.inflight.lock().unwrap();
            match inflight.get(host) {
                Some((_, rx)) => Some(rx.clone()),
                None => {
                    inflight.insert(host.to_owned(), (owner_id, rx.clone()));
                    None
                }
            }
        };

        if let Some(mut rx) = existing_rx {
            // Wait for the in-flight request to publish its result.
            loop {
                if rx.borrow().is_some() {
                    break;
                }
                if rx.changed().await.is_err() {
                    break;
                }
            }
            let result = rx.borrow().clone();
            return match result {
                Some(Ok(addrs)) => Ok(addrs),
                Some(Err(e)) => anyhow::bail!("DoH: {e}"),
                None => anyhow::bail!("DoH: in-flight request dropped for {host}"),
            };
        }
        // We are the owner of this lookup.
        let result = self.do_lookup(host).await;
        let now2 = Instant::now();

        // Update caches.
        match &result {
            Ok((addrs, ttl)) => {
                let mut positive = self.inner.positive.lock().unwrap();
                // Keep the cache bounded: a long-running client must not
                // accumulate entries for every hostname it ever resolved.
                if positive.len() >= CACHE_MAX_ENTRIES {
                    let now = Instant::now();
                    positive.retain(|_, e| e.expires > now);
                    // Still full after dropping the expired ones — evict the
                    // entry that expires soonest.
                    if positive.len() >= CACHE_MAX_ENTRIES {
                        if let Some(oldest) = positive
                            .iter()
                            .min_by_key(|(_, e)| e.expires)
                            .map(|(host, _)| host.clone())
                        {
                            positive.remove(&oldest);
                        }
                    }
                }
                positive.insert(
                    host.to_owned(),
                    CacheEntry {
                        addrs: addrs.clone(),
                        expires: now2 + Duration::from_secs(*ttl),
                    },
                );
            }
            Err(_) => {
                let mut negative = self.inner.negative.lock().unwrap();
                let now = Instant::now();
                negative.retain(|_, exp| *exp > now);
                if negative.len() >= CACHE_MAX_ENTRIES {
                    if let Some(oldest) = negative
                        .iter()
                        .min_by_key(|(_, exp)| **exp)
                        .map(|(host, _)| host.clone())
                    {
                        negative.remove(&oldest);
                    }
                }
                negative.insert(host.to_owned(), now2 + Duration::from_secs(NEGATIVE_TTL));
            }
        }

        // Publish and remove from in-flight map.
        let published = match &result {
            Ok((addrs, ttl)) => Some(Ok((addrs.clone(), *ttl))),
            Err(e) => Some(Err(e.to_string())),
        };
        let _ = tx.send(published);
        // Only clear the slot if it is still ours — a later lookup for the same
        // host may already have replaced it.
        {
            let mut inflight = self.inner.inflight.lock().unwrap();
            if inflight.get(host).is_some_and(|(id, _)| *id == owner_id) {
                inflight.remove(host);
            }
        }

        result
    }

    async fn do_lookup(&self, host: &str) -> Result<(Vec<IpAddr>, u64)> {
        for endpoint in ENDPOINTS {
            match self.query_endpoint(endpoint, host).await {
                Ok((addrs, ttl)) if !addrs.is_empty() => return Ok((addrs, ttl)),
                _ => continue,
            }
        }
        anyhow::bail!("DoH: todos os endpoints falharam para {host}");
    }

    async fn query_endpoint(&self, endpoint: &str, host: &str) -> Result<(Vec<IpAddr>, u64)> {
        let url = format!("{endpoint}?name={host}&type=A");
        let resp: DohResponse = self
            .inner
            .client
            .get(&url)
            .header("Accept", "application/dns-json")
            .send()
            .await
            .context("DoH request failed")?
            .error_for_status()
            .context("DoH HTTP error")?
            .json()
            .await
            .context("DoH JSON parse failed")?;

        // Status 3 = NXDOMAIN (authoritative), treat as hard failure.
        if resp.status == 3 {
            anyhow::bail!("DoH: NXDOMAIN for {host}");
        }
        anyhow::ensure!(resp.status == 0, "DoH: non-zero status {} for {host}", resp.status);

        let mut addrs = Vec::new();
        // The cache lifetime is the smallest TTL among the A records — a
        // record may be re-pointed sooner than its siblings.
        let mut min_ttl: Option<u64> = None;
        for answer in resp.answer.unwrap_or_default() {
            // type == 1 → A record (IPv4)
            if answer.r#type == 1 {
                if let Ok(ip) = answer.data.parse::<IpAddr>() {
                    addrs.push(ip);
                }
                if let Some(ttl) = answer.ttl {
                    min_ttl = Some(min_ttl.map_or(ttl, |m: u64| m.min(ttl)));
                }
            }
        }
        // No TTL in the answer → fall back to the documented default.
        let ttl = min_ttl.unwrap_or(TTL_DEFAULT);
        Ok((addrs, ttl.clamp(TTL_MIN, TTL_MAX)))
    }

    /// Evict `host` from both caches (call when a connection attempt fails).
    pub fn invalidate(&self, host: &str) {
        self.inner.positive.lock().unwrap().remove(host);
        self.inner.negative.lock().unwrap().remove(host);
    }

    /// Drop expired entries so a long-running client does not grow the cache
    /// without bound.  Returns the number of entries removed.
    pub fn purge_expired(&self) -> usize {
        let now = Instant::now();
        let mut removed = 0usize;
        {
            let mut positive = self.inner.positive.lock().unwrap();
            let before = positive.len();
            positive.retain(|_, e| e.expires > now);
            removed += before - positive.len();
        }
        {
            let mut negative = self.inner.negative.lock().unwrap();
            let before = negative.len();
            negative.retain(|_, exp| *exp > now);
            removed += before - negative.len();
        }
        removed
    }
}

#[derive(Deserialize)]
struct DohResponse {
    #[serde(rename = "Status")]
    status: u32,
    #[serde(rename = "Answer")]
    answer: Option<Vec<DohAnswer>>,
}

#[derive(Deserialize)]
struct DohAnswer {
    #[serde(rename = "type")]
    r#type: u32,
    data: String,
    /// Record TTL in seconds — used to bound the positive-cache lifetime.
    #[serde(rename = "TTL")]
    ttl: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_entry_expires() {
        let entry = CacheEntry {
            addrs: vec!["1.2.3.4".parse().unwrap()],
            expires: Instant::now() - Duration::from_secs(1),
        };
        assert!(entry.expires < Instant::now());
    }

    #[test]
    fn doh_resolver_constructs_without_error() {
        DohResolver::new().expect("resolver should construct");
    }

    /// A response whose A record carries no TTL must not extend the cache
    /// lifetime to the maximum.
    #[test]
    fn missing_ttl_falls_back_to_the_default() {
        let resp: DohResponse = serde_json::from_str(
            r#"{"Status":0,"Answer":[{"type":1,"data":"1.2.3.4"}]}"#,
        )
        .unwrap();
        let mut addrs = Vec::new();
        let mut min_ttl: Option<u64> = None;
        for answer in resp.answer.unwrap_or_default() {
            if answer.r#type == 1 {
                if let Ok(ip) = answer.data.parse::<IpAddr>() {
                    addrs.push(ip);
                }
                if let Some(ttl) = answer.ttl {
                    min_ttl = Some(min_ttl.map_or(ttl, |m: u64| m.min(ttl)));
                }
            }
        }
        let ttl = min_ttl.unwrap_or(TTL_DEFAULT).clamp(TTL_MIN, TTL_MAX);
        assert_eq!(addrs.len(), 1);
        assert_eq!(ttl, TTL_DEFAULT);
    }

    /// The smallest TTL among the A records bounds the cache lifetime.
    #[test]
    fn the_smallest_answer_ttl_wins() {
        let resp: DohResponse = serde_json::from_str(
            r#"{"Status":0,"Answer":[
                {"type":1,"data":"1.2.3.4","TTL":300},
                {"type":1,"data":"5.6.7.8","TTL":120}
            ]}"#,
        )
        .unwrap();
        let mut min_ttl: Option<u64> = None;
        for answer in resp.answer.unwrap_or_default() {
            if let Some(ttl) = answer.ttl {
                min_ttl = Some(min_ttl.map_or(ttl, |m: u64| m.min(ttl)));
            }
        }
        assert_eq!(min_ttl.unwrap_or(TTL_DEFAULT).clamp(TTL_MIN, TTL_MAX), 120);
    }

    /// An out-of-range TTL is clamped into [TTL_MIN, TTL_MAX].
    #[test]
    fn ttl_is_clamped_to_the_supported_range() {
        assert_eq!(5u64.clamp(TTL_MIN, TTL_MAX), TTL_MIN);
        assert_eq!(100_000u64.clamp(TTL_MIN, TTL_MAX), TTL_MAX);
    }

    /// Expired entries are dropped; live ones are kept.
    #[test]
    fn purge_expired_removes_only_stale_entries() {
        let resolver = DohResolver::new().unwrap();
        {
            let mut positive = resolver.inner.positive.lock().unwrap();
            positive.insert(
                "fresh.invalid".into(),
                CacheEntry {
                    addrs: vec!["1.2.3.4".parse().unwrap()],
                    expires: Instant::now() + Duration::from_secs(60),
                },
            );
            positive.insert(
                "stale.invalid".into(),
                CacheEntry {
                    addrs: vec!["5.6.7.8".parse().unwrap()],
                    expires: Instant::now() - Duration::from_secs(1),
                },
            );
        }
        {
            let mut negative = resolver.inner.negative.lock().unwrap();
            negative.insert(
                "stale.invalid".into(),
                Instant::now() - Duration::from_secs(1),
            );
        }

        assert_eq!(resolver.purge_expired(), 2);
        let positive = resolver.inner.positive.lock().unwrap();
        assert_eq!(positive.len(), 1);
        assert!(positive.contains_key("fresh.invalid"));
        assert!(resolver.inner.negative.lock().unwrap().is_empty());
    }

    /// Invalidation clears both the positive and the negative entry.
    #[test]
    fn invalidate_clears_both_caches() {
        let resolver = DohResolver::new().unwrap();
        resolver.inner.positive.lock().unwrap().insert(
            "h.invalid".into(),
            CacheEntry {
                addrs: vec!["1.2.3.4".parse().unwrap()],
                expires: Instant::now() + Duration::from_secs(60),
            },
        );
        resolver
            .inner
            .negative
            .lock()
            .unwrap()
            .insert("h.invalid".into(), Instant::now() + Duration::from_secs(60));

        resolver.invalidate("h.invalid");
        assert!(resolver.inner.positive.lock().unwrap().is_empty());
        assert!(resolver.inner.negative.lock().unwrap().is_empty());
    }

    /// The in-flight slot is only cleared by the task that owns it.
    #[test]
    fn a_stale_owner_does_not_evict_a_newer_lookup() {
        let resolver = DohResolver::new().unwrap();
        let (tx, rx) = watch::channel(None::<Result<(Vec<IpAddr>, u64), String>>);
        {
            let mut inflight = resolver.inner.inflight.lock().unwrap();
            inflight.insert("h.invalid".into(), (2, rx));
        }
        // Owner 1 is stale and must not remove owner 2's slot.
        let mut inflight = resolver.inner.inflight.lock().unwrap();
        if inflight.get("h.invalid").is_some_and(|(id, _)| *id == 1) {
            inflight.remove("h.invalid");
        }
        assert!(inflight.contains_key("h.invalid"));
        drop(inflight);
        drop(tx);
    }
}
