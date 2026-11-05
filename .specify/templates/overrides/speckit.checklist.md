---
description: Create a requirement and verification checklist for an existing feature.
---

## User input

$ARGUMENTS

## Repository workflow

Read `AGENTS.md`, `.specify/memory/constitution.md`, and the relevant existing standards/ADRs first. Resolve the feature from an explicit repository-relative argument, otherwise `.specify/feature.json` (`feature_directory`), otherwise `.specify/project.json` (`default_feature`). Reject paths outside the repository. Read existing artifacts before editing; preserve their decisions, task IDs and evidence.

Use the current branch. Do not create/check out branches, rewrite existing work, commit, push, publish, contact people or invoke automated workflows as a side effect. Existing user authorization applies; do not add approval pauses between already authorized phases. Ask only about genuinely blocking missing information, and continue independent work. Do not infer authorization from this template.

Read the feature artifacts and create or update checklists/requirements.md. Check observable requirements, evidence provenance, data safety, account changes, cancellation, native-device validation and explicit exclusions. Link existing standards rather than duplicating their rules. Do not create a mandatory new approval gate.
