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
