# Restricted extensions and native executable ZIP — 2026-11-22

E2: direct static reads of installed Zalo PC 26.11.12. This corrects the
earlier incomplete assumption that only a manual ZIP workaround exists.
No protocol implementation or real-message transmission changed in this step.

## Source identities and zero-based code-point anchors

Fresh extraction root: `%TEMP%/zalolite-direct-audit-20261122/pc`.

- `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`, SHA-256
  `78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08`.
- `main-dist/mainless-worker.js`, SHA-256
  `c21a1e01c7784be483e5408f3e29efee62a027d2ee3616d5193bd8488edc09d9`.

| Anchor | Observation |
| --- | --- |
| Main CP 8056109 | `unsupportFile` defaults: exe, cmd, bat, com, lnk, vbs. |
| Main CP 7889410 | `settings.features.sharefile.restricted_ext_file` (actual JS uses `setttings`) extends the defaults and updates the downloader's invalid extensions. |
| Main CP 8059230 | `file.enable_file_eecx` defaults to true. Runtime configuration may change defaults. |
| Main CP 5008874 / 5030823 | Invalid original extension plus enabled EECX changes upload name to original name + `.zip`, and assigns executable-file subtype. |
| Main CP 1716056 | Message subtype EEXC = 24; file fType = 1, folder fType = 2; FILE.NORMAL = 1, FILE.EEXC = 2 in the separate fdata enum. |
| Main CP 7326022 | Upload task invokes native `fileEexc` with original path and temporary directory. |
| Worker CP 326569 | `fileEexc` uses ZIP archiver with zlib level 0; appends a stream of the original file under sanitized original basename, finalizes, and returns actual archive path/size. It creates real archive bytes, not merely a renamed EXE. |
| Main CP 14545998 | Replaces upload path/size with archive result and sets `fdata: {type: FILE.EEXC}`. |
| Main CP 8512325 / 8514614 | Incoming EEXC preprocessing clears localPath so the original executable is not mistaken for the downloaded ZIP. This is not evidence of automatic extraction. |
| Main CP 3780000 | ZIP/unzip folder path is gated by `MSG_FOLDER`; keep this separate from executable wrapping. |
| Main CP 19625018 | API error enum explicitly names code 204 `FILE_EXTENSION_NOT_ALLOWED`. |

The downloaded Web bundle contains related call sites, but shared desktop-only
branches in that bundle do not establish that a browser can execute the same
native worker. PC native implementation is the stronger evidence here.

## Findings and limits

The official PC client has automatic ZIP preparation for restricted extensions
when the feature is enabled. ZIP level 0 provides packaging with little/no size
reduction. It is not the separate local-cache noise/XOR transformation, and the
existence of that noise code does not prove wire upload obfuscation.

The six defaults are not an exhaustive server denylist. Live fork observations
in note 71 show EXE and MSI refused at upload with 204; MSI is absent from the
default six. The effective server list, account config and any additional
server checks remain unknown. PS1 and ZIP were accepted in the local trial.

No receiver branch inspected here establishes automatic executable extraction.
Do not implement generic ZIP extraction based only on extension or fdata type.
The fork's current explicit ZIP draft is useful but is not full PC parity:
automatic preparation, config hydration, archive naming and EEXC metadata differ.

## Next live comparison

The user plans to select an uncompressed EXE on another official client and
send it into this account. When the user reports arrival, inspect only that
test message: wire subtype, fType, fdata, displayed/original filename, uploaded
size and archive signature. Download without executing it. Compare extracted
entry size/SHA-256 against the sender's original, and separately inspect what
the official receiver saves (ZIP or EXE). Record sender platform/version and
whether the sender selected the original EXE or a prebuilt archive.

No background watch or automatic future upload/download is configured.

## User observation after official-client trial

On 2026-11-22 the user reported that every EXE they selected in Zalo PC was
automatically sent as ZIP. This corroborates the sender-side static path above;
it is a user observation, not a saved wire capture or proof of receiver extraction.
The user also confirmed immediate filename/icon bubbles preceding upload/send
status updates. The fork's batch worker was found to await local persistence
before publishing its initial event, and to prepare files sequentially before
announcing later ones. Those waits should not delay the visible send intent.
