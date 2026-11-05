---
description: Define a bounded feature specification without replacing existing requirements.
---

## User input

$ARGUMENTS

## Repository workflow

Read `AGENTS.md`, `.specify/memory/constitution.md`, and the relevant existing standards/ADRs first. Resolve the feature from an explicit repository-relative argument, otherwise `.specify/feature.json` (`feature_directory`), otherwise `.specify/project.json` (`default_feature`). Reject paths outside the repository. Read existing artifacts before editing; preserve their decisions, task IDs and evidence.

Use the current branch. Do not create/check out branches, rewrite existing work, commit, push, publish, contact people or invoke automated workflows as a side effect. Existing user authorization applies; do not add approval pauses between already authorized phases. Ask only about genuinely blocking missing information, and continue independent work. Do not infer authorization from this template.

Update or create `spec.md` using the resolved spec template. Describe observable behavior, requirement IDs, acceptance scenarios, exclusions and unknowns. Keep confirmed PC/Web evidence separate from hypotheses. Existing feature requests already approved by the user are inputs, not questions to repeat. Never claim that writing a spec implements it.
