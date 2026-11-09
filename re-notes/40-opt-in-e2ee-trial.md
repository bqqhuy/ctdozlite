# E2EE trial boundary (2026-11-09)

Unknown — AMBIGUOUS. Evidence remains re-notes/36 section 8 and the E2 route
index; no new E1 capture or official schema was obtained. Maintainer explicitly
requested trying the unverified implementation with redacted diagnostic logs.

Fork deviation because the historical inferred prekey POST bodies and custom
v1 binary envelope are available only with Cargo feature `experimental-e2ee`
AND launch flag `--e2ee-experimental`. See ADR-0036 and docs/E2EE_TRIAL.md.
The experiment does not assert official parity and never retries as plaintext.
Its trial test results are local/loopback evidence, not Zalo acceptance.
