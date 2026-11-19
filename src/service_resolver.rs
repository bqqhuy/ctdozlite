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
    MediaStore,
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
            "media_store" | "media_store_send2me" => Self::MediaStore,
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
    QueueWait,
    Policy,
    Attempt,
    BodyAck,
    TotalIntent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptResult {
    Success,
    HttpFailure,
    AuthFailure,
    PolicyRejected,
    RateLimited,
    ProtocolFailure,
    ConnectFailure,
    Timeout,
    TransportFailure,
    PreviewReady,
    PreviewCacheHit,
    PreviewFallback,
    BudgetExceeded,
}

/// Text mutations use a shared finite deadline; media uploads retain their own budgets.
/// The 10s request ceiling follows PC default P03. Loopback checks exercise
/// budget enforcement only; no live latency tuning or acceptance is claimed.
pub(crate) const TEXT_REQUEST_BUDGET: std::time::Duration = std::time::Duration::from_secs(10);
pub(crate) const TEXT_TOTAL_BUDGET: std::time::Duration = std::time::Duration::from_secs(20);
pub(crate) const QUEUE_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);
pub(crate) const POLICY_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);
const SEND_TIMING_LIMIT: usize = 256;

// The bounded-ring stress fixture intentionally evicts 300 samples. Keep its
// writes separate from fixtures asserting intermediate phase retention.
#[cfg(test)]
pub(crate) static TIMING_FIXTURE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Bounded, process-local diagnostics: no destinations, text, links, request
/// parameters or raw client IDs. Runtime is an ephemeral session identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendPhaseTiming {
    pub intent_tag: String,
    pub retry_count: u32,
    pub phase: TimingPhase,
    pub service: ServiceKey,
    pub candidate_index: Option<u8>,
    pub elapsed_ms: u32,
    pub budget_ms: Option<u32>,
    pub cache_hit: Option<bool>,
    pub result: AttemptResult,
}

#[derive(Clone)]
pub(crate) struct SendTimingContext {
    runtime: usize,
    tag: Option<String>,
    retries: u32,
    service: ServiceKey,
    text_deadline: Option<std::time::Instant>,
}

tokio::task_local! { static SEND_TIMING: SendTimingContext; }
type PhaseRing = std::collections::VecDeque<(usize, SendPhaseTiming)>;
fn phase_ring() -> &'static std::sync::Mutex<PhaseRing> {
    static RING: std::sync::OnceLock<std::sync::Mutex<PhaseRing>> = std::sync::OnceLock::new();
    RING.get_or_init(Default::default)
}

pub(crate) fn intent_tag(client_id: &str) -> Option<String> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    static SALT: std::sync::OnceLock<Option<[u8; 32]>> = std::sync::OnceLock::new();
    let salt = SALT
        .get_or_init(|| {
            let mut bytes = [0u8; 32];
            crate::random::fill(&mut bytes).ok().map(|()| bytes)
        })
        .as_ref()?;
    if client_id.is_empty() {
        return None;
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(salt).ok()?;
    mac.update(client_id.as_bytes());
    Some(
        mac.finalize().into_bytes()[..16]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}

impl SendTimingContext {
    #[cfg(test)]
    pub(crate) fn new(
        runtime: usize,
        client_id: &str,
        attempt: u32,
        group: bool,
        text: bool,
    ) -> Self {
        Self::from_enqueue(
            runtime,
            client_id,
            attempt,
            group,
            text,
            std::time::Instant::now(),
        )
    }
    pub(crate) fn from_enqueue(
        runtime: usize,
        client_id: &str,
        attempt: u32,
        group: bool,
        text: bool,
        enqueued: std::time::Instant,
    ) -> Self {
        Self {
            runtime,
            tag: intent_tag(client_id),
            retries: attempt.saturating_sub(1).min(255),
            service: if group {
                ServiceKey::Group
            } else {
                ServiceKey::Chat
            },
            text_deadline: text.then(|| enqueued + TEXT_TOTAL_BUDGET),
        }
    }
}

pub(crate) async fn with_send_timing<T>(
    context: SendTimingContext,
    work: impl std::future::Future<Output = T>,
) -> T {
    SEND_TIMING.scope(context, work).await
}

/// Applies only inside the outgoing text intent's task-local scope. A caller
/// shares this deadline across all failover candidates and response-body reads.
pub(crate) fn current_text_request_budget() -> Option<std::time::Duration> {
    SEND_TIMING
        .try_with(|context| {
            context.text_deadline.map(|deadline| {
                deadline
                    .saturating_duration_since(std::time::Instant::now())
                    .min(TEXT_REQUEST_BUDGET)
            })
        })
        .ok()
        .flatten()
}

fn phase_budget(phase: TimingPhase, text: bool) -> Option<u32> {
    let duration = match phase {
        TimingPhase::QueueWait => QUEUE_BUDGET,
        TimingPhase::Policy => POLICY_BUDGET,
        TimingPhase::LinkPreview => std::time::Duration::from_secs(2),
        TimingPhase::HttpRequest | TimingPhase::Attempt | TimingPhase::BodyAck if text => {
            TEXT_REQUEST_BUDGET
        }
        TimingPhase::TotalIntent if text => TEXT_TOTAL_BUDGET,
        _ => return None,
    };
    Some(duration.as_millis().min(u128::from(u32::MAX)) as u32)
}

fn store_phase(
    context: &SendTimingContext,
    phase: TimingPhase,
    service: ServiceKey,
    candidate_index: Option<u8>,
    elapsed: std::time::Duration,
    result: AttemptResult,
    cache_hit: Option<bool>,
) {
    let Some(tag) = &context.tag else {
        return;
    };
    let sample = SendPhaseTiming {
        intent_tag: tag.clone(),
        retry_count: context.retries,
        phase,
        service,
        candidate_index,
        elapsed_ms: elapsed.as_millis().min(u128::from(u32::MAX)) as u32,
        budget_ms: phase_budget(phase, context.text_deadline.is_some()),
        cache_hit,
        result,
    };
    let mut ring = phase_ring().lock().unwrap_or_else(|e| e.into_inner());
    while ring.len() >= SEND_TIMING_LIMIT {
        ring.pop_front();
    }
    ring.push_back((context.runtime, sample));
}

pub(crate) fn record_current_phase(
    phase: TimingPhase,
    elapsed: std::time::Duration,
    result: AttemptResult,
) {
    let _ = SEND_TIMING.try_with(|context| {
        store_phase(context, phase, context.service, None, elapsed, result, None)
    });
}

/// Records an interrupted await even when the outer shared deadline drops it.
pub(crate) struct PhaseGuard {
    context: Option<SendTimingContext>,
    phase: TimingPhase,
    started: std::time::Instant,
    finished: bool,
}
impl PhaseGuard {
    pub(crate) fn new(phase: TimingPhase) -> Self {
        Self {
            context: SEND_TIMING.try_with(Clone::clone).ok(),
            phase,
            started: std::time::Instant::now(),
            finished: false,
        }
    }
    pub(crate) fn finish(mut self, result: AttemptResult) {
        if let Some(context) = &self.context {
            store_phase(
                context,
                self.phase,
                context.service,
                None,
                self.started.elapsed(),
                result,
                None,
            );
        }
        self.finished = true;
    }
}
impl Drop for PhaseGuard {
    fn drop(&mut self) {
        if !self.finished {
            if let Some(context) = &self.context {
                store_phase(
                    context,
                    self.phase,
                    context.service,
                    None,
                    self.started.elapsed(),
                    AttemptResult::BudgetExceeded,
                    None,
                );
            }
        }
    }
}
pub(crate) fn record_current_request(
    phase: TimingPhase,
    service: ServiceKey,
    candidate_index: Option<u8>,
    elapsed: std::time::Duration,
    result: AttemptResult,
) {
    let _ = SEND_TIMING.try_with(|context| {
        store_phase(
            context,
            phase,
            service,
            candidate_index,
            elapsed,
            result,
            None,
        )
    });
}
pub(crate) fn record_current_cache_status(phase: TimingPhase, hit: bool) {
    let _ = SEND_TIMING.try_with(|context| {
        store_phase(
            context,
            phase,
            context.service,
            None,
            std::time::Duration::ZERO,
            AttemptResult::Success,
            Some(hit),
        )
    });
}
/// Diagnostics for one live session; older sessions never appear in this view.
pub(crate) fn send_phase_timings(runtime: usize) -> Vec<SendPhaseTiming> {
    phase_ring()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .filter(|(owner, _)| *owner == runtime)
        .map(|(_, sample)| sample.clone())
        .collect()
}
pub(crate) fn record_queue_phase(context: &SendTimingContext, elapsed: std::time::Duration) {
    store_phase(
        context,
        TimingPhase::QueueWait,
        context.service,
        None,
        elapsed,
        if elapsed > QUEUE_BUDGET {
            AttemptResult::BudgetExceeded
        } else {
            AttemptResult::Success
        },
        None,
    );
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
        for key in ["media_store", "media_store_send2me"] {
            let mut candidates = domains
                .service_fallbacks
                .get(key)
                .into_iter()
                .flatten()
                .filter(|host| crate::session::trusted_service_url(host))
                .cloned()
                .collect::<Vec<_>>();
            candidates.truncate(crate::session::MAX_SERVICE_CANDIDATES);
            if !candidates.is_empty()
                && (reset || self.service_endpoints.get(key) != Some(&candidates))
            {
                self.register_service(key, candidates);
            }
        }
    }

    pub(crate) fn candidates_for_url(&self, url: &str) -> Option<(String, Vec<String>)> {
        for (key, hosts) in &self.service_endpoints {
            let canonical_path =
                crate::endpoints::service(key).and_then(|canonical| url.strip_prefix(canonical));
            let path =
                canonical_path.or_else(|| hosts.iter().find_map(|host| url.strip_prefix(host)));
            let Some(path) = path else {
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

#[cfg(test)]
mod send_phase_timing_tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn phase_diagnostics_are_bounded_salted_and_session_scoped() {
        let _fixture = TIMING_FIXTURE.lock().await;
        let runtime = usize::MAX - 741;
        let context = SendTimingContext::new(runtime, "fixture-client-id-never-log", 3, true, true);
        with_send_timing(context, async {
            record_current_cache_status(TimingPhase::Policy, true);
            for _ in 0..300 {
                record_current_phase(
                    TimingPhase::Attempt,
                    Duration::from_millis(3),
                    AttemptResult::Success,
                );
            }
        })
        .await;
        let samples = send_phase_timings(runtime);
        assert!(!samples.is_empty() && samples.len() <= SEND_TIMING_LIMIT);
        assert!(samples.iter().all(|s| s.retry_count == 2
            && s.intent_tag.len() == 32
            && s.service == ServiceKey::Group));
        assert!(!format!("{samples:?}").contains("fixture-client-id-never-log"));
        assert!(send_phase_timings(usize::MAX - 742).is_empty());
        assert!(
            current_text_request_budget().is_none(),
            "task-local deadline must end with the intent"
        );
    }

    #[tokio::test]
    async fn text_total_deadline_includes_time_already_spent_in_command_queue() {
        let _fixture = TIMING_FIXTURE.lock().await;
        let enqueued = Instant::now() - Duration::from_secs(4);
        let context = SendTimingContext::from_enqueue(
            usize::MAX - 746,
            "fixture-queued",
            1,
            false,
            true,
            enqueued,
        );
        let remaining = context
            .text_deadline
            .unwrap()
            .saturating_duration_since(Instant::now());
        assert!(remaining <= Duration::from_secs(16) && remaining > Duration::from_secs(15));
        with_send_timing(context, async {
            record_current_phase(
                TimingPhase::TotalIntent,
                enqueued.elapsed(),
                AttemptResult::Success,
            );
        })
        .await;
        assert!(send_phase_timings(usize::MAX - 746)
            .iter()
            .any(|sample| sample.phase == TimingPhase::TotalIntent && sample.elapsed_ms >= 4000));
    }

    #[tokio::test]
    async fn text_deadline_is_shared_and_upload_context_has_no_text_deadline() {
        let mut context = SendTimingContext::new(usize::MAX - 743, "fixture-text", 1, false, true);
        context.text_deadline = Some(Instant::now() + Duration::from_millis(80));
        with_send_timing(context, async {
            let first = current_text_request_budget().unwrap();
            tokio::time::sleep(Duration::from_millis(5)).await;
            let next = current_text_request_budget().unwrap();
            assert!(next < first && next <= Duration::from_millis(80));
        })
        .await;
        with_send_timing(
            SendTimingContext::new(usize::MAX - 744, "fixture-upload", 1, false, false),
            async {
                assert!(
                    current_text_request_budget().is_none(),
                    "upload timeout must not inherit text ceiling"
                );
            },
        )
        .await;
    }

    #[tokio::test]
    async fn loopback_text_budget_smoke_preserves_success_for_short_requests() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for _ in 0..20 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request).unwrap();
                let body = b"{\"error_code\":0,\"data\":{\"msgId\":\"fixture-confirmed\"}}";
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(body).unwrap();
            }
        });
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        eprintln!("loopback text budget smoke (not live latency tuning): OS={}, architecture={}, dataset=20 fake HTTP confirmations",std::env::consts::OS,std::env::consts::ARCH);
        eprintln!("task,iterations,min_ms,avg_ms,p50_ms,p95_ms,p99_ms");
        for (task, budget) in [
            ("budget_smoke_35s", Duration::from_secs(35)),
            ("budget_smoke_10s", TEXT_REQUEST_BUDGET),
        ] {
            let mut samples = Vec::new();
            for _ in 0..10 {
                let started = Instant::now();
                let result = tokio::time::timeout(budget, async {
                    client
                        .post(format!("http://{address}/fixture"))
                        .body("fake")
                        .send()
                        .await
                        .unwrap()
                        .json::<serde_json::Value>()
                        .await
                        .unwrap()
                })
                .await
                .unwrap();
                assert_eq!(result["data"]["msgId"], "fixture-confirmed");
                samples.push(started.elapsed().as_secs_f64() * 1000.0);
            }
            samples.sort_by(f64::total_cmp);
            eprintln!(
                "{task},10,{:.3},{:.3},{:.3},{:.3},{:.3}",
                samples[0],
                samples.iter().sum::<f64>() / 10.0,
                samples[4],
                samples[8],
                samples[9]
            );
        }
        server.join().unwrap();
    }

    #[tokio::test]
    async fn loopback_text_body_stall_exhausts_one_deadline_without_replay() {
        let _fixture = TIMING_FIXTURE.lock().await;
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let received = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = received.clone();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request).unwrap();
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n")
                .unwrap();
            std::thread::sleep(Duration::from_millis(180));
        });
        let mut session = crate::session::Session::new(false, false, false).unwrap();
        session.http = reqwest::Client::builder().no_proxy().build().unwrap();
        session
            .service_resolver
            .write()
            .unwrap()
            .register_service("chat", vec![format!("http://{address}")]);
        let mut context =
            SendTimingContext::new(usize::MAX - 745, "fixture-loopback", 1, false, true);
        context.text_deadline = Some(Instant::now() + Duration::from_millis(70));
        with_send_timing(context, async {
            let started = Instant::now();
            let logical = session.resolve_service_url("chat", "/api/fixture");
            let result = session.post_form(&logical, "payload=fixture".into()).await;
            assert!(
                result.is_err(),
                "headers must not reset the production Session body deadline"
            );
            assert!(started.elapsed() < Duration::from_secs(1));
        })
        .await;
        server.join().unwrap();
        assert_eq!(received.load(std::sync::atomic::Ordering::SeqCst), 1);
        let samples = send_phase_timings(usize::MAX - 745);
        assert!(samples
            .iter()
            .any(|s| s.phase == TimingPhase::BodyAck && s.result == AttemptResult::BudgetExceeded));
        assert!(samples
            .iter()
            .any(|s| s.phase == TimingPhase::Attempt && s.result == AttemptResult::Success));
        assert!(samples.iter().any(|s| s.phase == TimingPhase::HttpRequest
            && s.result == AttemptResult::Timeout
            && s.service == ServiceKey::Chat
            && s.candidate_index == Some(0)));
        assert_eq!(session.request_attempt_timings().len(), 1);
        assert_eq!(
            session.request_attempt_timings()[0].result,
            AttemptResult::Timeout
        );
    }
}
