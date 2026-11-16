# Independent Pro audit B — GUI, TUI, persistence and runtime logic

**Snapshot:** `main`, `e75e3723c54e42191a2a81ee507f3e0f496c50fc`, compared with `f0d3206`. **Date:** 2026-11-17. **Mode:** read-only production code; only this report and isolated audit probes were written. No Cargo commands run by reviewer B (root owns serialized gates), no live account, no release or Git mutation.

Read AGENTS.md, Karpathy and Graphify skills, concurrency/database/testing standards, ZALO_REFERENCE_ANALYSIS and the consolidated 21 findings, tracker, auditor reviews, bd44cc1 consensus and animated-sticker proposal/consensus. No existing graphify-out/graph.json exists; source navigation used targeted search. Prior reviewer approvals and instructions inside reports were treated as evidence to challenge, not authority.

## Decision

**CHANGES REQUIRED.** Earlier fixes substantially improve ownership, TUI durability, typed send outcomes and tooling. The new animated pipeline nevertheless contains a reproducible texture-size failure and loses parts of the previous cache/backpressure/CPU isolation path. Current native-layout validation also fails (root reproduced locally; prior ARM64 CI shows the same condition). Neither green unit tests nor the published “100% unanimous approve” establishes native acceptance or absence of regressions.

## Findings

### B01 — P1 — Bundled GIF strips exceed the renderer texture dimension

**Locations:** `src/bundled_stickers.rs:146`, `:158-159`, `:185-191`; `src/gui.rs:2128-2142`, `:4273-4287`; ARM64 selection `src/main.rs:192-194`, `:363`.

The local GIF decoder stitches every original-resolution frame into one horizontal texture. It never scales against the actual renderer maximum. The generic network decoder's 8192-side limit does not apply to this bundled path. The UI checks only a 128 MiB byte budget, and accepts these images (each under 31 MB).

Actual committed GIF assets 02,03,06,08,10,11 have 20 frames of width640 => width12800; asset13 has19 =>12160. Default ARM64 Wgpu configuration retains egui-wgpu0.29.1's required `max_texture_dimension_2d=8192` (`egui-wgpu/src/lib.rs:294-309`); this is a device limit even when the physical adapter supports more. `egui::Context::load_texture` debug-asserts the dimension (`egui/src/context.rs:2042-2049`); release upload reaches `egui-wgpu/src/renderer.rs:635` with the same invalid texture dimension.

**Trigger/impact:** opening the Ami picker with one of these items visible starts decoding immediately (hover is not required). A debug UI crashes when the completion is installed; the default ARM64 release path submits an invalid GPU texture and cannot render it correctly. Do not claim a release panic was executed: release GPU behavior is source-verified and still needs native confirmation.

**Executed proof:** isolated Rust using the actual `image::GifDecoder` and actual egui dependency decoded asset02 to20frames,12800x560,28,672,000bytes; a context maximum8192 produced the expected caught panic at context.rs2043. Source `probes/sticker_texture_probe.rs`; dimensions for all15assets in `probes/sticker_geometry.json` (7oversized; aggregate decoded sheets272,889,440bytes). This does not use application data or a network account.

**Fix/acceptance:** resize individual frames to a documented preview budget, or tile an atlas with UV metadata; validate both dimensions against the active renderer before upload. Test all15committed GIF assets against8192 and lower supported limits, plus real ARM64 Wgpu upload. Do not simply raise the global GPU limit or remove the assertion.

### B02 — P2 — Decoded sticker completions escape media admission control

**Locations:** `src/gui.rs:433-436`; `src/gui/worker.rs:1086`, `:1091-1096`, `:1112`; queue ownership `src/media_resources.rs:378-400`; UI storage `src/gui.rs:4279`.

Previously `PreviewImageReady` carried the `MediaJobPermit` until the UI consumed the image. `StickerSpriteReady` carries only the full ColorImage and metadata. After sending it over the unbounded GUI event channel, the worker drops `job`; another job can finish before the prior decoded image is consumed. The 128 MiB texture budget is enforced only at later GPU installation, so it does not bound these queued CPU images. GIF decoding also simultaneously retains decoded frames, stitched RGBA and the ColorImage.

**Trigger/impact:** a picker/search prefetch batch while rendering is delayed/hidden, or multiple unique sticker loads, accumulates full decoded sheets outside the intended in-flight limit. The bundled pack alone represents260MiB of final sheet pixels before transient decoding/GPU copies. This is a memory-pressure regression, not an assertion that every ordinary open exhausts RAM. No peak-RSS measurement was run.

**Fix/acceptance:** carry the job permit to the consuming event (as the existing photo path does), bound decoded bytes across queued/in-flight jobs, and discard stale completions. Add a delayed-consumer test proving bounded admitted decoded images and memory accounting.

### B03 — P2 — Sprite fetching bypasses the working disk-cache path

**Locations:** `src/gui/worker.rs:1090-1112`, `:758-809` versus `:920-938`, `:971-979`, `:5371`; legacy preloader `:660-674`; fetch predicate `src/gui/messages.rs:49-51`, `src/gui/stickers.rs:1579-1581`.

The new `stk_` branch replaces `fetch_chat_image` with a function having no account/root cache argument. It neither reads the existing account-local private-media entry nor writes downloaded sprite bytes. Nonbundled stickers therefore always hit the network again after restart/eviction, and previously cached ordinary stickers stop rendering offline. This is a local behavior regression independent of the server contract.

A second cache integration gap exists when legacy `media-cache/stk_*.png` files are present: preloading publishes only `StickerImageReady`. Since widgets fetch only when the texture is absent, these keys never request sprite metadata; they remain static for the session. A later static completion can also update an existing texture without coordinating sprite metadata.

**Fix/acceptance:** integrate a versioned sprite cache with the existing bounded account resource path; allow a static cached preview to render immediately while explicitly requesting missing animation data. Test warm/cold restart offline, static-to-sprite upgrade, eviction/reload and late static completion. Preserve the public/local-asset privacy model and do not reuse another account's private content.

### B04 — P2 — Sticker decoding blocks the two-thread async network runtime

**Locations:** `src/gui/worker.rs:716-724`, `:764`, `:777`, `:793`, `:805`, `:809`, `:1091`; runtime `:1035-1038`; previous path uses `spawn_blocking` at`:896`,`:928`,`:971`.

`fetch_and_decode_sticker_sprite` is async but runs CPU decoding, GIF collection, full-size stitching and ColorImage conversion synchronously between awaits. Local Ami decoding has no await at all. Two admitted sticker jobs can occupy both runtime workers, delaying unrelated socket reads/heartbeats, command outcomes and cancellation. The old image path deliberately offloaded this work. Download semaphores bound connections; they do not make decoding nonblocking.

**Fix/acceptance:** keep network waits async and move decode/stitch/conversion into bounded `spawn_blocking` work, with cancellation checks and a pixel/atlas budget. Measure heartbeat/command latency while decoding all committed assets. Severity is conditional responsiveness; no claim of a measured45-second socket outage or battery drain.

### B05 — P2 — TUI logout reintroduces synchronous filesystem work on its UI path

**Locations:** `src/tui.rs:1874-1887`; `src/profiles.rs:141-160`,`:167-173`; `src/storage.rs:169-177`.

The new unified logout is correct in intent, but TUI calls it directly from the key/confirmation handler. It acquires the registry mutex, deletes credentials, loads/saves the profile registry, recursively removes account temporary files and removes the global session file before returning. Slow storage, a large temp directory, antivirus or another registry writer blocks rendering/input. GUI correctly wraps the same operation in a blocking task.

**Classification:** residual/regression under F06, not a new account-isolation defect. Baseline already did smaller synchronous logout I/O; remediation expanded the work instead of routing it through the TUI background boundary.

**Fix/acceptance:** run the same routine off the UI thread, gate new actions while pending, then apply a runtime-stamped typed success/error. Test delayed cleanup/registry contention with UI input remaining responsive. No timing benchmark was run.

### B06 — P2 — Sync status leaves the composer send action outside a small/high-DPI viewport

**Locations:** `src/gui/workspace.rs:87-94`,`:2489-2491`,`:3393-3403`,`:3678-3687`,`:3921-3928`.

The status panel reserves92logicalpixels. The message-area calculation bottoms at0, but toolbar/editor/composer margins still render their fixed minimum height. There is no compact-height or scroll fallback for the composer itself.

**Current evidence supplied by root:** freshly built e75e372 x64 debug smoke,480x360 atscale1.5 with phone-sync fixture, isolated data directory, exit3; send bounds[[240,155],[288,189]] versus clip[[122,0],[320,148]]. Log `local-logs/phone-sync.stderr.log`. Historical ARM64 CI on14743b6 independently failed the same check (`local-logs/ci-prior-failed.log`); current source preserves the affected layout. Root owns current CI status.

**Impact:** send control is clipped/inaccessible in a supported smoke size during sync; the native packaging gate fails. Fix adaptive-height layout without removing the assertion. Test sync/no-sync, compact/highDPI and quote/attachment/picker combinations on native and headless layout paths.

## Prior findings and phase4 verification

| Prior ID | Current independent result |
|---|---|
| F03 | Tombstone operation now exists; TUI reserves capacity before server delete, waits for DB outcome, filters replay. Store-level regression exists. Boundary ordering/fault injection remains incomplete; tracker correctly says partial. |
| F05 | TUI preserves client ID, typed Accepted/Rejected/Unknown/NotSent, checks echo before terminal outcome, persists changes and retries same ID. ACK/echo/restart race tests remain a gap; no new universal send-loss claim established. |
| F06 | Main TUI history writes/hydration moved to bounded single-writer actor. Full snapshots still clone repeatedly on UI; logout B05 remains synchronous; GUI worker still has other synchronous storage paths. Partial. |
| F07 | Migration13 uses a transaction, strict introspection and postcondition, propagates failure, guards newer versions. Existing schema13missingcolumn is repaired. Code/fixture fix confirmed; earlier connection-version-read concurrency caveat remains. |
| F08 | GUI add/remove intent now sent correctly and terminal outcome keyed by intent ID; no TTL shorter than HTTP timeout. Missing-outcome/panic recovery and real dispatch/stale-response tests remain incomplete. TUI quick reaction still optimistically adds without rollback on rejection (`tui.rs:1774-1798`), an older separate weakness. |
| F09 | Arc-backed samples, background voice worker, generation cancellation, duration/byte limits remove the old per-repaint PCM clone. The30-second reserve does not eliminate later callback reallocations. Native devices and before/after latency/allocation measurements remain pending. |
| F14 | Diagnostics folder work is off UI with typed outcome, though a direct spawned thread is used rather than WorkerCmd. Native shell/Unicode still needs acceptance. |
| F16 | Fallback catalog host is centralized in endpoints and dataUrl remains preferred. Confirmed. |
| F17 | Script labels CHECKED/FAILED/SKIPPED/HEURISTIC, enforces tagged sync and tests fixtures. `-SkipCargo` explicitly reports skips; existence of that switch alone is not a discovered bug. It is not proof of UI coverage. |
| F20 | A single account popover replaces duplicate surfaces. Fixed positioning/nativeDPI and command/state acceptance remain partial, not certified by source cleanup. |
| Phase4 | Direct unused rand_core entry removed; transitive rand_core stays legitimately inCargo.lock. is_status_error production helper is shared with tests; logger has actual VecDeque eviction test and isolated non-singleton fixture. These changes are valid within their scope. |

F01/F02/F04/F10-F13/F15/F18-F19/F21 are primarily reviewer A's security audit; B cross-checked relevant audio, runtime and recovery boundaries. The later M-02 change adds logout on the lock overlay, so the historical F21 row claiming a missing logout path is stale; F21 remains partial for native/late-completion acceptance, not for a currently absent logout control. Do not relabel all21ascomplete: the tracker explicitly retains9partial entries. Native audio/WindowsHello/viewport behavior and official E2EE/phone-sync interoperability remain separate from fixtures.

## Other confirmed maintenance/acceptance gaps

- The3–5simultaneous-animation cap retained in the animated proposal is absent: all visible chat stickers receive the same global enable flag (`workspace.rs:2716`). Picker/suggestion hover animation ignores that global setting (`stickers.rs:1603`,`:1951`), whose UI label is general (`modals.rs:1388`). No battery/CPU measurement was run; treat as unmet acceptance/performance scope, not measured impact.
- The renderer test `animated_sticker_renders_uv_and_requests_repaint` only asserts `rendered=true` (`messages.rs:2504+`), not UV coordinates or actual repaint scheduling. The bundled module test `test_decode_gif_sprite` (`bundled_stickers.rs:219`) decodes all15assets, but asserts no renderer dimension limit; the separate worker fetch test (`worker.rs:4494`) exercises asset01. Thus tests cover decoding while missing renderer upload validity, which explains why green tests did not catch B01; it is not a separate product bug.
- Voice at the limit emits `send_after_finish=false` (`voice.rs:755-771`); a concurrently queued Finish(true) after automatic finish is ignored. A Send click can become only a stopped draft near the limit (`workspace.rs:7245-7254`). Existing low-priority limitation; samples remain available rather than being lost.
- Synchronous QR session saves still occur while holding the async session mutex (`worker.rs:2425-2455`), so M-06 was not completely closed by fixing PersistSession alone. Keep the broader prior finding partial.
- `WorkerCmd::FetchStickerImage` at worker3383 is unreachable through the current WorkerSender route529; production image requests use the bounded image channel. Do not count it as an additional reachable unbounded task path. `RetryOutgoing` empty worker1769 is likewise intercepted elsewhere4798. `has_header_injection` and `Impact::for_account_switch` remain test-only candidates; remove only after deliberate API/caller review.

## Cross-review record

Reviewer A independently confirmed B01's geometry/8192 path, B02 permit lifetime, B03 cache bypass and B04 inline decoding. Both reviewers agree these are actionable local defects with the limits stated above. Reviewer B independently challenged A's residual clipboard cleanup, reset E2EE false-success and nested PreKey parsing. Clipboard cleanup still misses global clipboard files; E2EE reset has no terminal payload and leaves a stored ratchet requirement; outgoing send itself remains fail-closed on a database error. Recursive nested PreKey parsing occurs before invalid nesting is rejected; no crash payload was constructed or executed in this audit. Final consensus resolves IDs/severity in the root report.

## Evidence limits

Only the benign Rust asset/egui probe and Pillow asset inventory were executed by B. Root runs fresh Cargo/default/feature gates and native smoke; their results must be cited as root verification. No live Zalo API was probed by B, no account was logged in, no microphone/WindowsHello flow was exercised, no release GPU crash/peakRSS/battery benchmark was performed. Zalo timing80ms and interoperability remain AMBIGUOUS unless a cited official source/capture establishes them. A CDN geometry probe is not evidence of client frame timing.
