# ADR-0038: Identity-bound verification in the local E2EE trial

- **Status:** accepted for local correctness; official Zalo interoperability unverified
- **Date:** 2026-11-16
- **Decider:** maintainer authorization to remediate audit findings F13 and F19

## Context

The local model signs prekeys with the Curve25519 DH identity using XEdDSA.
Its exported signing hint repeats that identity. The verifier previously let an
independent caller-supplied signing key authenticate a different DH identity.
The previous display fingerprint hashed only a pair of keys, despite claiming
the Signal display scheme. Neither assumption establishes an official Zalo contract.

## Decision

Fork deviation for fail-closed local validation: only signatures verifiable
under the DH identity are accepted by the synthetic bundle verifier. A distinct
signing hint without an authenticated binding is unsupported and rejected. This
is not a general assertion that all signing and DH keys must be identical; a
future separate-key scheme needs evidence and its own binding validation. The
legacy independent Ed25519 fallback is removed. Existing XEdDSA raw/DJB input
compatibility remains; server request schemas and default trial gates do not change.

The display fingerprint is independently implemented from the Signal algorithm
and public known-answer data. Each party's stable identifier and DJB-serialized
key contribute to its fingerprint; the display combines the two party values.
Production manager calls require both account and peer identifiers. Key-only
compatibility display helpers remain explicitly unsuitable for peer verification.
No libsignal implementation is copied or added as a dependency.

Crypto schema 9 atomically resets old `verified` flags and timestamps once,
without removing identities, sessions, prekeys or encrypted receipts. A key-only
verification must not silently become identifier-bound verification. Failed
migration leaves the old version and flags unchanged. Schema-8 binaries refuse
schema 9; never manually lower a live database's version.

## Evidence and acceptance

- Local source and failing scenarios: audit F13/F19 and `re-notes/43-audit21-remediation.md`.
- Signal algorithm/public vector: <https://github.com/signalapp/libsignal/blob/main/rust/protocol/src/fingerprint.rs>.
- Existing Zalo boundary: ADR-0035, ADR-0036, ADR-0037 and re-notes/36 section 8.
- Tests must cover DH-key substitution without state mutation, valid bound bundles,
  identifier changes/symmetry, the independent display vector, and atomic schema upgrade.

Unknown — AMBIGUOUS for official Zalo identifier semantics and server parity.
Passing the Signal vector proves this display algorithm's scope only. Native
Zalo verification and full E2EE interoperability remain separate acceptance work.
Before device trials, back up the isolated test profile with the application closed.
Rollback means restoring a compatible backup/build, not deleting keys or downgrading schema.
