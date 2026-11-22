# Composer remote config — 2026-11-22

Direct reinspection found the legacy/column distinction and `WJhq` geometry:
legacy normal maximum is 5.5 * 22.5 px; the expanded ratio is 2/3 of the column.
The column CSS fallback is 66.667%, not a universal half-window limit.
The fork's five-line/half-chat policy below follows user observations and is
not proof of full official editor/rollout parity. See
[direct source audit](../docs/research/2026-11-22-direct-pc-web-android-analysis.md#3-composer-ba-khái-niệm-hình-học-khác-nhau)
for feature gates, box definitions and reproducible source anchors.

Parity with Zalo PC 26.11.12, E2 static evidence. Source:
`%TEMP%/zalo-asar-analysis/pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`,
SHA-256 `78801014154A86BADE096EB916E541223A5F553F888882AA0EF3A218A44D6C08`.
Offsets are UTF-16 character positions. No live response or credentials were read.

- 15,047,534: `getServerInfo` calls the auth-domain login endpoint with plain
  signed `imei,type,client_version,computer_name,sp_mtn,bkt` (optional extras).
- 7,941,729: success envelope unwraps `error_code=0` to `data`.
- 7,913,211: server-info handler emits `data.setttings.features` (the source
  spelling has three t letters). Login/auth additionally emits
  `payload.config.settings.features` at 14,634,787.
- 20,077,505: Q7+W cache singleton listens to both feature events; I9Fb stores
  an in-memory object. 7plT/8zvx select the `zcinput` key and validate/merge it.
- 6,122,155: HKtk validates version/style/behavior, and freezes the configuration
  used for the session. Version-mismatching style/behavior fall back.
- 15,579,316: gpmR defaults are version 1, lineHeight 22, inputMaxLines 12.5,
  rtfInputMinLines 1; feature flags default to zero.

Fork reads only typed geometry from the existing startup getServerInfo response
and `apply_login_config`'s existing auth configuration. The latter stores the
typed subset in Session and carries it in LoginSuccess, applying it after
the account reset so fresh login receives config without another request.
It does not add requests, polling, storage of the full response or automatic
feature enablement. Events carry account and runtime identity, checked by UI.
Malformed fields retain defaults; finite bounds (lines 1–50, line height 12–48)
are explicit fork rendering safeguards, not inferred server limits.

Normal input remains capped at five visible lines and expanded mode uses half
the available chat geometry, per user-confirmed native observations. Remote
inputMaxLines applies only to rich mode. Defaults alone do not prove the active
native implementation or account feature flags. Existing request failure or
absent/unrecognized config retains defaults. Socket updates and live/native
parity remain outside this minimal read.
