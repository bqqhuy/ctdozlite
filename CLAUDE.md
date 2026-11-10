# Repository entry point

Read [AGENTS.md](AGENTS.md) for the canonical repository instructions.
For Spec Kit work, read [docs/SPEC_KIT.md](docs/SPEC_KIT.md) and the
selected feature's spec.md, plan.md, tasks.md and verification.md. Do not maintain
a separate policy, task list or acceptance status for this agent.

The shared Karpathy-inspired coding discipline is in AGENTS.md. An optional
`karpathy-guidelines` skill is available under `.claude/skills/`; it delegates to
that same policy. No marketplace plugin or global installation is required.

## Claude Code model setup

- `.claude/settings.json` sets the main session to Sonnet 5.5 and the advisor to
  Fable 5.2. Follow **Advisor checkpoints** in AGENTS.md.
- Fable is preferred. If the advisor is off, or a call shows `Advisor unavailable`,
  tell the user and ask them to run `/advisor claude-opus-5-5` (Opus 5.5 fallback).
  New sessions return to Fable from the project settings.
- Read-only Haiku 4.5 subagents in `.claude/agents/`: `search` (project map),
  `grep` (definitions and call sites), `docs` (repository documentation). Delegate
  lookups to them. Planning, edits and verification stay in the main session.
