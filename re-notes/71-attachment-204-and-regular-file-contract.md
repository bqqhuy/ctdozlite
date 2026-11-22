# Attachment rejection 204 and regular-file metadata — 2026-11-22

Parity with Zalo PC/Web for regular-file metadata; explicit fork deviation for
an optional local ZIP-to-draft action. No automatic send, rename or retry.

## Direct evidence

Sources freshly extracted/read in the current chat:

- PC 26.11.12 main bundle SHA-256
  `78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08`.
- PC `lazy/lang-vi.bd6430df3fbb9dc6dc93.js`, code-point offset 302201:
  `STR_SEND_MSG_ERR_204` describes an unsupported file format. English locale
  independently maps the same key to unsupported file type.
- Main bundle `k3JU`, CP 16560059: normal file = 1, folder = 2.
  `DLcZ.getFileAsyncItem`, CP 4552015, uses the file's `fType`; 2 is not a
  generic video marker. The Web main bundle has the same metadata builder.
- PC normal upload builder around CP 3975962 constructs numbered chunks and
  the file uploader uses `asyncfile/upload`. Its `waitAsyncFileInfo` resolves
  uploaded URLs before `sendMsgFileAsync` submits `asyncfile/msg`.

Offsets are Python code points, zero-based, not UTF-16/UTF-8 byte positions.
Extraction root and bundle identities are recorded in
`docs/research/2026-11-22-direct-analysis-evidence.json`.

## Local observations, not a new traffic capture

The app's bounded diagnostics contain eight `send_failure` events with
`failure=server_rejected`, `server_code=204`, `http_status=null` in the current
log. They do not identify upload versus message submission.

A read-only query of recent persisted outgoing-job metadata found six failed
attachment jobs reporting 204: five `.exe`, one `.msi`, all direct chats.
Only aggregate extension/count/group values were emitted. No filenames,
destinations, messages, cookies or keys were copied into this note.

This is evidence for the retained examples, not proof that every attachment
failure has the same cause or that all accounts share an extension blacklist.

## Corrections and limits

- Keep rejection 204 explicit and explain unsupported format in Vietnamese.
- Optional `Tạo ZIP…` packages the original bytes locally, stages a new ZIP
  draft under the owning account/thread, and requires the user's Send action.
  No source execution, silent extension change or automatic network mutation.
- Use `fType=1` for ordinary-file fallback, including MKV/WebM/AVI and native
  video preparation fallback. Preserve source bytes, client identity and the
  two-stage upload/URL/submit order.
- Add typed `attachment_failure` stages for photo/file upload and submission;
  logs accept only enums and numeric status codes, never request payloads.
- ZIP writer: pinned `zip=2.4.2`, default features off, Stored entries. This is
  packaging, not encryption or size reduction. Existing one-hour flat temp-file
  pruning/account logout cleanup applies; failed original jobs remain intact.

Neither locale strings nor synthetic fixtures prove real-account delivery of
ZIP or every codec/file format. Native/live acceptance remains separate.

## Live follow-up, 2026-11-22, approximately 10:42–10:44 local time

The user authorized tests with existing Downloads files in their own Cloud.
The new ARM64 GUI displayed a sent 18.7 MB ZIP. Read-only local state then
confirmed server-accepted PS1 and ZIP messages, with server references and
download metadata. The latter ZIP was generated from the rejected MSI.
Two new allowlisted diagnostic events report `file_upload`, `server_code=204`,
`http_status=null`, corresponding to installer attempts (EXE/MSI). This
locates those refusals before file-message submission; it does not justify
changing the submit contract or silently retrying uploads.

These are live fork observations, not an official-client traffic capture.
The official PC client was logged out, so cross-client display/download and
downloaded-byte equality are not yet verified. During the interactive check
the user took over the GUI and asked the assistant to monitor logs only;
subsequent sends must be attributed to that manual test, not automated clicks.
No account identifiers, download URLs, credentials or file payloads are saved here.
