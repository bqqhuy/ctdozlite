# Claude review follow-up — 2026-11-09

Source baseline: a610cbf plus the uncommitted cache/audio mutex edits.

- Fork deviation because disposable local DNS/client caches must recover after a
  poisoned writer without trusting partially mutated entries. Clear all entries
  before clearing poison. Audio state is not recoverable: report through the
  existing media error path and let StopOnExit stop the media tasks.
- Fork deviation because document previews must read bounded account-local copies
  and use the existing credential-free CDN transport on a cache miss. The cache
  key is unchanged from attachment_cache (length-delimited conversation/message
  SHA-256 plus sanitized filename); document requests now carry ChatImage context.
  Existing allowlist, redirect validation and HTTP status rules apply. No server
  endpoint, signing parameter, frame ID, key schema or cipher flow is changed.
- Unknown — AMBIGUOUS: official PC/Web document-cache implementation and E2EE
  interoperability. These local robustness regressions do not prove parity.
- No TLS pinset is introduced: the report did not provide controlled server keys
  or a secure rotation contract. Existing Rustls/WebPKI verification stays enabled.
- Evidence: direct source tracing and isolated local/loopback regressions; no live
  Zalo capture or real-account call acceptance is claimed.
