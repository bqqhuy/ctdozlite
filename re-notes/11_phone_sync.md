# Phone-approved transfer: independent implementation evidence

Date: 2026-11-02. No authenticated phone transfer was performed during development.

## Sources and scope

- Public Zalo Web shell and `sync-v2-worker.1b2d86e9567f6be024ce.js`, downloaded without cookies from the official chat site. The worker exposes protobuf modules `Ho1Y` (conversation), `1y5R` (messages), and wasm-bindgen ABI `Vox7`.
- Public official asset `libs/libzproto_wasm_bg.ecc0555ba9d0f8867a4699ce943ec745.wasm`: 291308 decompressed bytes, SHA-256 `3a868037a32739a4fcb3c22425150745613006f9dbc561f30fd705effaed55fa`. URL is centralized in `src/endpoints.rs`.
- Public interoperability research `ardennguyen/zalo-agent-cli` at `f867f0e992bea50d082c34cbe0637f209ee3a3c1` identifies the 590/591 control envelopes and empirical sync message-type numbers. Its newer implementation has CC BY-NC-SA terms; its JavaScript is not copied, translated or bundled in this fork. Rust transport, bounded protobuf parser, host ABI, storage integration and UI are independently written. Protocol facts do not establish a supported public Zalo API.

## Wire observations used

590/sub0/version1 is plain JSON inside the existing authenticated socket frame:
`{data:{syncId,syncType:0,ek,ik,toDevice:0,tempKey:"",deviceName,req,ver:1,ussidx:0},reqId}`.
`ik`/`ek` are base64 of 33-byte public keys including type prefix; private keys are 32 bytes. `req` carries `type`, `priority`, `batchSize:2000`, and queries with partition/from/to/limit. Conversation priority0 asks partition `""`; message priority2 asks `oneone/<opaque>` or `group/<opaque>` partitions, at most30 per round. The range starts at0 and ends at the request time; it cannot restore data the phone no longer holds.

601 controls have `content.act_type=transfer_sync2`, action `transfer_status` or `upload_batch`; nested data may be a JSON string. Status3 means waiting for approval,4 confirmed,5 inherited approval. Completion uses `isLast` or coverage of **all** message partitions. Conversation rounds need `isLast`. Each batch gives `batchType`, `idx`, signed `msgUrl`, scopes; metadata type2 precedes messages. 591 releases `{data:{syncId,toDevice:0,reason:1},reqId}` on cancellation/error/normal completion. Reconnect changes an epoch; a queued request is never replayed into the new socket.

Binary files contain little-endian u32 length-prefixed chunks. Metadata calls official `zprotoSync2DecryptMetadata(chunk,ik.public,ik.private,ek.private,None)` and yields plaintext/sessionRecord. Messages call `zprotoSync2DecryptMessage(chunk,sessionRecord)`. Decompressed protobuf supports Zstd, gzip, zlib or uncompressed data. A bounded decoder rejects truncated fields, invalid varints and resource exhaustion. Message.globalId is **signed int64 string**; timestamp/clientId are uint64. Numeric sync msgType is distinct from live API numeric msgType, so conversion is explicit and raw metadata is preserved.

Unknown group identity stays quarantined, never guessed by display name. User global IDs can be resolved with profile `/api/gid/decrypt`, encrypted params `{globalUids:JSON.stringify(ids),imei}`. Requests use the existing session's encoding/transport; no new unencrypted credential path.

## Verification boundary

`tests/fixtures/phone_sync_crypto_synthetic.json` contains synthetic keys/ciphertexts generated with the pinned official module entirely offline. The explicit asset test checks known-answer decryption plus independent fresh-key metadata/message roundtrips and authentication rejection. It does **not** prove server acceptance, phone prompt delivery, historical completeness, media URL lifetime or phone model compatibility. The physical MateBook/phone validation in ADR-0005 remains required.

## Range scheduling follow-up (2026-11-02)

The original implementation always requested `from:0`. The public wire schema already has `from`/`to`; the public reference carries `win.from` into message queries. The recent-first implementation now sends explicit ranges without changing command IDs, cipher calls or approval. A synthetic request test checks exact bounds and partition order. User reports confirm the original full transfer works; range filtering/resume on the real phone still needs manual verification. Coverage is recorded only after all batches in a completed round are imported, never inferred from existing min/max message timestamps. See ADR-0007 for overlap, cancellation and legacy database limits.

## Minimal-request policy follow-up (2026-11-02)

The previous “at most 30 per round” describes the reference/client batching policy,
not a demonstrated server ceiling. The maintainer explicitly requested grouped
queries, separating one sync initiation from multiple upload_batch/CDN files.
The client now groups ranges with a local 64 KiB serialized-payload budget and
uses the existing queries schema. It does not add commands or change crypto.

No captured numeric Zalo business code for “too many partitions” is available.
Tests use explicitly synthetic nonzero responses with reason strings
`too_many_partitions`, `too many partitions`, `too_many_conversations`,
`too many conversations`, or `payload_too_large`, optionally maxPartitions or
maxConversations. Only a pre-acceptance rejection in this allowlist can split;
auth/protocol errors, 429, generic errors and timeout cannot. This conservative
recognizer is not a claim that the live service emits those particular strings.
Actual device verification must record a redacted real rejection before adding
any new mapping. Rate-limit delay/cooldown uses retryAfter when supplied.

For larger grouped ranges, scopes alone no longer imply transfer completeness;
`isLast` for the matching syncId is required before checkpointing. Device testing
must verify this final marker and the allowed query count/payload. A missing final
marker stops at the existing timeout without checkpointing or auto-restarting.
