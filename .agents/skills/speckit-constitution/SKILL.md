---
name: speckit-constitution
description: Maintain a short index of existing repository policy.
compatibility: Requires spec-kit project structure with .specify/ directory
metadata:
  author: github-spec-kit
  source: override:speckit.constitution
---

# Speckit Constitution Skill

## User input

$ARGUMENTS

## Repository workflow

Read `AGENTS.md`, `.specify/memory/constitution.md`, and the relevant existing standards/ADRs first. Resolve the feature from an explicit repository-relative argument, otherwise `.specify/feature.json` (`feature_directory`), otherwise `.specify/project.json` (`default_feature`). Reject paths outside the repository. Read existing artifacts before editing; preserve their decisions, task IDs and evidence.

Use the current branch. Do not create/check out branches, rewrite existing work, commit, push, publish, contact people or invoke automated workflows as a side effect. Existing user authorization applies; do not add approval pauses between already authorized phases. Ask only about genuinely blocking missing information, and continue independent work. Do not infer authorization from this template.

Read the canonical repository standards and ADR index. Update .specify/memory/constitution.md only to keep references and project-specific status semantics accurate. Do not invent a second glossary or rewrite accepted ADRs. Do not synchronize or overwrite AGENTS.md automatically. Preserve user instructions and established authorization.
