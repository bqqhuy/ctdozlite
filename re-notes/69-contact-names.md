# UID-scoped contact names — 2026-11-22

Direct reinspection in the continuing session found an additional alias-v2
branch: PC/Web select `/api/alias/list/v2` using `enable_feature_alias_v2`,
and disable the legacy phonebook flow/name getter in that branch. The fork
below implements the legacy source combination, not full feature-gated parity.
See [direct source audit](../docs/research/2026-11-22-direct-pc-web-android-analysis.md#2-alias-và-pbname-phải-phân-biệt-v1-với-v2)
and its separate CP/UTF-16/UTF-8 evidence index. The active account flag is unknown.

Parity with Zalo PC (E2, static bundle 26.11.12):
`%TEMP%/zalo-asar-analysis/pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`.
SHA-256: `78801014154A86BADE096EB916E541223A5F553F888882AA0EF3A218A44D6C08`.
Offsets below are UTF-16 character positions, not UTF-8 bytes.

- 15,120,549: `getPhonebookV2(e=0,t,n)` builds encrypted params
  `{phonebook_version:e}` for GET `/api/social/friend/phonebook/v2`
  on the configured profile domain. Optional `src` is only added when positive.
- 10,570,108: caller consumes decrypted data; `_handleSucess` reads `version`
  and `phonebook`. `_updateDataPhonebook` at 10,569,675 replaces the complete
  UID-to-string-name map, removes empty/self entries, and signals changed UIDs.
- 4,643,137: `getInfo(uid)` selects alias (subject to official feature flags/
  eligibility), then phonebook `pbName`, then public `zName`. `getDName(uid)`
  falls back to alias and phonebook if the mini-profile is unavailable.
- 4,642,076: group-member mini-profiles enter the same UID-scoped cache,
  preserving `aliasName`, `pbName`, and `zName` as distinct sources.

Fork implementation requests version zero for a full phonebook snapshot,
validates the entire map before replacing cached data, and retains the older
snapshot on failure. Alias and phonebook sources remain separate in account
cache and UI state. Existing presentation resolvers receive a derived map
with alias-over-phonebook precedence; alias-edit controls use only actual
aliases. Group messages resolve UID-scoped contact/profile names before the
wire display name and preserve the original message payload. Each completed
name source is published without waiting for unrelated account capabilities.

Limits: E2 proves request/response usage, not live account entitlement. Native
device acceptance, same-user phonebook example, server availability and
official alias feature-flag parity remain unverified. No API route alone was
used to infer the schema. No phonebook writes or automatic address-book upload
are implemented.
