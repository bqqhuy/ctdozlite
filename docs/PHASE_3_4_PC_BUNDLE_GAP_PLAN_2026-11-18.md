# Phase 3–4 remaining work plan from Zalo PC extraction

Baseline: `main` at `b5f9925`, checked 2026-11-18. This is a research and
completion plan, not a declaration that either phase has passed native/live
acceptance. PC evidence is E2 static bundle evidence in
`../re-notes/48-pc-261112-phase3-phase4-gap-extraction.md`. Source implementation,
fixtures, native UI and live server acceptance are tracked as separate levels.

## Current state

| Phase | Source state | Remaining completion work |
| --- | --- | --- |
| 3: MAP-15, MAP-10–12 | Source and fixture work is recorded in `PHASE_3_4_EXECUTION_2026-11-18.md`; do not reopen it without a reproducible regression or new evidence. | Native UI acceptance for MAP-10–12; live host/DNS, latency, ACK/echo and duplicate-delivery evidence for MAP-15. This needs controlled test accounts and no replay of an uncertain real message. |
| 4: MAP-13/14, MAP-16–29 | Source/fixture implementation is recorded; archive is explicitly local SQLite coverage. The extraction adds MAP-14 preload limits/gates and corrects claims that capture shortcut/call camera toggle are absent. | Reconcile E2 gaps below, close feasible source gaps with tests, then complete native UI checks. Live server acceptance remains distinct; destructive account operations are not part of routine verification. |

## Remaining checklist by MAP

“Source implemented” below reflects the existing Phase 3–4 execution record; it
is not a fresh independent source audit or native/live acceptance. “Pending” is
kept visible until the stated evidence is collected.

| MAP | Recorded source state | Remaining work / exit evidence |
| --- | --- | --- |
| 10 | E2EE experimental UI hidden; fail-closed policy retained. | Native scan at supported window sizes/DPI for leftover labels, glyphs, drawer/settings/tools entry points. |
| 11 | Thin sync banner and cycle dismissal behavior implemented. | Native compact/wide layout, theme, progress and close behavior. |
| 12 | Responsive drawer, quote strip and six reactions wired. | Native narrow/DPI/keyboard and quote-image rendering; controlled ACK check for reactions. |
| 13 | Rich composer toggle, whole-message formatting and shared picker wired. | Trace whether PC selection-range editor actually reaches the send path; native keyboard, selection, picker anchor and GIF checks. Preserve draft on unsupported metadata paths. |
| 14 | Settings matrix and consumers documented; capture shortcut and active-call camera control exist. | Auto-download remains a source gap until authoritative size exists end-to-end. Verify settings/native device, Run-key, Hello, notification and privacy behavior. Resolve inline-video mute/capture scope only after full PC caller trace. |
| 15 | Routing, budgets, plain-text link preview fallback and uncertain-delivery protections have source/fixture evidence. URL-bearing quote/format/mention payloads preserve metadata but skip the bounded preview path. | Trace PC caller chain before changing quote/mention wire behavior; this is AMBIGUOUS, not a confirmed defect. Controlled live DNS/host, latency and ACK/echo checks plus native UI; no replay after uncertain delivery. |
| 16 | Server pin list and scoped loading/jump implemented. | Native list/loading/error/retry; controlled pin list and ACK behavior. |
| 17 | Reminder flow and text urgency/style metadata implemented for verified text contract. | Verify PC mapping against full builder path; test native draft preservation. Keep media/voice/sticker/scheduled metadata unsupported until contract evidence exists. |
| 18 | Header label panel uses shared worker-backed label source. | Native open/apply/refresh and controlled live label acceptance. |
| 19 | Personal alias and permission-gated group avatar flows implemented. | Native modal/pending states and controlled permission/upload/result verification. |
| 20 | Link title/URL/thumbnail paths and archive date controls implemented. | Native thumbnail/layout/loading fallback and copy guard. PC extraction verifies date UI only; no archive server-schema claim. |
| 21 | My Documents drawer and account-scoped cloud path implemented. | Native auth/quota/cleanup flow; controlled account verifies usage and delete result. Missing quota must remain an error/unknown, not zero. |
| 22 | Conversation type uses authoritative metadata; unknown remains neutral. | Native rendering and controlled community entitlement/role behavior. |
| 23 | Call history local projection and identity handling implemented. | Native/WS history, unread and dedup behavior; ensure local delete remains local. |
| 24 | Relative-time formatter and boundary tests implemented; periodic repaint keeps idle sidebar labels current. | Native PC comparison for rounding, local timezone and DPI. |
| 25 | Shared file-type resolver implemented, including user-requested CAD extensions. | Native icon legibility across DPI/theme, long/unknown names and all shared surfaces. |
| 26 | Local SQLite archive filters, bounded keyset paging and date picker implemented; schema-14 migration and persistence boundary normalize timestamp units, and UI sorting uses normalized epochs. | Native filter interaction and local-history disclosure; explicitly do not claim server archive completeness. |
| 27 | Archive selection/actions, identity, confirmations and per-item results implemented. | Native clipboard/download/expired-media checks. No destructive live test on personal content; controlled test data required for live mutation proof. |
| 28 | Bubble menu and multi-select flows implemented with scope/permission guards. | Native ordering, submenu, keyboard, edges, scroll/push and modal checks; compare full PC conditions. Link-card forwarding remains documented fork deviation. |
| 29 | Sidebar context menu uses stable target thread and backend handlers; latest-message selection normalizes timestamp units before marking unread. | Extract full PC eligibility branches; native menu-condition comparison and controlled live mutation checks. Opening the menu must remain side-effect-free. |

## Work items

| Order | Scope | Plan against PC and current fork | Completion evidence |
| --- | --- | --- | --- |
| 1 | MAP-13 composer | Trace the full editor-to-chat-box caller chain before deciding whether selected-range style parity is in scope. Compare the actual fork serialized fields to PC `textProperties:{styles,ver}` and urgency `metaData`. Preserve the existing explicit whole-message formatting behavior unless the caller trace supports a safe extension. | Focused serialization tests for text styles/urgency and a clear unsupported-media behavior. Native keyboard, selection and picker anchor checks. Do not claim selection-format parity from the editor module alone. |
| 2 | MAP-14 auto-download | Implement only if the fork has a trusted size before queueing. Match PC defaults and gates: global disabled by default, per-account preference, 30 MiB file cap, video predownload disabled, 100 MiB video cap, and exclude failed/pending E2EE. Use the same source-size value for decision and actual download; reject unknown/oversize before scheduling. | Tests for disabled/global/account gates, exact cap boundaries, unknown size, video disabled, E2EE pending/failed, account switch and stale worker result. Native settings and actual bounded download check. If no authoritative size is available, document deferred instead of exposing a cosmetic toggle. |
| 3 | MAP-14 call/capture settings | Inventory current persisted settings and effective consumers. Keep the existing `Ctrl+Alt+S` capture route and active-call camera control counted as implemented. Search full PC caller chain for inline-video mute persistence and capture target/window scope; only add controls when PC behavior and a real fork consumer are established. | Source-to-consumer matrix plus native camera toggle/capture test; any new preference must persist and change the actual media/capture path. Unknown PC scope stays marked ambiguous. |
| 4 | MAP-16–19, 21–25 | No new source gap is evidenced by this extraction; use the MAP-level checklist above to close native/live acceptance. | Preserve individual pending evidence; do not group-complete these MAPs. |
| 5 | MAP-20/26 archive parity | Compare local date presets, staged calendar confirmation/cancel, future-date handling and local-day photo groups against PC E2. Preserve the fork's local SQLite-only disclosure; do not invent a remote archive query. | Tests for date boundaries, cancel/confirm, timezone/day grouping and combined local filters; native layout check. Server completeness remains explicitly unverified. |
| 6 | MAP-27 destructive/archive actions | Compare menu eligibility and self-storage flow to PC. Keep content-protection check, exact self ID, parent/attachment identity, confirmation and per-item result handling. Do not run delete/recall against real user data as an audit shortcut. | Fixture tests for protected/missing identity/error/partial batch and stale selection. Native dry inspection of menus; live mutation only with explicit test data and authorization. |
| 7 | MAP-28 bubble actions | Compare ordering/conditions and submenu against the enclosing PC menu module, not isolated strings. Verify target identity remains stable across scroll, push and submenu. Keep unknown opaque forwarding fail-closed and ordinary link-card behavior documented as a fork deviation. | Menu condition/identity regression tests and native edge/keyboard/scroll/modal checks. Mutation success remains unverified absent controlled account test. |
| 8 | MAP-29 conversation menu | Extract complete eligibility branches for each action and preset. Compare target-row semantics, labels, pin/unread/mute/hide/report/add-to-group eligibility. Preserve stable target conversation ID and ensure opening menu alone has no mutation. | Table-driven conditions and target-scope tests; native menu comparison. Live mutation checks require a controlled account and are not inferred from E2. |
| 9 | Phase 3 final acceptance | Use `SEND_LATENCY_NETWORK_ANTIGRAVITY_GUIDE_2026-11-18.md` and execution checklist. Measure host candidate and DNS behavior, text versus link latency, timeout/failover classification and ACK/echo on a controlled test account. Exercise native UI separately. | Sanitized logs with timing phases and mutation count; no message content, UID or secrets. Confirm one logical send, no unsafe retry, late ACK/echo reconciliation. If unavailable, status remains pending, never “complete”. |
| 10 | Phase 4 closeout | Run relevant source tests and repository-required checks without building a release/binary unless separately requested. Update phase tracking only for the verification level actually achieved. | Checklist row per MAP: source, fixture, native and live statuses. No phase-level “done” while required acceptance entries remain pending. |

## Recommended execution sequence

1. Close evidence gaps first: MAP-13 caller trace, MAP-14 size provenance and
   complete PC menu conditions for MAP-28/29.
2. Decide MAP-14 preload feasibility from actual fork projections. Implement a
   bounded end-to-end path only where size is authoritative; otherwise keep it
   explicitly deferred.
3. Run focused source/fixture corrections in order MAP-13, MAP-14, then any
   reproducible MAP-20/26–29 discrepancy. Preserve completed source paths.
4. Perform native checks on a Windows device. Run controlled live checks only
   for non-destructive send/ACK behavior and explicitly authorized test data.
5. Reconcile the phase checklist and report each MAP by evidence level. Phase 3
   and Phase 4 remain separate completion decisions.

## Exit criteria

- No unverified route, menu label or static UI fragment is presented as a server
  contract.
- Every implemented control has an effective consumer; unsupported or unknown
  behavior is stated plainly and does not silently drop payload or alter protocol.
- All source/fixture checks pass; native and live checks are marked only when
  actually observed. The absence of native/live access is a recorded blocker to
  acceptance, not a reason to stop source work or overstate completion.
- Archive scope remains local unless complete server behavior is independently
  evidenced. Destructive actions remain untested on personal data.
