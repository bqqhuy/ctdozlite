# ADR-0004: Account-scoped local conversation menu

- **Status:** accepted; mute flag superseded by [ADR-0013](0013-upstream-v006-selective-port.md)
- **Date:** 2026-11-02
- **Deciders:** maintainer request for conversation menu integration

## Decision

Expose local pin/unpin, notification mute/unmute, hide/show, mark read and confirmed local history deletion. Labels identify this-device behavior; these controls do not claim server-wide privacy or unread synchronization. Hidden threads remain recoverable through a visible toggle and are excluded from normal sidebar message search. They are not PIN-protected.

Persist preferences in the account SQLite database through the worker, independently of network thread snapshots. Schema 2 -> 3 adds `thread_preferences` transactionally without rewriting messages. Clear history and its checkpoint atomically, preserve thread identity/preferences, then update cached/UI snapshots after success. Later explicit server history sync may restore messages. Reject stale-owner commands and events across login boundaries.

## Verification and rollback

Tests cover account isolation, network refresh preserving preferences, real schema-2 upgrade, composite identity preservation and transactional deletion rollback. Previous schema-2 binaries refuse schema 3; do not downgrade user databases or discard user data. A rollback build must retain schema-3 reading/migration support while disabling the menu.
