# Review: Antigravity animated sticker proposal

Date: 2026-11-16 · Base: `main` @ `344e00c` · Reviewer: Claude Code (Opus 5.5)

## Verdict

**Do not approve as written.** The rendering approach (decode frames off the UI
thread, pick the frame by elapsed time, animate only visible stickers, cap
concurrent animations) is sound. The format survey is wrong, and the spec
depends on it.

## Evidence (E2, live CDN probe, 5 sticker IDs)

| Endpoint | Result |
|---|---|
| `https://api.zaloapp.com/api/emoticon/sticker/webpc?eid={id}&size=130` (used today by `endpoints::legacy_sticker_image`) | **Static PNG** for every sampled ID (2005, 21610, 23339, 46990, 50601). No APNG `acTL` chunk. No animated WebP observed. |
| `https://zalo-api.zadn.vn/api/emoticon/sprite?eid={id}&size=130` | **Horizontal PNG sprite sheet**. eid 23339 → 360×180 = 2 frames of 180×180. Static stickers return a single frame (130×130). |

The sample is small. It is still enough to show that Zalo sticker motion comes
from **sprite sheets**, not Animated WebP. The code comment on
`ChatMessage::sticker_image_url` (`src/chat.rs:565`) already says it returns
the static PNG URL.

## Errors and gaps in the proposal

1. **"webpc returns Animated WebP" is incorrect.** It returns static PNG.
   Swapping in a WebP/GIF animation decoder does not animate Zalo stickers.
2. **The `sprite` endpoint is missing.** `src/` never uses it, and it is the
   real source of motion.
3. **Frame timing is unknown.** A sprite PNG has no per-frame delay. Frame
   count can be derived as `width / height`. Playback speed must be sourced
   from Zalo PC metadata or JS (cite the source as other endpoints do), never
   guessed.
4. **"Smooth 60 FPS" is a claim, not a requirement.** Stickers with a few
   frames need roughly 10–25 FPS. `request_repaint_after` should fire at the
   frame cadence, not every 16 ms.
5. **Texture budget is not addressed.** `GuiApp::store_texture` enforces
   `media_resources::TEXTURE_BUDGET` (128 MiB) with eviction. N frames cost
   N× memory and must be accounted for there.
6. **Bundled GIFs are the one correct case.** `src/bundled_stickers.rs` ships
   30 Ami stickers with `.gif` variants. Decoding these with
   `image::AnimationDecoder` is valid, and the `gif` feature is already
   enabled in `Cargo.toml`.
7. **Drop approaches 2 and 3.** Streaming WebP and Lottie have no observed
   use for chat stickers. Keep them out of the spec.

## Recommended design

### Sources
- Zalo stickers: fetch `sprite?eid={id}&size=130` (new endpoint constant in
  `src/endpoints.rs`). On failure or invalid geometry, fall back to the
  current static `webpc` image.
- Bundled stickers: decode the embedded GIF frames and delays.

### Decode (worker, `spawn_blocking`)
- Use the existing bounded path: `limits::read_bounded_response` /
  `limits::decode_image` limits, private media cache, `validate_image_body`.
- Validate the sprite: `height > 0`, `width % height == 0`,
  `1 <= frames <= MAX_STICKER_FRAMES` (for example 64), and per-frame side at
  most 512 px. Anything else falls back to frame 0 or the static image.
- Send one event carrying the sprite `ColorImage`, frame count and frame
  duration(s). Keep `StickerImageReady` for static images.

### Texture and render (UI thread)
- Upload **one sprite texture** per sticker and draw the current frame with a
  UV sub-rect (`egui::Image::uv`). Do not create N textures. Account the full
  sprite bytes in `texture_budget`.
- Animation state: `start: Instant` per visible sticker key. Frame index =
  `(elapsed % total) / frame_duration`.
- Animate only when `ui.is_rect_visible(rect)` and the sticker is among the
  newest 3–5 visible animated stickers. All others render frame 0.
- Schedule `ctx.request_repaint_after(time_to_next_frame)`. When no animated
  sticker is visible, schedule nothing (zero idle repaint).
- Add a setting to disable sticker animation (battery / reduced motion).
- Apply the same widget to the sticker picker, but cap it to stickers under
  the hovered or visible rows.

### Out of scope
- Lottie, streaming WebP, full-screen reaction effects.

## Prerequisite research (before writing the spec)

1. From Zalo PC JS: how the client decides a sticker is animated, and where it
   reads frame duration / FPS (sticker detail fields or a constant). Record
   the file and offset as existing comments do.
2. Probe a larger sample (≥ 30 IDs across categories) to confirm the sprite
   layout is always horizontal, and record `size=130` vs larger-size
   behaviour (eid 23339 returned 180 px frames).
3. Confirm whether `webpc` ever returns animated content for any category.

## Required tests (closure criteria)

- Sprite slicing: 1 frame, 2 frames, N frames; non-divisible width; zero
  height; frame count above cap → static fallback.
- Frame index math at boundaries (t = 0, t = frame_duration, wrap at total).
- Texture budget counts the whole sprite and evicts as for static textures.
- No repaint is scheduled when no animated sticker is visible.
- Animated sticker limit: only the newest K visible stickers animate.
- GIF bundled sticker: frames and delays decoded; a zero delay is clamped
  (for example to ≥ 20 ms).
- Gates per `AGENTS.md` (fmt, check, clippy default + all-features, tests
  default + `experimental-e2ee`, prepush scripts).

## Acceptance left to the owner

Visual smoothness, CPU/GPU usage and battery impact on native Windows 11 ARM64
(E3) are not verifiable in this review and remain the owner's sign-off.
