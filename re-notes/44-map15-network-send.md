# MAP-15 network send reliability implementation notes

Date: 2026-11-18. Scope: `session.rs`, `service_resolver.rs`, `outgoing.rs`.

## Evidence and parity

- E2 PC bundle evidence in `docs/SEND_LATENCY_NETWORK_ANTIGRAVITY_GUIDE_2026-11-18.md`:
  P01–P05 describe the service-domain manager, HTTP failure switching, a default
  2-second send-time preview wait, and queue-level retry configuration. These are
  static bundle observations; they do not establish effective runtime settings or
  guarantee that this fork should replay a mutation.
- Fork source evidence in the same guide: F01–F04 identify canonical-host
  precedence, `Session::send` retry behavior, 10/35-second HTTP timeouts, and
  sequential link metadata parsing/scraping. N01–N02 are machine/time-specific DNS
  and TLS observations; an HTTP 404 at `/` does not validate an API route.
- Parity statement: preserve discovery order for validated service-map hosts, with
  the canonical endpoint as compatibility fallback. Do not infer API validity from
  route existence or root-host reachability. Reads may try a small bounded candidate
  set; mutations remain single-attempt on HTTP or transport uncertainty.

## Implementation

- `ServiceResolver` is synchronized from sanitized `Domains` on service-map
  updates, session restore, and logout/account reset. Discovered hosts retain map
  order, then canonical fallback. The URL builder and ordinary request path use the
  same resolver. Resolver snapshots cap attempts at three candidates.
- Each `Session::send` has a 35-second total attempt budget. Automatic failover is
  restricted to GET on HTTP 404, connect failure, or timeout. POST/other mutation
  failures are returned immediately; there is no blind replay after a response or
  transport error. Voice-call GET endpoints are also excluded because the existing
  call API uses GET for state-changing signals (`call.rs::voicecall_get`). Multipart
  upload keeps its separate timeout path.
- The session retains at most 16 typed timing samples. Each records service key,
  HTTP or preview phase, bounded elapsed milliseconds, candidate index where
  applicable, and a result class. It omits URL, message, account, request body,
  cookies, and identifiers.
- Successful link metadata uses a session-scoped, origin-separated in-memory cache:
  SHA-256 URL key, 32-entry LRU, 10-minute TTL, and a 16 KiB per-entry size cap.
  The source URL is not kept as the lookup key; returned metadata can itself contain
  a URL and is retained only in process memory. Cache state clears on logout/session
  restore and rejects access from a revoked runtime.
  Outgoing send waits at most 2 seconds for preview and then sends the original text
  through its explicit ordinary text endpoint. E2EE text still bypasses previews.

## Verification boundary

- Tests added for discovered route order and replacement, invalid-map retention,
  restore routing, cache bounds/origin isolation, remembered successful candidate,
  and non-replay of a mutation after HTTP 404.
- No live message, real account store, or native Zalo behavior was exercised.
- Composer-time debounce requires the existing GUI/worker preview hook and remains
  a caller integration; send-time caching/budget alone is not that debounce.
- MAP-15 is not complete from these source/fixture checks. Native/live latency,
  effective PC settings, DNS failure behavior across all transport layers, delivery
  reconciliation after ambiguous mutation timeout, and end-to-end composer cache
  behavior remain unknown until separately verified.
