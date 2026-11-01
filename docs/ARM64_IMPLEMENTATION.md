# Windows ARM64 implementation notes

These changes address the audit baseline at commit `3f28d2f99413d84e31d979cb914be3cc7945fb15`. See [Windows ARM64](WINDOWS_ARM64.md) for build and runtime instructions.

| Audit item | Implemented behavior |
|---|---|
| A01 | Current-user DPAPI on Windows; every session save checks remember-login; disabling it removes the persisted session. |
| A02 | Per-account history/cache paths; revoked session snapshots cannot persist credentials; logout/expiry replaces the worker runtime and UI event channel. Sensitive UI state is cleared. |
| A03 | Absolute per-user data paths, explicit portable mode, atomic file replacement and one instance per data directory. Legacy unowned data is not imported automatically. |
| A04 | The outbound test allowlist is opt-in through ZALO_CALL_TEST_LOCK; group calling remains unsupported. |
| A05 | FFmpeg is resolved by absolute path, checked as ARM64 on Windows ARM64 and checked for capture, codec and filter support before enabling video. No FFmpeg binary is bundled. |
| A06 | Windows ARM64 wgpu is constrained to DX12; adapter/backend is reported during initialization; glow remains an explicit/fallback renderer. |
| A07 | Capture/playback dispatches F32/I16/U16/I32/F64 conversions and reports missing devices and stream creation failures in the GUI. |
| A08 | Network responses are capped while streaming; image dimensions/allocations and decompressed payload sizes are bounded; concurrent image jobs are capped at four. |
| A09 | Call signaling/wire logs require opt-in diagnostics and rotate under the data directory. |
| A10 | ARM64 workflow covers PR/main, uses a pinned Rust toolchain/actions, runs all-target tests and checks/runs the built executable. |
| A11 | Packaging fails if the installer is absent and emits an ARM64-specific checksum and source/toolchain manifest. Build jobs use read-only repository permissions; publishing uses a separate write-enabled job. |
| A12 | Folder selection runs off the GUI thread and returns through the current UI event channel. |

Persistence snapshots are transactional. A cancelled polling task releases its in-flight flag, including during account transitions. Session flags and paths are exercised with synthetic accounts; tests do not authenticate to Zalo or call real contacts.

## Validation and remaining checks

`cargo test --locked --all-targets` on Linux passes 103 library tests (one live test ignored), the same tests compiled into the application binary, and the separate account/storage lifecycle integration test. Tests include atomic replacement, account cache/database separation, remember-login, revocation, transaction rollback, polling cancellation, chunked-response limits, decompression limits and PE architecture parsing.

The Windows-only DPAPI test is included in ARM64 CI. Windows ARM64 execution, actual DPAPI behavior, GPU rendering/fallback, microphone/camera/Bluetooth, sleep/resume and installer behavior still require the Windows runner or physical hardware. The build script and workflow provide those build/test hooks; a Linux test pass does not establish Windows runtime behavior.

The binaries remain unsigned. SQLite history is scoped per account but not encrypted at the application layer. Legacy session/history is retained in its old location and needs a verified migration before reuse. The unimplemented autostart setting is disabled in the UI; attachment links still use the external application/browser for downloads.
