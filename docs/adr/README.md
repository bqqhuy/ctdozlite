# Architecture Decision Records

One file per decision: `NNNN-<kebab-name>.md`, numbered from `0001`, never renumbered.

## Template

```markdown
# ADR-0001: <Decision title>

- **Status:** proposed | accepted | superseded by ADR-NNNN | deprecated
- **Date:** YYYY-MM-DD
- **Deciders:** <names/roles>

## Context

Forces at play, constraints, evidence (capture note, upstream behaviour).

## Decision

What we will do, in one paragraph, present tense.

## Consequences

- Positive:
- Negative:
- Follow-ups:

## Alternatives Considered

| Option | Why rejected |
| :--- | :--- |
```

## Index

| ADR | Title | Status |
| :--- | :--- | :--- |
| [0001](0001-windows-arm64-runtime.md) | Windows 11 ARM64 runtime and upstream integration | accepted |

| [0002](0002-bounded-media-and-durable-message-state.md) | Bounded media resources and durable message state | accepted |
| [0003](0003-group-sfu-native-media.md) | Selective group SFU integration with native media | accepted |
| [0004](0004-local-conversation-menu.md) | Account-scoped local conversation menu | accepted |

| [0005](0005-phone-approved-history-sync.md) | Phone-approved history sync | accepted |
| [0006](0006-exact-message-identity-and-quote-navigation.md) | Exact message identity and quote navigation | accepted |
| [0007](0007-recent-first-resumable-phone-sync.md) | Recent-first resumable phone sync | accepted |
| [0008](0008-fail-closed-mutations-and-background-unread.md) | Fail-closed mutations and coalesced background unread delivery | accepted |

Related domain language lives in [`/GLOSSARY.md`](../../GLOSSARY.md).

- [0009 — Outgoing retry, streaming uploads and Windows popups](0009-outgoing-retry-streaming-and-windows-popup.md)
- [0010 — Minimal-request phone sync](0010-minimal-request-phone-sync.md)
- [0011 — Visible media priority](0011-visible-media-priority.md)
- [0012 — Account tools and schema 6](0012-account-tools.md)
- [0013 — Selective upstream v0.0.6 port](0013-upstream-v006-selective-port.md)

- [0014 — Conservative pipelined phone sync](0014-conservative-pipelined-phone-sync.md)
- [0015 — Native notifications and safe media presentation](0015-native-notices-and-media-presentation.md)
- [0016 — Upstream v0.0.6 UI and bundled GIFs](0016-upstream-v006-ui-and-bundled-gifs.md)
- [0017 — Complete phone transfer lifecycle](0017-complete-phone-transfer-lifecycle.md)
- [0018 — Finite media recovery](0018-finite-media-recovery.md)
- [0019 — History tools and ARM64 updater](0019-history-tools-and-arm64-updater.md)
- [0020 — Bounded queue catch-up](0020-bounded-queue-catchup.md)
- [0021 — Drafts and delete replay markers](0021-drafts-and-delete-replay-markers.md)
- [0022 — Resumable control queue drain](0022-resumable-control-queue-drain.md)
- [0023 — Phased, parallel and resumable phone sync](0023-phased-parallel-resumable-phone-sync.md)
- [0024 — Update-only sync, media renewal and selected PR #78 features](0024-update-only-sync-and-selected-pr78.md)
- [0025 — Video messages and media forwarding like Zalo PC](0025-video-messages-and-forward-like-zalo-pc.md)
- [0026 — Persisted queue cursors](0026-persisted-queue-cursors.md)

- [0027 — v0.003-arm desktop UI and release identity](0027-v003-arm-desktop-ui.md)

- [0028 — v0.004-arm selected upstream improvements](0028-v004-arm-selected-upstream.md)

- [0029 — Scoped chat file-drop zones](0029-chat-file-drop-zones.md)

- [0030 — Indexed local search and explicit server state](0030-v006-search-and-server-state.md)

- [0031 — Group policy, message lifecycle and explicit restoration](0031-v007-group-policy-and-restoration.md)

- [0032 — Lightweight Spec Kit pilot](0032-lightweight-spec-kit-pilot.md)

- [0033 — V1 RSA backend after release audit](0033-v1-rsa-backend-release-audit.md)

- [0034 — Hidden-chat PIN compatibility and local display gate](0034-hidden-chat-pin-compatibility.md)
