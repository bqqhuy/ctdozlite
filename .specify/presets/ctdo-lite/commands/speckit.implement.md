---
description: Implement authorized tasks while preserving account and protocol invariants.
---

## User input

$ARGUMENTS

## Repository workflow

Read `AGENTS.md`, `.specify/memory/constitution.md`, and the relevant existing standards/ADRs first. Resolve the feature from an explicit repository-relative argument, otherwise `.specify/feature.json` (`feature_directory`), otherwise `.specify/project.json` (`default_feature`). Reject paths outside the repository. Read existing artifacts before editing; preserve their decisions, task IDs and evidence.

Use the current branch. Do not create/check out branches, rewrite existing work, commit, push, publish, contact people or invoke automated workflows as a side effect. Existing user authorization applies; do not add approval pauses between already authorized phases. Ask only about genuinely blocking missing information, and continue independent work. Do not infer authorization from this template.

Read spec.md, plan.md, tasks.md and verification.md. Execute authorized unblocked tasks and update evidence as work proceeds. Record a blocker and continue independent tasks instead of stopping the whole release. Use the existing worker boundary and replay-safe store. Run appropriate checks; record failures and ignored tests. Never mark a device test complete from unit tests. Report remaining partial requirements without renaming them as done.
