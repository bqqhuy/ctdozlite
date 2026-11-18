# MAP-26 / MAP-27 archive evidence and implementation

Parity with Zalo PC/Web (source: installed PC 26.11.12 `app.asar`, `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`). Evidence level E2; no live server acceptance claim.

## My Documents save

The installed renderer bundle is 21,691,083 bytes. At character offset 11867128, `SaveToMyCloudMenuItem` (`class Fe`) calls the chat controller `_shareMessage(e, V.default.sendToMeId, t, l.SHARE_FROM_MESSAGE)`. The handler checks content protection before sending. Its completion callback checks each result; an Error or object containing `error_code` is not success. The menu title uses `cloud_send2me.enable` to choose the localized My Documents/Cloud wording. Therefore saving an original message to My Documents is forwarding to the exact server-provided `sendToMeId`; it is not a local download or an invented separate save route.

The fork opens its existing two-step preview with the self-storage recipient bound to `own_cloud_id`, then uses the normal durable Forward payload, exact client identity and delivery reconciliation. Every selected parent is forwarded once. Missing self ID, source policy, unsupported forwarding or locked access prevents submission. Fixture checks do not certify native/live acceptance.

## Multiple attachments and destructive actions

Existing retained phone-history `meta.attachsList` is expanded into individual archive items while each item keeps its full original parent message. Source parent IDs are deduplicated before forward/delete/recall. Downloads retain each selected URL, filename and selection key. A multi-attachment parent cannot reliably renew one chosen child through the existing single-attachment renewal projection; that fallback is disabled rather than substituting a sibling. Clipboard actions carry the displayed URL explicitly. Delete and recall use the already implemented E2-backed `delete_message_for_me` / `recall_message`, a confirmation snapshot, serial execution, checked local persistence and a result for each parent, with no automatic retry after uncertain delivery.

## Local history pagination

Fork deviation because server archive filter/pagination completeness is not established for all retained message projections: the archive now reads bounded SQLite keyset pages `(ts,msg_id)` independently of the loaded timeline. Each page scans at most 200 rows (plus one lookahead); local filters intersect across scanned pages. Empty/expired pages still advance and expose Read older local history. Owner/runtime/thread/generation and hidden-access checks reject stale results. UI explicitly states local history coverage and does not claim a complete server archive.

`archive_history_paging_tests` checks tied timestamps, cross-thread exclusion, older rows beyond the timeline window, expiry, retained source metadata and EXPLAIN QUERY PLAN (`idx_thread_ts`). The measurement test prints deterministic CSV columns for 20 runs on 10,000 fake SQLite rows, comparing the previous 3,000-row timeline load against a 200-row archive page. Parent verification records the actual measurements and hardware/OS; no measurement has been fabricated here.

## Ordinary link-card forwarding

Fork deviation because ordinary link cards can use the already E2-backed link-send contract (chat.rs send_link_message_with_client_id; original PC 26.11.10 cmd11286/11193), while the PC26.11.12 generic forwarding service is a separate implementation. The projection preserves caption, href, title, description, thumbnail and source; no preview/scrape is invoked and durable identity is unchanged. Source protection, destination E2EE and normal worker policy remain mandatory. Unknown opaque webcontent without a valid ordinary HTTP(S) card remains unsupported. This does not claim generic PC forward-wire parity or native/live acceptance.

Regression extends the encrypted loopback request test through forward_message_with_client_id for BOTH direct/group targets, asserting all card fields and the original client ID. Unit tests reject unsafe/missing link destinations, calls and recalls. Outgoing persisted/rendered card keeps the same metadata.
