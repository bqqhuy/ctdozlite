# ADR-0036: Explicit E2EE experiment with bounded diagnostics

- **Status:** accepted for experimental testing only
- **Date:** 2026-11-09
- **Decider:** maintainer explicitly requested unverified E2EE trials with redacted logs

## Decision

Unknown — AMBIGUOUS (source: re-notes/36 section 8). The routes exist in the
official index, but the prekey payloads and custom binary envelope remain E3
hypotheses. This does not establish parity with Zalo PC/Web. ADR-0035 remains
the default production boundary. Its prohibition on experimental requests is
amended only for the explicitly requested trial described here.

Both Cargo feature `experimental-e2ee` and launch flag `--e2ee-experimental`
are required. Compilation alone never activates the experiment. The GUI shows
a persistent experimental warning. A separate per-conversation action persists
the encrypted-message requirement before the user sends a test text. Media,
groups, quotes and other unsupported encrypted payloads stay blocked. There is
no plaintext fallback. Actual key fingerprints are still required for safety
number verification; an experimental success is not official interoperability.

The experiment uses the historical inferred POST payload shapes inside the
standard ParamCipher v2/signkey envelope and session cookie path (unlike the
historical adapter's session-AES-only wrapper). Endpoint-tag suffix rules are
inferred for these routes; server acceptance is unknown. A malformed bundle, missing
success code, invalid signed-prekey signature, upload rejection or missing send
receipt is a failure. No upload error is ignored. Crypto/database work for
outgoing sends runs in blocking tasks, with no connection or mutex across await.
Existing incoming parsing remains on the worker.

Crypto schema 7 adds `e2ee_trial_attempts(profile_id,peer_id,client_id)`. An atomic
reservation precedes all trial network work and sets `e2ee_required`. The same
send identity cannot be tried again, including after restart. Even a failure
before transmission consumes the attempt; the user must inspect the conversation
and compose a new test message. No uncertain send is automatically replayed.
The standard session layer may select another service host after a definite 404.
No key/message content is stored in the attempt table. Migration is forward-only;
schema-6 binaries reject schema 7. Back up the entire isolated test profile with
the application closed before testing; never lower `user_version` manually.

## Diagnostics

Use the existing nonblocking channel (256 events) and rotating JSONL writer
(`diagnostics/sync-media.jsonl`, one previous file, approximately 2 x 4 MiB).
Every E2EE event has run/attempt correlation, per-process salted account/peer
tags, version/architecture, experimental state, stage, outcome, elapsed time,
byte counts, safe envelope type/counters when available, numeric server/HTTP
codes and allowlisted failure classification. Response shape is a bit mask:
1 = object, 2 = error_code present, 4 = data object.

No arbitrary error strings, response bodies, message content, ciphertext, raw
keys, account/peer IDs, cookies, tokens, IMEI or URLs enter this schema. The salt
is random and never logged; tags cannot correlate identities across launches.
Raw/dev dump flags cannot be combined with the experimental launch flag.
Diagnostics remain local. Writer failure must not break message processing;
queue overflow emits `diagnostic_dropped` when the writer next receives work.
Abrupt process termination may lose queued final events. A stage identifies the
failed boundary, not necessarily the ultimate cause of an incompatible server
contract; sanitized official captures are still needed to resolve schema gaps.

## Verification limits

Unit/loopback tests cover privacy, request encoding, server error handling,
durable duplicate suppression, account isolation and existing local crypto
regressions. These do not certify Zalo E2EE, server entitlement, delivery to an
official client or Windows ARM64 hardware behavior. No live user messages are
sent by automated verification.
