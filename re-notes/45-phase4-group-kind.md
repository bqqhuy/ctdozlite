# MAP-22 — explicit group/community metadata

Evidence level: E2 (static official client), inspected 2026-11-18.

Source: installed Zalo PC 26.11.12 `resources/app.asar`, entry
`pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`.
Entry SHA-256: `78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08`.
At character offset approximately 3424127 the native `getGroupType(e)` reads
`getGroupInfoFromCache(e).type`. The normal=1/community=2 enum is also recorded
in re-notes/34. This is client metadata evidence, not a live server acceptance.

Parity with Zalo PC/Web: use the explicit numeric metadata type. Missing or
unknown type renders a neutral group-conversation label; names and member counts
never classify a community. Partial roster updates preserve a previously known
type, while an explicit unknown type clears it. Type controls presentation only;
the existing fresh membership/manager/settings checks remain authoritative for
mutations and sending. Cached type never grants permission.

Fixture coverage: explicit group/community, unknown numeric/string type,
misleading name/member count, partial update preservation, explicit invalidation,
and no implicit send-policy grant. Native/live verification is pending.

## MAP-23 — native call cards and local boundaries

Same entry, character offset 8361185: `generateCallMsgTemplate` creates
`msgType:"chat.recommended"`, content `action:"recommened.calling"` for callers
or `"recommened.receivecall"` for receivers; `duration` is
`Math.floor(t.duration/1e3)` seconds. The spelling `recommened` is native.
Presentation reads only these explicit actions, and preserves ordinary chat text.
No inferred numeric call-status enum is introduced. Local terminal records retain
their existing text, now scoped to the account and deduplicated by call ID rather
than second-granularity wall time. Failure before receiving a call ID uses a
process-unique local ID. This is a fork bookkeeping correction, not a wire change.
