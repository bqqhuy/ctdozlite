# Zalo PC 26.11.12 upload size limits and chunk configuration

Date: 2026-11-18. Evidence level: E2 (static renderer bundle). Same bundle as
`re-notes/48`: `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`, extracted at
`%LOCALAPPDATA%/Temp/ctdoz-map27-pc-26.11.12.js`, 21,691,083 bytes,
SHA-256 `78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08`.
Offsets are character offsets in the extracted JavaScript file.

## Configurations found

### `newFile` (offset 8,020,910)

- `maxFileSize: 1*2**30` = 1 GiB
- `webUploadFileE2eSizeLimit: 52428800` = 50 MiB
- `webDownloadFileE2eSizeLimit: 52428800` = 50 MiB
- `webUploadWasm: 1*2**30` = 1 GiB
- `webDownloadWasm: 1*2**30` = 1 GiB

### `multi_site_upload` (module `C8hE`, offset 4,281,977)

- `zcloud_upload_file_max_size: 1073741824` = 1 GiB
- `small_file_size_limit: 104857600` = 100 MiB
- `chunk_size: 3145728` = 3 MiB
- `part_size: 20971520` = 20 MiB
- Schema validator pins `zcloud_upload_file_max_size` at `min(1073741824)`.

### Limit resolution logic (offset 8,860,424)

```js
static get isWasmReady(){ return e2ee.enable_wasm && KweF.isWasmReady()
  && AesGcmWasmFactory.instance.isWasmInstalled() }
static getLimitSizeUpload(){ return isWasmReady
  ? Math.max(newFile.webUploadWasm, zcloud_upload_file_max_size)
  : newFile.webUploadFileE2eSizeLimit }
static checkValidFileSizeUpload(size, threadId){
  return !isOnE2ee(threadId) || Number(size) <= getLimitSizeUpload() }
```

`e2ee.enable_wasm: true` (offset 8,047,599).

## Interpretation and fork policy

- Normal threads: 1 GiB upload bound.
- E2EE threads: 1 GiB only with WASM crypto; 50 MiB on the JS fallback.
- These are client-side bounds read from PC config. The note does not
  establish server acceptance beyond what PC enforces.
- The fork ships native Rust AES-GCM instead of WASM. Until the encrypted
  file-upload packet format is captured and verified, the fork applies the
  conservative 50 MiB E2EE bound and refuses E2EE file uploads above it.
- `CHUNK` moves 1 MiB to 3 MiB to match `multi_site_api_config.chunk_size`;
  resumable checkpoints are keyed by `chunk_size` so old checkpoints are
  discarded safely on mismatch.

## Fork changes (this commit)

- `src/upload.rs`: `CHUNK = 3 MiB`, `MAX_UPLOAD_BYTES = 1 GiB`,
  `MAX_UPLOAD_E2EE_BYTES = 50 MiB`, `check_upload_size()`.
- `src/chat.rs` `send_attachment_with_client_id`: size check before any
  network work; E2EE threads use the stricter bound.
- Regression tests for normal, oversized, E2EE boundary and limit equality.
