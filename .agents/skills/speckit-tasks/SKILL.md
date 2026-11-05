---
name: speckit-tasks
description: Maintain requirement-linked tasks and explicit verification levels.
compatibility: Requires spec-kit project structure with .specify/ directory
metadata:
  author: github-spec-kit
  source: override:speckit.tasks
---

# Speckit Tasks Skill

## User input

$ARGUMENTS

## Repository workflow

Read `AGENTS.md`, `.specify/memory/constitution.md`, and the relevant existing standards/ADRs first. Resolve the feature from an explicit repository-relative argument, otherwise `.specify/feature.json` (`feature_directory`), otherwise `.specify/project.json` (`default_feature`). Reject paths outside the repository. Read existing artifacts before editing; preserve their decisions, task IDs and evidence.

Use the current branch. Do not create/check out branches, rewrite existing work, commit, push, publish, contact people or invoke automated workflows as a side effect. Existing user authorization applies; do not add approval pauses between already authorized phases. Ask only about genuinely blocking missing information, and continue independent work. Do not infer authorization from this template.

Read spec.md, plan.md and verification.md when present. Update tasks.md in dependency order; retain stable task IDs. Each task names requirements, files, status and required evidence. A checked box means that particular task's stated acceptance passed, not that the whole feature is complete. Keep device acceptance tasks unchecked until observed. Do not create external issues automatically; GitHub issue/PR status remains governed by docs/standards/issue-tracker.md.
