---
name: speckit-analyze
description: Review consistency of requirements, tasks, code and evidence.
compatibility: Requires spec-kit project structure with .specify/ directory
metadata:
  author: github-spec-kit
  source: override:speckit.analyze
---

# Speckit Analyze Skill

## User input

$ARGUMENTS

## Repository workflow

Read `AGENTS.md`, `.specify/memory/constitution.md`, and the relevant existing standards/ADRs first. Resolve the feature from an explicit repository-relative argument, otherwise `.specify/feature.json` (`feature_directory`), otherwise `.specify/project.json` (`default_feature`). Reject paths outside the repository. Read existing artifacts before editing; preserve their decisions, task IDs and evidence.

Use the current branch. Do not create/check out branches, rewrite existing work, commit, push, publish, contact people or invoke automated workflows as a side effect. Existing user authorization applies; do not add approval pauses between already authorized phases. Ask only about genuinely blocking missing information, and continue independent work. Do not infer authorization from this template.

Perform a read-only cross-check of spec.md, plan.md, tasks.md, verification.md and the release scope map. Report omissions, contradictory statuses, duplicated sources of truth and unsupported protocol assumptions with file references. Do not edit artifacts or mark tasks complete during analysis.
