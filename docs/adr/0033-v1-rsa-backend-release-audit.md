# ADR-0033: Replace V1 RSA backend after release audit

- **Status:** accepted
- **Date:** 2026-11-05
- **Deciders:** repository maintainer, implementation assistant

## Context

Release preparation found RUSTSEC-2023-0071 affecting the rsa 0.10.0-rc.20 dependency
selected in ADR-0031. A prerelease pin did not remove this advisory. The user's release
request authorizes necessary fixes, not silently ignoring a known vulnerability.

## Decision

Supersede only ADR-0031's RSA dependency selection with aws-lc-rs 1.19.0. Generate
RSA-2048 on the blocking worker, export X.509 SPKI DER and unwrap the phone key using
PKCS#1 v1.5 as required by the existing V1 protocol. Private material remains owned
by the task/backend and is not persisted. Unwrapped bytes use zeroizing storage.
No endpoint, padding mode, AES derivation, session matching or Sync2 behavior changes.
Remove the rsa and direct rand dependencies. Update the yanked clipboard-win
transitive dependency from 5.4.1 to 5.4.3 within arboard's compatible version range.

## Evidence and verification

An independent OpenSSL throwaway key, SPKI and wrapped fixed plaintext verify public
key encoding/decryption; another generated private key must reject the ciphertext.
The existing AES known-answer and XZ memory-limit tests remain. Full Linux test and
Clippy checks are run; native ARM64 CI is required before attaching release assets.
The deliberate public test key is documented and never used by runtime code.

cargo audit reports zero vulnerabilities after the change. Maintenance warnings
remain in pre-existing audio/font/UI dependencies; the glib unsoundness warning is
not in the Windows ARM64 dependency graph. Registry yank checking timed out, so the
completed security audit uses --no-yanked and this limitation is disclosed.

## Consequences

AWS-LC adds a native build dependency, validated by Windows ARM64 CI rather than
assuming compatibility from Linux. This does not prove iPhone V1 compatibility or
complete BinNet decoding. Keep the explicit experimental label and release limitations.
