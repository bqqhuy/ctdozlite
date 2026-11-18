# Zalo PC 26.11.12 evidence for Phase 3–4 gaps

Date: 2026-11-18. Evidence level: E2 (static renderer bundle), except where
explicitly marked otherwise. This note records observable client behavior only;
it does not establish server acceptance or undocumented API contracts.

## Source identity

- Extracted renderer: `%LOCALAPPDATA%/Temp/ctdoz-map27-pc-26.11.12.js`
- Installed client: Zalo PC 26.11.12, renderer bundle
  `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`
- Size: 21,691,083 bytes
- SHA-256: `78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08`
- Offsets below are character offsets in the extracted JavaScript file, not
  byte offsets in the original archive.

## Findings relevant to open work

| MAP | PC observation (E2) | What it supports | What it does not prove |
| --- | --- | --- | --- |
| 13 | Message builder at 2,126,809 retains rich-text `styles` in RTF parameters; shared `buildParams` at 15,086,328 emits `textProperties` with `styles` and `ver`, and puts `urgency` in `metaData`. | Separate formatting and urgency metadata paths exist. The existing fork path can be compared field-for-field against this evidence. | The editor module at 2,266,112 has selection-range style machinery, but this static fragment does not prove the chat composer wires it into the send path. Formatting scope/HTML and all media combinations remain unconfirmed. |
| 14 | Configuration at 8,060,761 sets `enable_predownload_file:false`, max files 100, max file size 31,457,280 bytes (30 MiB), video-size predownload false, max video size 104,857,600 bytes (100 MiB). Module `M0pa` at 7,453,073 exposes those settings and gates automatic predownload on global enable plus the account auto-download preference. Incoming path at 17,226,988 checks cached file size against the max and only auto-downloads photo/file/GIF when enabled; failed/pending E2EE is excluded. | The PC file/video bounds and preference gates are explicit client configuration. Fork can implement an equivalent local policy if it has a trustworthy size before scheduling. | These values do not establish a server limit, nor should E2EE pending/failed content be downloaded by inference. Fork currently lacks the equivalent predownload consumer and projections do not consistently expose authoritative size. |
| 14 | Fork source has `Ctrl+Alt+S` capture action (`src/gui/workspace.rs`) and active-call camera state/control (`src/media.rs`, `src/call.rs`, command 12006). | Capture shortcut and call camera enable/disable are already present in source. | Does not prove parity for persistent inline-video mute, capture-window scope, or native hardware operation. Do not count the shortcut/camera toggle as wholly missing. |
| 17 | The PC builder at 2,126,809 keeps urgency distinct from rich text; `buildParams` at 15,086,328 serializes styles and urgency separately. Observed urgency values in shared constants: normal `-1`, important `1`, urgent `2`, mention `3`. | Fork's text-only verified mapping has a concrete E2 comparison point. | Does not establish support for these metadata fields on voice, sticker, scheduled or arbitrary media sends. |
| 20 / 26 | Archive date-picker module `nNCO` at 17,439,296 defines clear, one-week, one-month, three-month and custom modes. The calendar stages a range and exposes confirm/cancel behavior; archive UI groups photo items by local day. | Fork's local date presets/calendar and daily photo grouping can be checked against visible PC behavior. | No evidence of a server-side archive filter schema or complete server history. |
| 27 / 28 | Bubble-menu region near 192,300 conditionally exposes recall/delete actions. My Documents menu at 11,867,017 checks content protection and calls the chat controller share-to-self flow. | Actions are conditional and self-storage is a share/forward-to-self behavior subject to protection. | Static menu visibility does not prove mutation success or authorize testing destructive actions against a real account. |
| 29 | Conversation-menu region near 3,055,735 conditionally assembles pin, label/read state and other actions. Mute presets observed: 1 hour, 4 hours, until 8 AM and unlimited. | Provides a visible menu-condition/preset comparison for the fork UI. | Static UI does not prove that every mutation succeeds, identify every account/group eligibility condition, or establish the server contract. |

## Phase 3 evidence boundary

The 26.11.12 bundle scan did not reveal a new static-client finding that
changes the source checklist for MAP-15 or MAP-10–12. It cannot resolve the
remaining live DNS/host selection, latency distribution, delivery ACK/echo,
or native layout acceptance. Those remain explicit verification work, not a
reason to infer contracts from route names.

## Follow-up source repair

The Phase 4 audit found mixed seconds/milliseconds epochs could misorder archive
pages, sidebar mark-unread selection and in-memory timeline merges. The fork now
canonicalizes timestamps at parse and persistence boundaries, migrates existing
SQLite message/thread/phone-sync rows to schema 14, sorts archive/timeline data
by normalized milliseconds, and preserves rejection of negative sync ranges.
Focused regressions cover archive pages, phone ranges and mixed-unit merge order.
This is a local data-consistency repair; it does not assert any new Zalo server
contract.

## Follow-up extraction needed

Before implementing any remaining UI parity detail, capture the full enclosing
module/caller chain (not only the matching token) for composer selection styles,
video mute and capture scope, and exact conditions on MAP-29 actions. Record new
offsets and a bundle hash. If only static behavior is available, classify it E2;
do not promote it to live API evidence.
