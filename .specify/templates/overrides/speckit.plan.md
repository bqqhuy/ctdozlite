---
description: Plan a feature against the existing Rust architecture and protocol evidence.
---

## User input

$ARGUMENTS

## Repository workflow

Read `AGENTS.md`, `.specify/memory/constitution.md`, and the relevant existing standards/ADRs first. Resolve the feature from an explicit repository-relative argument, otherwise `.specify/feature.json` (`feature_directory`), otherwise `.specify/project.json` (`default_feature`). Reject paths outside the repository. Read existing artifacts before editing; preserve their decisions, task IDs and evidence.

Use the current branch. Do not create/check out branches, rewrite existing work, commit, push, publish, contact people or invoke automated workflows as a side effect. Existing user authorization applies; do not add approval pauses between already authorized phases. Ask only about genuinely blocking missing information, and continue independent work. Do not infer authorization from this template.

Read spec.md and the relevant re-notes. Update plan.md with affected modules, dependencies, migration/cancellation/account boundaries, validation and unresolved evidence. Reference ADRs instead of copying policy text. Record a new ADR only for a new architectural decision. Never turn a guessed endpoint or response into a confirmed contract.
