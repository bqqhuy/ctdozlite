# ADR-0002: Bounded media resources and durable message state

- **Status:** accepted
- **Date:** 2026-11-02
- **Deciders:** Windows ARM64 product requirements

## Context

History pagination retained every message in background UI threads and repeatedly persisted the UI snapshot. Images could accumulate while the UI was hidden. Fixed camera modes and per-callback resampling assumed device behavior that is not guaranteed on Snapdragon hardware. Upstream snapshot `541ecb5` introduces resource/updater foundations but does not wire them into the product.

## Decision

Keep SQLite authoritative for complete history, retain 20 messages in background UI threads and a bounded expanded window in the active thread. Distinguish live deltas from historical pages before unread accounting. Bound texture bytes, image admission including pending GUI results, and video preview frames by call generation. Decode and resize images outside the UI thread; isolate private disk caches by account.

Negotiate camera-native NV12 output without a fixed capture rate, normalize 2D pitch and dimensions, then gate the existing 15 fps encoder. Preserve audio resampling phase and decode the actual Opus sample count. Media device errors must not implicitly stop otherwise usable media paths.

Persist allowlisted typed message metadata alongside legacy fields. Preserve local flags across network refresh; retain recall tombstones against old-page replay. Redact default diagnostics before locking the logger, and attach session credentials only to HTTPS Zalo hosts. No new call wire parameters or undocumented history endpoints are introduced.

## Consequences

SQLite retains full data; UI residency limits require explicit history expansion. Limits favor responsiveness over retaining every preview. Hardware conversion/capture and real call interoperability still need physical Windows ARM64 validation. Updater remains an unwired foundation; automatic update is not shipped. No account credentials or real contacts are used by automated tests.

## Verification

Synthetic tests cover history retention versus persisted rows, unread replay, image/cache backpressure, recall/local flag persistence, host scoping and diagnostic redaction. Audio tests use real Opus frames and chunk-independent resampling. Native CI must additionally run Media Foundation 2D buffer and codec tests, DX12 screenshots and tray lifecycle. Benchmark figures are development-host measurements, not target-device performance promises.
