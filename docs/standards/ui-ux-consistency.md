# UI/UX Consistency and Acceptance

This standard applies to every agent and human adding or changing GUI surfaces,
including a small banner, link, button or status row. It complements the no-stub,
worker, account-isolation and protocol-evidence requirements in `AGENTS.md`.
Backend wiring and UI/UX acceptance are separate requirements; both must pass.

## 1. Establish the baseline before editing

- Identify the affected surface, its container, and adjacent working components.
  Record the reported reproduction, affected states, available width, theme and
  scale when known. Distinguish user reports, measured observations and hypotheses.
- Read the current theme, shared widgets and layout/interaction implementation,
  plus `docs/ZALO_GUI_UX_OVERHAUL_PLAN.md` and
  `docs/GUI_UX_HANDOVER_AI_GUIDE.md`. Historical completion labels do not certify
  the current build.
- Compare equivalent Zalo PC/Web states when parity is claimed. Record the source,
  version/date and evidence level. Mark unverified behavior AMBIGUOUS; explicitly
  justify a fork deviation. Do not present guessed dimensions or copy as native.
- Specify the intended placement, hierarchy, sizing, overflow behavior, states and
  interaction before implementation. A short task note/table is sufficient; a new
  design framework or separate roadmap is not required.

## 2. Integrate with the existing visual language

- Reuse existing theme tokens and suitable shared widgets for fonts, colors,
  spacing, padding, radius, borders, icons, hover, focus and disabled states.
  Use Lucide/vector assets where the current action-icon system uses them; do not
  substitute arbitrary emoji or raw technical text controls.
- Compare the new surface beside its neighbors, including alignment, density,
  visual emphasis and hit targets. A new banner must fit its parent panel's content
  bounds and spacing conventions; it must not impose new global margins merely
  to hide its own overflow.
- Measure text using the actual font/style and actual available content width.
  Budget icons, gaps, padding, CTA and close controls together. Reserve usable
  action targets; wrap, reflow or deliberately truncate secondary text with a way
  to retrieve the full content. Never clip a required action or hide the only
  explanation behind a hover-only affordance.
- Handle runtime/server-provided copy and all relevant states rather than sizing
  only for one default string. Do not overwrite configuration or change protocol
  semantics just to make a label fit. A shorter local label requires an explicit
  copy/fallback policy and evidence if called native parity.
- New shared abstractions are justified by actual reuse. A local fix may reuse
  existing primitives without creating another parallel widget system.

## 3. Integrate behavior and state

- Choose banner, inline row, menu, anchored popover or modal according to purpose
  and reference behavior. Do not expose a backend feature as a raw link/button in
  an unrelated zone merely because that is easy to wire.
- Define applicable idle, loading/progress, success, empty, error/retry,
  unavailable/disabled, dismiss/reopen and account-switch states. Display truthful
  outcomes from the existing state/worker flow; do not simulate progress, fabricate
  payloads or report success before confirmation.
- Preserve pointer, wheel, keyboard, focus, IME and shortcut ownership. For
  overlays, verify backdrop/dialog/child ordering across frames, outside-click
  policy, Escape/X and focus restoration. No click-through or invisible blocker
  may survive close, navigation or account/runtime change.
- Keep local-only UI interactions local. Operations requiring network/DB must
  retain the established worker/result path, request ownership and error handling.
  Consistency changes do not authorize weakening privacy or sync safeguards.

## 4. Verify the component and its surroundings

Use a risk-based matrix and record actual values/results:

| Dimension | Required coverage when applicable |
| :--- | :--- |
| Content | Short/long Vietnamese text with diacritics, real configured copy, dynamic status, empty/error states |
| Geometry | Minimum supported panel width, ordinary/wide window, sidebar resizing, drawer open/closed, compact/narrow transitions |
| Scale/theme | Light/dark; Windows 100%, 125%, 150%, 200% or record unsupported/unavailable cases explicitly |
| Interaction | Pointer and keyboard activation, focus visibility/restoration, repeated open/close, outside click, child overlays, IME where text entry exists |
| Integration | Adjacent rows/header/composer remain aligned and usable; no new clipping, hit-target overlap, scroll jump, accidental send or background action |

Capture before/after screenshots of the same affected state where available;
include a view with surrounding UI, not only an isolated widget. Prefer synthetic
data/redacted captures; never commit account data, cookies or session dumps.
Offline layout/hit-test regressions are useful for deterministic defects but do
not replace direct Windows rendering and interaction checks. Native tests that
send messages, place calls or mutate an account require existing authorization.

## 5. Completion and review gate

Every GUI change must report the following in the existing task/verification
record or PR, scaled to the change:

- [ ] Scope/reproduction and reference or justified deviation recorded.
- [ ] Existing tokens/widgets and surrounding layout reviewed; local exceptions explained.
- [ ] Applicable states and real action/result flow checked; no UI stubs or discarded payloads.
- [ ] Text, geometry, theme and input matrix results recorded; clipping/overlay regressions checked.
- [ ] Relevant compilation/tests passed for code changes; checks named accurately.
- [ ] Screenshot/native acceptance evidence linked, or remaining device checks explicitly pending.

Report implementation, fixture checks and native acceptance separately. A build,
test count or backend response alone cannot justify "UI complete", "native
verified" or "100% parity". If device access is unavailable, finish independent
authorized work and hand off concrete acceptance steps without marking them done.
Expand a local repair only after reproducing a shared defect and identifying its
consumers; preserve already-working surfaces and review any shared change's blast
radius. Documentation-only edits do not require invented GUI tests.
