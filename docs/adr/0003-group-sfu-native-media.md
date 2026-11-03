# ADR-0003: Selective group SFU integration with native media

- **Status:** accepted
- **Date:** 2026-11-02
- **Deciders:** maintainer request to integrate upstream relay/SFU fixes

## Context

Upstream `23f06b7` (reviewed through `20e742c`) implements group relay initialization, SFU layer announcements/subscriptions and member-control handling. The fork previously refused outbound groups and uses Media Foundation instead of upstream FFmpeg. ADR-0001's group-call limitation is reopened for this implementation, without claiming verified live interoperability.

## Decision

Import group signalling/relay control and probe-selected relay routing while keeping signed session transport, redacted diagnostics, allocation validation and native codecs. Carry group mode explicitly into media rather than infer it from port 3000/3500. Keep call records separate from server-accepted messages. If group allocation fails, report failure without fanning out 1:1 calls to group members.

Advertise only the single 320x240 base layer actually encoded. Preserve upstream group RTP extension shape while deriving absolute-send-time from NTP. Separate Opus decoder state per audio SSRC and mix on a shared playout clock (at most 32 active sources). Decode at most two remote video sources independently; preview queues remain generation-bound and bounded. Keep receiving video when local camera is off.

## Evidence and verification

See `re-notes/09_group_sfu_arm64.md`: immutable upstream source references, sanitized packet fixtures, selected-relay/nested-callSetting tests, RTP extension and simultaneous-Opus tests. Native Windows CI must additionally validate codec/build/runtime; real group call interoperability with the official client remains a manual device/account requirement.

## Consequences and rollback

No new external executable or dependency. More than two remote videos are not rendered; there is no arbitrary-size group gallery. Live incoming group acceptance, membership changes and NAT behavior require manual validation. Revert this call-only commit to return to the previous unsupported-group behavior; account database format is unchanged by this decision.
