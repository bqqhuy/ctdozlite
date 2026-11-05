# ctdozlite — Spec Kit constitution

Version: 1.0.0. Adopted: 2026-11-05 (Asia/Saigon).

## Canonical policy

This is an index, not a replacement policy set. User instructions take precedence.
Start with the [repository guide](../../AGENTS.md).

- [AGENTS.md](../../AGENTS.md): architecture, worker boundary and protocol rules.
- [GLOSSARY.md](../../GLOSSARY.md): terminology and INV-N declarations.
- [Standards](../../docs/standards/): read the relevant standard before edits.
- [ADR index](../../docs/adr/README.md): accepted architectural decisions.
- [Protocol evidence](../../re-notes/): observed PC/Web contracts and unknowns.

Never copy the whole standards/glossary into a new policy or rewrite an accepted ADR.

## Feature and evidence semantics

Resolve the feature from an explicit path, a checkout-local `.specify/feature.json`,
or the tracked default in `.specify/project.json`. No branch creation is needed.
Scope is defined in spec.md; plan.md references architecture; tasks.md records work;
verification.md records results and limitations. Release notes link these artifacts.
GitHub issue/PR states remain canonical for ticket lifecycle, as defined in the
issue-tracker standard; local task checkboxes are implementation details, not labels.

Use these evidence levels, with no automatic promotion:

| Status | Meaning |
| :--- | :--- |
| chưa làm | No implementation of this task is present. |
| một phần | Some required behavior is missing or unresolved. |
| đã có mã | Implementation exists, acceptance evidence is still missing. |
| qua kiểm thử | Named automated/fixture checks passed for the stated scope. |
| xác nhận trên thiết bị | A recorded native-device scenario passed on the stated build. |

A passing fixture is not a successful iPhone transfer, a complete group permission
model, or a native Windows UI test. Keep protocol hypotheses distinct from official
source observations and real traffic. Record the source, date, command/scenario,
outcome, ignored tests and limitations. Never store credentials or user messages.

## Authorized progress

Proceed through already authorized work without a new approval gate per document.
When one task is blocked, record why and continue independent authorized work.
Spec Kit does not authorize commits, pushes, releases, new account mutations,
external communication, or changes outside the user's requested scope. No automated
workflow/event dispatch or community extension is enabled by this pilot.
