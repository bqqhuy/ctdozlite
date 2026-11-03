# Exact send identity and loaded-quote navigation

Selective upstream port, 2026-11-02:

- `ctdoteam1110/ctdozlite@c65e82e3329b1cbe2c5342612a715ecf7ad04408` (#61), including the upstream message-identity integration fixtures and send-variant loopback fixture.
- `ctdoteam1110/ctdozlite@752969644a180975eb8ee7fcb884144b8521a444` (#60), including the pure reference resolver and egui click/scroll fixtures.

The request wire fields/endpoints/cipher path remain the existing ones. The behavioral change is allocating `clientId` once, propagating it through send variants/fallbacks and receipt events, and reconciling only explicit identities. Client IDs remain millisecond-shaped integers with an in-process atomic monotonic allocator. They are not cryptographic material. ID strings are preserved without float conversion. Empty/zero receipt IDs do not fall back to `cliMsgId` as a server ID.

The upstream text/time heuristic is replaced in thread merge, previews, and group history. Ambiguous client keys do not select the first pending message. Direction/sender mismatches do not merge; two outgoing self representations (live sender 0 versus resolved phone self) are account-scoped aliases. Keep this fork's native call records as local-only records rather than fake send acknowledgements.

Additional fork storage boundary: under the existing snapshot transaction, query at most two candidates using schema-4 `idx_message_client`. Promote a unique pending key to its server ID and server clock; a late pending snapshot maps to the existing canonical row. Preserve local flags/recall and roll back promotion with a failed batch. No bulk migration or cleanup by names/content occurs.

Quote navigation only examines the current resident message list. Canonical, client and wire IDs participate without namespace preference; a cross-namespace collision is ambiguous. Preserve leading zeroes and exact large identifiers. The quote action widens rendering only, performs no I/O, highlights the exact row, and retains the older scroll position after the highlight expires. Missing/ambiguous references produce a user notice instead of selecting a similar text.

The native screenshot fixture pre-seeds a completed quote-click action; headless egui tests independently exercise actual pointer clicks on incoming/outgoing quote blocks and animated scroll/highlight behavior. No fresh authenticated capture or real sending was performed. Real server/phone interoperability and physical-device behavior remain manual checks.
