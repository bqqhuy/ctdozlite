# MAP-15 — routing, bounded sends and delivery reconciliation

Updated 2026-11-18. Source/fixture evidence only; no live/native acceptance.

## Evidence and parity

Parity with Zalo PC/Web (source: docs/SEND_LATENCY_NETWORK_ANTIGRAVITY_GUIDE_2026-11-18.md, P01–P05): validated service-domain discovery, preview cache and default 2s preview wait. PC 26.11.12 bundle is E2 static evidence, not proof of effective runtime configuration, DNS failover or real-account delivery. Root-path HTTP 404 does not validate a send endpoint.

Fork deviation because replay after an ambiguous response can duplicate messages: mutations replay only for typed DNS preflight failure or connection refusal before sending. HTTP failure, body stall/reset and timeout never authorize replay. State-changing call GETs are excluded from read failover. Effective PC settings, live server acceptance and before/after latency remain UNKNOWN.

## Implemented path

- URL builders retain canonical logical service URLs; Session::send alone selects discovered/remembered candidates followed by canonical compatibility fallback. Snapshots cap attempts at three candidates. Restore, map replacement and account reset synchronize resolver state. Both clients use typed DNS and disable redirects.
- Bounded read failover; mutation replay only in definite pre-send cases above. Typed auth/policy/rate-limit/protocol errors include nonzero HTTP-200 envelopes and nested error codes. Error chains survive worker boundaries.
- WorkerSender normalizes one durable identity before enqueue. Definitely unsent errors become Failed; ambiguous failures and restored Pending become DeliveryUnknown. Late authoritative ACK/echo reconciles without duplication or Sent regression.
- Text budgets: queue5s, policy5s, preview2s, request at most10s, total20s from enqueue INCLUDING queue wait. These bounded fork choices are fixture checked, not live-tuned. Upload/media retain separate timeouts.
- Composer preview is debounced/thread-scoped; worker E2EE policy precedes preview/cache and fails closed for required/unknown/error. Send-time preview shares one2s budget, then ordinary text fallback preserves mentions/quote without a second preview.
- Preview cache: session/origin scoped,32 entries,10min TTL, SHA-256 URL lookup,16KiB metadata cap; revoked on logout/restore.

## Sanitized timing evidence

Session::send_phase_timings exposes a bounded256-sample account/runtime-scoped ring. Fields: process-salted HMAC intent tag, phase, service class, candidate index, elapsed/budget milliseconds, retry count, cache hit, result. No raw ID/account/destination/message/URL/cookie/key.

Phases: QueueWait, Policy, LinkPreview, Attempt, BodyAck, HttpRequest, TotalIntent. Cancellation-safe guards record interrupted attempt/body phases; policy snapshot cache hits are recorded. HTTP result classes distinguish auth/policy/rate-limit/protocol/connect/timeout/transport.

## Verification boundary

Actual Session fixtures cover DNS preflight, HTTP401/403/404/429/500/503, nonzero envelopes, reset after headers and body stall. Body-stall fixture uses Session::post_form, observes production timing samples and asserts exactly one mutation. Other regressions cover restore/account routing, remembered candidates, cache scope, preview fallback, queue-inclusive deadline, identity and late-echo reconciliation. Twenty-request budget smoke is NOT live latency improvement evidence.

Final commands/audits: docs/PHASE_3_4_EXECUTION_2026-11-18.md. No real account mutation, DNS/hosts-file edit, binary/release build or native interoperability test.
