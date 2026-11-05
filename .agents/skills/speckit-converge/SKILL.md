---
name: speckit-converge
description: Assess implementation convergence using evidence, including native-device
  gaps.
compatibility: Requires spec-kit project structure with .specify/ directory
metadata:
  author: github-spec-kit
  source: override:speckit.converge
---

# Speckit Converge Skill

## User input

$ARGUMENTS

## Repository workflow

Read `AGENTS.md`, `.specify/memory/constitution.md`, and the relevant existing standards/ADRs first. Resolve the feature from an explicit repository-relative argument, otherwise `.specify/feature.json` (`feature_directory`), otherwise `.specify/project.json` (`default_feature`). Reject paths outside the repository. Read existing artifacts before editing; preserve their decisions, task IDs and evidence.

Use the current branch. Do not create/check out branches, rewrite existing work, commit, push, publish, contact people or invoke automated workflows as a side effect. Existing user authorization applies; do not add approval pauses between already authorized phases. Ask only about genuinely blocking missing information, and continue independent work. Do not infer authorization from this template.

Compare every requirement with code, task state and verification.md. Inspect diffs and relevant tests, record contradictions, missing behavior and unverified hypotheses. Fix authorized issues where feasible. Report code convergence separately from release/device acceptance. An unresolved required device scenario or protocol gap means release readiness remains incomplete. Do not build a release or publish as a side effect.
