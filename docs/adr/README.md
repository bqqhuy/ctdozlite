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

Related domain language lives in [`/GLOSSARY.md`](../../GLOSSARY.md).
