# AGENTS.md

Operating guide for AI agents and humans working in this repository.

## Project

`ctdozlite` is a Windows 11 ARM64 Zalo desktop client written in Rust: `egui` GUI, `ratatui` TUI, a Tokio worker for network I/O, and a local SQLite message store. No Node.js, no Electron, no Chromium.

- Repository: <https://github.com/bqqhuy/ctdozlite> (upstream: <https://github.com/ctdoteam1110/ctdozlite>)
- Pinned toolchain in rust-toolchain.toml / edition 2021
- Product docs are Vietnamese (`docs/`, `README.md`); engineering standards are English (`docs/standards/`)

## Commands

| Task | Command |
| :--- | :--- |
| Build (debug) | `cargo build --locked` |
| Build (release binary) | `cargo build --release --locked --bin ctdozlite --target aarch64-pc-windows-msvc` |
| Test | `cargo test --locked` |
| Format | `cargo fmt --all` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Run GUI | `cargo run -- --gui` |
| Run TUI | `cargo run -- --tui --qr` |

`CMAKE_POLICY_VERSION_MINIMUM=3.5` is required when building against CMake 4.x (see `docs/WINDOWS_ARM64.md`).

## Architecture Map

| Path | Responsibility |
| :--- | :--- |
| `src/main.rs` | CLI entry; selects GUI or TUI mode |
| `src/lib.rs` | Library surface shared by binaries and tests |
| `src/gui.rs`, `src/gui/` | egui app shell, worker bridge, messages, modals, theme, stickers, workspace |
| `src/gui/worker.rs` | Tokio runtime; `WorkerCmd` / `WorkerEvent` boundary between UI and network |
| `src/tui.rs`, `src/ui.rs` | ratatui terminal client |
| `src/chat.rs` | Message and thread operations against the Zalo chat API |
| `src/session.rs` | `session.json`, cookies, service map, request signing and params |
| `src/login.rs`, `src/qr.rs` | QR and password login flows |
| `src/socket.rs` | WebSocket transport, frame codec, push/pull command IDs |
| `src/param_cipher.rs` | `ParamCipher v2` request encoding |
| `src/crypto.rs` | AES-CBC/GCM helpers, MD5 `signkey` |
| `src/media.rs` | Attachments, images, stickers, voice payloads |
| `src/call.rs`, `src/media.rs`, `src/windows_video.rs` | VoIP signalling and audio capture/playback |
| `src/store.rs` | SQLite persistence (`messages`, `threads`) |
| `src/endpoints.rs` | Single source of truth for service hosts |
| `src/logger.rs`, `src/voip_log.rs` | Diagnostic logging and call tracing |

## Non-Negotiables

- **Never commit secrets.** `session.json`, `*.db`, cookies, `zpw_sek`, `zpw_enk`, IMEI and capture dumps stay out of Git (`.gitignore`, `SESSION_SECURITY.md`).
- **No blocking work on the UI thread.** Network, disk and crypto go through `WorkerCmd`; the UI only renders and sends commands.
- **All hosts live in `src/endpoints.rs`.** Never hardcode a Zalo host elsewhere.
- **Every outbound request goes through the standard path:** params → `ParamCipher v2` → `signkey` → session cookie attachment.
- **Comments in English, user-facing strings in Vietnamese.** See the comment taxonomy below.
- **Do not silently change protocol behaviour.** Any change to params, signing, frame IDs or cipher flow requires evidence (capture, note in `re-notes/`) and a test or documented manual verification.

## Engineering Standards

MUST read the corresponding standard file under `docs/standards/` before modifying related code or tests:

- **Issue tracking & tickets** → `docs/standards/issue-tracker.md`
- **Domain glossary & ADRs** → `docs/standards/domain-docs.md`
- **Comments & docstrings** → `docs/standards/code-comment-taxonomy.md`
- **API calls, IPC, and error handling** → `docs/standards/api-design-and-error-handling.md`
- **SQLite schema and migrations** → `docs/standards/database-and-migrations.md`
- **Threads, channels, locks, and races** → `docs/standards/concurrency-and-locking.md`
- **Tests, fixtures, and benchmarks** → `docs/standards/testing-and-fixtures.md`
- **Secrets, crypto, and session handling** → `docs/standards/security-and-cryptography.md`
- **Branches, commits, and PRs** → `docs/standards/git-flow-and-pr-matrix.md`

## Spec Kit pilot

For the remaining v0.007 work, read `docs/SPEC_KIT.md` and the relevant feature under
`specs/`; the release scope map is `docs/releases/v0.007-arm-plan.md`. Keep spec,
plan, tasks and verification consistent. Distinguish code, fixture checks and native
device confirmation; never mark the latter complete from unit tests. Existing
standards/ADRs and GitHub ticket lifecycle remain canonical. Use the current branch
and respect existing authorization; do not invoke automatic workflows or release
operations as a side effect of Spec Kit. Update command sources in
`.specify/templates/overrides/`, then regenerate skills with the pinned CLI.

## Karpathy-inspired coding discipline

Apply these principles when implementing, reviewing, or refactoring, alongside the
repository standards above. They are adapted from
[multica-ai/andrej-karpathy-skills](https://github.com/multica-ai/andrej-karpathy-skills),
reviewed at commit `2c606141936f1eeef17fa3043a72095b4765b9c2`.
These are community guidelines inspired by Karpathy, not a claim of his authorship
or endorsement. User instructions and existing protocol/security requirements take
precedence; this section does not authorize additional work or external actions.

1. **Think before coding.** State material assumptions and tradeoffs. Resolve routine
   choices from repository/session evidence and continue authorized work. Ask only
   when missing information materially affects correctness, scope or an irreversible
   decision; do not repeatedly seek approval already given. Distinguish source evidence,
   inference, test results and device acceptance.
2. **Simplicity first.** Implement the smallest coherent solution satisfying the agreed
   scope. Do not add speculative features, dependencies, configuration or abstractions.
   Keep required validation, account isolation and protocol/error handling intact.
3. **Surgical changes.** Preserve other work in the shared branch. Match local style,
   avoid unrelated rewrites, and remove only leftovers created by the current change.
   Broader cleanup is appropriate when explicitly requested; trace each change to that
   scope and report unrelated findings instead of silently altering them.
4. **Goal-driven execution.** Define observable success criteria, implement, and run
   relevant checks until resolved. For a bug, use a focused regression where practical;
   trivial documentation or reversible edits do not require invented tests. Scale
   checks to risk, report checks actually run and remaining limitations. A passing
   model/fixture test does not certify Zalo interoperability or native/device behavior.

These rules apply regardless of assistant vendor. Agent entry points and optional
skills delegate here rather than maintaining separate policies or task status.
See [multi-assistant setup](docs/AI_ASSISTANTS.md).
