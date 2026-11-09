# Independent audit remediation — 2026-11-09

Source baseline: `164af11a0097d929b50d450dcac6d8c500ecbe01`.

- Unknown — AMBIGUOUS: official E2EE containers/prekey schemas remain unresolved
  in re-notes/36 section 8. ADR-0036's explicit experiment remains available.
- Fork deviation because local correctness requires durable authenticated receive
  receipts, low-level send guards and group/direct policy separation (ADR-0037).
  Synthetic replay/rollback tests do not establish official interoperability.
- Friend endpoint consolidation preserves the literal value already present in
  `endpoints::FRIEND`; no method, tag, payload, cipher or signing rule is changed.
- The custom `ZALO_LAN:v1` probe had no official parity evidence or peer responder.
  Its active scanner and parity claims were removed; C03 remains open.
- Windows Hello uses the existing UserConsentVerifier availability API on a
  background thread. Native Hello/device acceptance is not claimed by mocks.
- SQLite/rusqlite are upgraded to bundled libsqlite3-sys 0.39.1 / rusqlite 0.41.1;
  the public API's checked unsigned conversions are retained via fallible_uint.
