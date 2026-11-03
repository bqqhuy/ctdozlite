# ADR-0015: Native notifications, explicit media quality and safe attachments

- Status: accepted
- Date: 2026-11-02
- Supersedes: ADR-0011 media presentation/admission where specified below.

## Context

The maintainer approved eleven repairs: phone sync, image/video speed, saving attachments, desktop popup, suppressing call history, group sender previews, cross-device call cleanup, stable sync layout, clipboard/upload crashes and unread badges. Reference UI was statically extracted from Zalo PC 26.11.10.3400; no installer code was executed or bundled.

## Decision

Use explicit thumbnail/preview/original URLs. Visible chat cards request thumbnails; opening a photo requests its preview; saving requests the original. Missing thumbnails do not silently download originals. Video uses a thumbnail/play affordance and a streamed local file followed by an MFPlay viewer with audio, pause, seek, volume and fullscreen. This first implementation waits for download completion before playback.

Attachment downloads stream to unique partial files, use two transfer slots, support cancellation/progress, and finish without overwriting an existing destination. Validate every redirect without attaching session credentials. Windows finalization uses a non-replacing move, including removable filesystems.

A native, nonactivating, topmost popup presents avatar/title/sender/body, dismiss button and exact-conversation activation. Lifetime is 4.5 seconds; a newer message replaces it with a 150 ms transition and a fresh timer. Popup and audio settings are honored. Numeric taskbar overlays (1–5, then 5+) and a tray red dot derive from unread state independently of popup settings; ordinary incoming messages no longer request flashing attention. Active calls retain their own attention behavior.

The official public notification MP3 is fetched once into a local cache, validated and played locally with burst coalescing. Do not extract/distribute proprietary resources in the repository. Download failure is visible and does not imply audio was successfully installed.

Filter call records from both cached and live chat presentation, search, previews and unread calculations, without deleting stored history rows or disabling active calls. Group previews share one sender formatter. Cross-device accepted/ended events clear pending local incoming calls; incoming actions carry their call identity.

Clipboard DIB parsing checks headers, stride, masks and pixel budgets before allocation. Release native handles before decoding and file I/O. Panic/native exception diagnostics contain phase names and code locations, not chat bodies, session material or memory dumps.

## Evidence

Installer SHA-256: `7166ea28925b3121369ffb25e87b4749a648f47a861272009dcb14094b5a5511`.
ASAR SHA-256: `70f1b53c4db61321bf1b11c722a9049823711328b87e17b0d19338f83a5f6ed3`.
Original popup dimensions/title/body/avatar and separate taskbar/tray update paths informed the implementation. The requested 4.5-second lifetime intentionally differs from the inspected ten-second reference.

## Validation and rollback

Unit tests cover media quality separation, call classification, sender previews, notice replacement/account isolation, clipboard corruption and safe streaming finalization. Linux GUI smoke and a Windows ARM64 API compile check supplement these; neither proves live phone compatibility or actual Windows audio playback. Native Windows runtime validation remains mandatory before declaring those behaviors verified. Reverting the change requires no database migration.
