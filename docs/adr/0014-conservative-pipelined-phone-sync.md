# ADR-0014: Bounded phone sync with independent download/import progress

- Status: superseded by ADR-0017
- Date: 2026-11-02
- Supersedes: ADR-0010 default request grouping and sequential download/import.

## Context

The maintainer reported interrupted transfers and approved conservative partition grouping, continuous downloads alongside ordered database imports, and two independent progress lines. Static inspection of the uploaded Zalo PC 26.11.10.3400 confirms distinct metadata/message containers and scoped final markers; it does not establish an account-ban threshold or a universal server partition limit.

## Decision

Use at most 30 partitions per request by default, respecting smaller learned limits and the existing payload budget. Only explicit, recognized size-limit responses permit bounded splitting. No speculative retries or parallel phone-initiation requests. Preserve the existing session identity, routing quarantine and range checkpoints.

A bounded producer prepares rounds while the consumer imports. Two credential-free downloads stream encrypted containers to private staging files; a queue of two applies backpressure. Metadata remains ordered before dependent message containers. One blocking writer owns the SQLite connection, with ordered, authenticated decoding outside the GUI thread. Download byte progress and import progress occupy a fixed-height panel. A scoped final marker completes only its partition, including embedded-metadata containers. Failed/rejected rounds never advance their checkpoint.

## Validation and limits

Synthetic tests cover foreign sessions, missing final markers, scoped completion and server rejection. Real phone Android/iOS compatibility requires device testing; no claim about server limits follows from these tests. Each encrypted/decoded container retains a 64 MiB defensive budget; this is separate from attachment uploads. Containers begin downloading once their round's control descriptors are complete.

## Rollback

Revert this implementation and retain existing database schema/checkpoint tables. Interrupted ranges replay idempotently. No destructive history migration is introduced.
