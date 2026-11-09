# ADR-0035: Fail closed at the unverified E2EE server boundary

- **Status:** accepted
- **Date:** 2026-11-09
- **Deciders:** maintainer request to remediate the independent audit

## Context

`re-notes/36-zalo-e2ee-signal-protocol-analysis.md` section 8 explicitly leaves
the official ciphertext container and prekey request/response contract unknown.
The route index is E2 evidence of route existence, not a server schema. The
previous custom binary envelope and inferred HTTP payloads therefore cannot
establish Zalo PC/Web interoperability.

## Decision

Amended by ADR-0036 for maintainer-authorized, double-opt-in experimental
testing only; the default production decision below remains unchanged.

Unknown — AMBIGUOUS: refuse production encrypted sends, prekey upload/fetch,
decryption with the custom envelope, and safety-number verification without
emitting a speculative server request or downgrading to plaintext. Retain the
local synthetic ratchet implementation for tests. An observed encrypted-message
requirement survives independently of a decodable local session. Opening this
boundary requires recorded official contract evidence and interoperable fixtures;
changing the error guard alone is insufficient.

Local state fixes use atomic SQLite transactions, monotonically allocated prekey
IDs and authenticated continuation of repeated prekey envelopes. These are local
correctness tests, not evidence of the official transport contract. Legacy fake
identity placeholders are removed by a forward-only migration.

## Consequences

- Users receive an explicit unavailable-contract error instead of false security
  claims or potentially undecryptable ciphertext.
- Existing encrypted threads cannot silently fall back to ordinary sends.
- Native-device acceptance and Zalo interoperability remain unconfirmed.

## Alternatives considered

| Option | Reason rejected |
| :--- | :--- |
| Keep sending the inferred wrapper | E3 cannot justify a server contract. |
| Retry with plaintext | Violates the encrypted-message requirement. |
| Delete local keys and sessions | Destroys user state without resolving the contract. |
