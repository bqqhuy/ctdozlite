# v0.007 protocol evidence (2026-11-05)

Sources inspected locally: Zalo PC 26.11.10 `compact-app-pc.5c0110a3a8749b3aacde.js`,
`lazy/main-startup.8b01fad46426bf4cc35b.js`; saved Zalo Web chunk
`1.c72c0957b455ab70b73c.js`. Minified offsets are source-specific, not stable APIs.

- PC compact `8Nax` (~3381650) and `wlxX` (~19992455): TTL is milliseconds;
  base is serverTime/sendDttm. No proof that device receipt time is equivalent.
- Main startup ~2055894: autodelete/blocksharing getConvers and updateConvers.
  Block-sharing snapshot rows use destId/isGroup/isActor/createdAt. TTL update
  uses POST with encoded query; blocksharing update uses encoded request body.
- Group create/v2 and invite/v2 use members/memberTypes arrays; ordinary group
  creation uses groupType:null. Admin add/remove are GET, owner transfer/leave/
  disband are POST. Related groups returns groupRelateds keyed by peer UID.
- PC compact ~1970000: board/list starts at page 1; note tab 1, poll tab 3;
  continuation uses the final item's data.id/poll_id and outer boardType.
  Plain text notes (type 0) are distinct from reminders (type 4).
- Poll create uses GROUP_POLL, array options and the observed option flags;
  vote/add option are GET; end is POST. Anonymous counts do not establish UIDs.
- Reaction removal: rType=-1 and empty rIcon (~18248189, ~19592440).
  Reactor details derive from received per-UID state; no unverified reactor-list
  endpoint is introduced.
- V1 `+Mel` (~671000): controls correlate public_key and pc_name; success info
  correlates public_key. pull_mobile_msg/cancel_pull_mobile_msg are GET.
  znoise (~15137682) maps numeric fids/gids via the service in endpoints.rs.

Desktop V2 format reader selectively adapted from upstream PR #96 (MIT/Apache-2.0).
V1 archive/key behavior also references upstream's history_sync implementation;
real-device cipher/media compatibility remains unverified.

Verification uses loopback/fully synthetic fixtures only. No real credentials,
private keys, URLs with signatures or message content are stored in this note.

Additional regression evidence: an independent OpenSSL AES-CBC vector checks V1's
uppercase-hex key derivation and zero-IV segment reset. This verifies the implemented
primitive contract, not an observed successful iPhone V1 transfer. Python-generated
XZ fixtures test decoder-memory rejection and truncated input. The upstream JXL
fixture originates from Chromium; its provenance and BSD license are retained in
`tests/fixtures/README-jxl.txt` and `LICENSE-jxl`.
