# Ma trận audit parity toàn miền Zalo PC — 2026-11-19

## Phạm vi và phương pháp

Đây là nhánh audit parity độc lập. Baseline source là `main` tại `50d6abe`,
cộng working diff đang có trong workspace; không gọi API, không dùng cookie/session,
không thực hiện mutation trên tài khoản và không chép mã bundle vào repository.

Bundle chính: Zalo PC 26.11.12 tại
`C:\Users\bqqhu\AppData\Local\Programs\Zalo\Zalo-26.11.12\resources\app.asar`,
SHA-256 `4c3a7aa9b8fb409a95c6739ef9dccdc01127d0dc4f6383ee396400b7f94484c5`.
ASAR header cho biết 13.169 entry: 12.736 PNG, 118 JS, 23 `.node` và các loại
asset khác. Các JS lớn/quan trọng gồm renderer `compact-app-pc` (21,69 MB),
`sync-v2-sub-worker` (21,95 MB), `search-worker` (21,92 MB), `zd-worker`
(8,00 MB), `shared-worker` (0,43 MB), `main-dist/main.js` (4,37 MB), preload
SQLite (2,84 MB) và Electron preload/bridge. Có 454 literal `/api/...` khác
nhau theo lần quét trước. Đây là inventory/heuristic, không phải 454 API hoạt
động hoặc đã giải schema.

Đã đối chiếu các note `re-notes/01`, `02`, `06–09`, `12–22`, `28–49`,
`docs/ZALO_REFERENCE_ANALYSIS.md`, `docs/ZALO_FEATURE_API_GAPS.md`, master
action plan và checklist Phase 3–4 với tên module/đường xử lý fork. Kết luận
bundle dưới đây là **E2**. Re-notes có một số E1 riêng lẻ nhưng lượt audit này
không tạo capture E1 mới. Nội dung cũ trong roadmap/gap docs được xem là lịch
sử cho tới khi kiểm lại source hiện tại.

## Kiến trúc đối chiếu

| Hệ thống | PC 26.11.12 | Fork | Mức kết luận |
| --- | --- | --- | --- |
| UI/runtime | Electron + Chromium renderer, main/preload/context bridge, worker riêng | Rust/egui; Tokio worker; SQLite; Win32/COM ở một số media path | Fork cố ý khác runtime. Chỉ so hành vi/interaction; không yêu cầu nhái Electron nội bộ. |
| Host/protocol | Đọc `getServerInfo`, service map, ParamCipher/signkey; nhiều HTTP service và sync workers | `session.rs`, `service_resolver.rs`, `endpoints.rs`, `param_cipher.rs`, `socket.rs` | Tĩnh cho biết route, command, caller; không đủ khẳng định entitlement, schema server hiện tại hay thành công với tài khoản. |
| Lưu trữ | SQLite/native bridge, worker/preload và một số local cache | SQLite Rust, data root/account namespace, DPAPI/session persistence | Cùng mục tiêu quan sát được, khác định dạng/runtime; cần kiểm tra lifecycle và import/export theo từng workflow. |

## UI paths with contract gaps

- **Todo (visible, active call path):** `gui/tools_panel.rs` sends `CreateTodo` and
  `FetchTodos`; worker calls `todo.rs` `/api/group/todo/create|list`. The PC bundle
  registers personal todo commands under `/api/board/personal/todo/*` (route-index
  offsets 2,530,968–2,531,299; command 12413, renderer byte 15,183,690). This is a
  source-level domain/contract mismatch, not proof the server rejects the fork's
  route; personal vs group semantics are not interchangeable. Remove the parity
  label or gate the visible actions until a full PC caller/schema and matching fork
  evidence exists. Do not rewrite the group feature as personal Todo by guesswork.
- **Business product catalog (visible, active call path):** `gui/tools_panel.rs`
  sends `GetBusinessCatalog`; worker calls `business.rs::fetch_product_catalog`.
  Installed PC registers `/api/prodcatalog/catalog/list` (12038) and
  `/api/prodcatalog/product/list` (12039), and `getCatalogDomain` uses a distinct
  `CATALOG` service (renderer byte 15,198,245). Fork currently resolves product/list
  through `profile`. This proves a service-map mismatch in source, not live failure
  or entitlement availability. Gate it or fix only after matching the full PC
  caller and response schema; the UI must retain the OA/business entitlement note.

## Coverage theo miền chức năng

| Miền | Bằng chứng PC đã tìm thấy | Đường xử lý trong fork cần đối chiếu | Kết quả / còn mở |
| --- | --- | --- | --- |
| Khởi động, đăng nhập, QR, session | `re-notes/01_login.md`: getServerInfo, password v2, reqqr, wait-scan/status, getLoginInfo, logout; E1 chỉ cho những lượt ghi rõ live-verified | `login.rs`, `qr.rs`, `session.rs`, `profiles.rs`, `storage.rs` | Nền luồng đăng nhập được ghi nhận; account/device management như mật khẩu, 2FA, activity list không đồng nhất với login/logout. Cần so từng bước QR/session ở thiết bị hiện tại. |
| Service discovery và giao thức | `re-notes/02_chat_data.md`, `17`, `29`, `ZALO_REFERENCE_ANALYSIS.md`; dynamic service map, request cipher/signing, worker command classes | `endpoints.rs`, `service_resolver.rs`, `param_cipher.rs`, `crypto.rs`, `socket.rs` | Có parity kiến trúc cho request chuẩn và service map; không suy route literal thành server contract. WebSocket framing/heartbeat còn mục AMBIGUOUS theo reference authority. |
| Sidebar, contacts, alias, presence, unread | `re-notes/02`, `32`, `34`, `35`, `45`; profile/friend lookup, conversation classification, unread/presence/alias behavior | `gui/contacts.rs`, `profile.rs`, `account_metadata.rs`, `labels.rs`, `inbox.rs`, `presence.rs`, `unread.rs`, `gui/workspace.rs` | Cần rà live privacy và đồng bộ alias/unread giữa thiết bị. Gap inventory vẫn liệt kê các workflow friend/block/report/server archive/hidden/quick reply chưa đủ sâu; không lấy code nền làm parity hoàn chỉnh. |
| Tin nhắn, nhận dạng, reply, reaction, recall, forwarding | `re-notes/12`, `18`, `28`, `29`, `46`; client builder/normalizer, message identity, quote/mention, async media, forward preparation | `chat.rs`, `outgoing.rs`, `message_model.rs`, `message_lifecycle.rs`, `reaction_pipeline.rs`, `reactions.rs`, `message_view.rs` | Hợp đồng đã có note/fixture khá chi tiết. MAP-28 link card có fork deviation ghi ở `re-notes/47`. Native interaction và live ACK/echo chưa chứng nhận. |
| Sync realtime và phục hồi hàng đợi | `re-notes/15`, `16`, `17`, `19`, `20`, `21`; `shared-worker` chứa `checkRequestfulfilled`, `takeUndeliveredScopes`, batch/isLast/cursor behavior | `socket.rs`, `socket/queue_receiver.rs`, `history_sync.rs`, `queue_catchup.rs`, `queue_cursors.rs`, `live_updates.rs`, `mute_sync.rs` | Có so sánh thuật toán hàng đợi và regression. Tính đúng với frame/account đang hoạt động, reconnect thật, replay và missed push còn cần thiết bị/capture. |
| Tìm kiếm, stranger inbox | Search worker riêng; `re-notes/32`, `docs/ZALO_FEATURE_API_GAPS.md` A01 và route index | `search.rs`, `gui/search_panel.rs`, `inbox.rs` | Chưa truy toàn call graph của search worker trong lượt này. API existence không chứng minh filter/ranking/privacy parity; stranger classifier cần xác nhận trên PC/live. |
| File, ảnh, video, sticker/GIF, upload | `re-notes/15`, `18`, `28`, `48`, `49`; async upload, renewLink, chunk config 3 MiB, local/video preview, client predownload gates 30 MiB file/100 MiB video, zCloud flow riêng | `media.rs`, `upload.rs`, `media_download.rs`, `media_renew.rs`, `media_resources.rs`, `zcloud.rs`, `video_player*.rs`, `gui/stickers.rs` | Nhiều luồng có source evidence; PC default predownload off và có preference/size gates. Cần chứng nhận size provenance, mọi extension/preview, upload progress, expired URL, ARM64 codecs và zCloud credential flow thật. |
| Nhóm/cộng đồng, quyền, poll/note/todo | `re-notes/30`, `31`, `34`, `35`, `45`; group type, lockSendMsg, role checks, group-board/poll APIs | `groups.rs`, `group_metadata.rs`, `group_state.rs`, `group_entry.rs`, `group_panel.rs`, `poll.rs`, `todo.rs`, `gui/tools_panel.rs` | Group/community phải phân biệt theo metadata/role; không suy qua tên/size. Gap A12–15/B03–04 vẫn ghi thiếu quản trị/entitlement sâu; poll/todo mới không chứng minh workflow tương đương. |
| Cuộc gọi, audio/video và history | `re-notes/06–09`, 48 và báo cáo `pc-parity.md`; call actions, enum, `zcall` wrapper | `call.rs`, `call_lifecycle.rs`, `windows_video.rs`, `voice.rs`, `media.rs`, `message_view.rs` | Call card/parser được đối chiếu E2. Không có API history độc lập được chứng minh. Cross-device answer/dismissal, two-way media, AEC/NS/AGC, camera, reconnect và sharing cần native/E1. |
| Bảo mật, privacy, E2EE | `re-notes/36–43`; client UI/policy and E2EE routes; PC UI/settings evidence giới hạn | `crypto/`, `e2ee_integration.rs`, `gui/auth_guard.rs`, `privacy.rs`, `policy_panel.rs`, `session.rs` | HTTPS/ParamCipher không phải E2EE. Fork engine thử nghiệm không được xem là interoperable chỉ vì bundle có route. Cần lifecycle/key interoperability; privacy setting mutation/Hello phải test native. |
| Lịch sử, archive, media store, My Documents/zCloud | `re-notes/47–49`; PC archive picker stages date range/confirm/cancel, groups photos by local day; cloud viewer routes riêng | `history_open.rs`, `history_sync.rs`, `store.rs`, `server_media.rs`, `zcloud.rs`, `gui/workspace.rs` | Fork archive hiện được xác định rõ là local SQLite, không tương đương server-complete history. Cloud drawer không chứng minh cùng My Documents/zCloud entitlement/usage. Destructive media paths cần controlled account. |
| Cài đặt, notification, window/native shell | `re-notes/48`, renderer + `main-dist`/preload; notification click/show, SQLite bridge, native module families | `AppSettings` trong `gui.rs`, `account_controls.rs`, `notification_sound.rs`, `tray.rs`, `platform.rs`, `gui/modals.rs`, `gui/theme.rs` | Có so sánh một số setting/UI và binary wrappers; chưa đọc đủ từng màn hình, notification settings, keyboard/menu và DPI. Native wrapper/binary presence không chứng minh fork hoạt động tương đương. |

## MAP coverage

Master plan MAP-00–29 được dùng làm coverage index, không phải tiêu chuẩn xác
nhận hành vi toàn PC:

| Nhóm MAP | PC evidence đã tham chiếu | Còn thiếu để tuyên bố parity |
| --- | --- | --- |
| 00–04: app lock, names, sidebar preview, modal/hover, font | Master plan/user-observed UI; bundle không chứng minh được từng pixel/interaction | Native Windows Hello/PIN, alias data, modal input gate, font fallback ở DPI/theme. |
| 05–09: forward, group member/common group, privacy modal, media/link classification | Forward/message/group/board routes và renderer; PC bundle có điều kiện UI từng phần | Full caller/eligibility trace cho mọi menu, avatar/name roles, privacy ACK, true CDN/media classifications. |
| 10–15: hide E2EE UI, sync banner, quote/reaction, editor/picker/settings, send latency | `re-notes/46`, `48`, latency note 44; PC builder có `textProperties`/urgency; settings/auto-download evidence | Composer selection-style full path, settings effective consumers, native layout, live host/DNS/latency/ACK. |
| 16–23: pins, reminder/urgency, labels, name/avatar, link preview, My Documents, community, call history | `re-notes/45`, `46`, `47`, `48`; specific E2 extracts for archive/pin/menu/calls | PC eligibility conditions, server ACK/role/quota, native UI, live device-call history. |
| 24–29: time, file icons, archive filters, archive actions, bubble/sidebar menus | `re-notes/47`, `48` and current Phase 4 checklist | Native DPI/keyboard/scroll/menu, stale target handling, destructive actions only on controlled test data; archive remains local-only. |

Tham khảo [Phase 3–4 bundle gap plan](../PHASE_3_4_PC_BUNDLE_GAP_PLAN_2026-11-18.md)
cho từng item MAP 10–29. Danh sách gap A01–A20/B01–B12 trong
`docs/ZALO_FEATURE_API_GAPS.md` là baseline ngày 05/11, không phải source-state
hiện tại; dùng nó để lập danh sách luồng cần xác minh, sau đó mới kiểm tra source.

## Fresh bundle trace ledger

Offsets trong bảng này là **byte offset tính từ đầu entry JS**, không phải character
offset ở `re-notes/48–49`, và không phải offset ASAR. Đã đọc fresh từ archive,
không chạy JavaScript. Renderer entry SHA-256 là
`78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08`.

| Entry / byte offset / symbol | Caller chain / hành vi đọc trực tiếp | Kết luận và giới hạn |
| --- | --- | --- |
| Renderer 15.078.739 `static async getQRCode`; login-startup 54.628 call `genFirstQR`, 56.415 `verifyDataWaitingScan` | API wrapper tạo reqqr params (`language`, unix time, imei, computer name, logged_uids, self_g, qr_form), encrypt→get→decrypt. Login-startup map response qua `Qe`, tạo QR bằng `genFirstQR`; scan iterator dùng returned chkWaitScan rồi verify link_data. | Thấy chain request→QR generation→scan verify, không chỉ route. Trust-device branch có điều kiện; không suy branch đang enabled tài khoản. |
| Renderer 11.846.025 caller `r.default.getLoginInfo`; 15.082.762 wrapper `static async getLoginInfo` | Wrapper tạo imei/computer/language/ts và optional info/cert fields, encrypt/get/decrypt. Caller chỉ xuất config/UIN/decryptKey khi HTTP200, outer error0 và data tồn tại; settings/auth_ext_info được đọc trước publish config. | So sánh fork phải giữ error/response data; không coi HTTP200 đơn thuần là logged-in. Không xác nhận session cookie mới/live. |
| Renderer 15.119.993 `static buildParams`; kế tiếp `static sendZText`; 15.122.854 `transformQuoteMessage` | `sendZText` tạo message/clientId/imei→buildParams→group grid+visibility/sendmsg hoặc direct toid/sms→_postSendMsg. Builder serialize styles/ver vào `textProperties`, urgency vào `metaData`; quote transformer có đường styles/urgency riêng. | Full wrapper chain xác nhận metadata separation và direct/group branches. Đây không chứng minh composer selection ranges đang wire hoặc every media kind hỗ trợ fields. |
| Shared-worker 287.001 caller→287.490 `checkRequestfulfilled`; 282.192 `takeUndeliveredScopes`→294.174 `rePrepare` | Batch receive gọi fulfill rồi ghi responsedScopes và settle waiting requests. Fulfill thêm idx vào set, isLast đặt totalBatch=idx+1, so size. Reprepare chỉ lấy undelivered sau BatchTimeout với cùng type/priority, giới hạn rounds và fallback per-partition. | Chứng minh retry/catch-up có điều kiện; fork giữ check đủ idx là deviation an toàn đã ghi. Không sao chép retry mutation mù từ route/label. |
| Renderer 95.038 action edit, 108.873 `Ln` reply, 176.191 `De` group checks | Edit/reply hỏi `ModuleContainer.resolve(...).verifySetting({field:"lockSendMsg",convId,...})` trước action; group action conditions đọc creator/admin/source author và allowed feature flags; share callback hỏi `isProtected` trước forward. | Permission/content-protection là gate tại action, không chỉ disabled button. Chưa truy toàn controller implementation/cache freshness hoặc every group role branch. |
| Renderer 8.888.887 `checkValidFileSizeUpload`; 14.519.398 `_videoTaskWrapper`; 14.521.659 clone dispatch | E2EE file limit check dựa active E2EE+resolved WASM size bound. Video clone wrapper tách video renewType1 và thumbnail renewType0, đợi cả hai, video fail thì fail callback; clone dispatch chọn video wrapper theo msgType. | Xác nhận upload bounds/forward preparation; `_videoTaskWrapper` là clone/renew **không phải video player** và không chứng minh HTTP Range seeking. |
| Renderer 7.474.700 `M0pa` exports `s/o/l/c`; 17.272.300 replace-E2EE message chain | Getter `c` yêu cầu global enable_predownload_file và account getEnableAutoDownload. Replace handler ghi Message→preprocess media→receiveMedia→cache-size guard→autoRunByMsg cho photo/file/GIF, loại failed/pending E2EE. | Có source-level consumer cho config; không suy fork setting được wire chỉ từ const. Cần size provenance và native behavior riêng. |
| Renderer 18.339.176 `_ensureListeners/_dispatch`; 18.341.414 `isNotShowNotificationMessageType`; 18.342.000 `createNotifyForMessages` | Native click/show/close callbacks normalize id/tag→lookup instance→invoke handler; message notification gate archived/OA/unread config/stranger/business eligibility; calltime không notify, some misscall reason3 not notify, pending E2EE không hiện content. | Rich notification eligibility tồn tại, không đồng nhất với “new message = toast”. Fork notification parity cần so policy/lock/account/runtime từng branch. |
| Renderer 13.181.700 `_selectSearchMessageItem`; lazy main-startup 370.607 stale search branch | Selected message checks group still available, tracks exact message then selectResult; search flow supplies isKeywordStale cancellation callback and refuses stale updates; file search dedups msgId+convId. | Thấy result identity/stale guards, không chỉ keyword token. Chưa truy index/tokenizer/ranking/all categories của search-worker. |
| Renderer 15.091.295 `getMediaFromConversation`; 14.651.449 `_handleGetGroupMediaResponse`; 14.654.978 controller `getMediaFromConversation`; 18.049.501/631 media service getters | Static wrapper builds `{media_type}`, optional numeric `lastFetchId`, `limit`, `last_id`; every target except exact `sendToMeId` also gets `group_id` and client IMEI. AES(JSON) is URI-encoded under outer query `params`, alongside common params; method GET. Exact sendToMeId selects `/api/media/oneone/list` + MEDIA_STORE_SEND2ME, others `/api/mediastore/list` + MEDIA_STORE. Handler reads `response.data.items`, `has_more`, `last_id`; controller continues with returned `last_id`. | Chứng minh contract fork hiện tại không cùng PC wrapper (finding bên dưới); oneone/list không phải arbitrary-peer contract. Không suy server acceptance của wrapper vừa đọc. |

Additional entry identity: `pc-dist/lazy/main-startup.41abf8e6af2a55551e3b.js`
SHA-256 `c29c9f0a7f0718c5b044764666b6fc5dec8b8eb65e178819b3cee3b1f93e3426`.
`pc-dist/shared-worker.0cd2e653e67efaa4bf4b.js` SHA-256
`889da03a1ec0cdae4f14707204e6919cf0f52407e9bc22e593e9abe3f038cb10`;
`pc-dist/lazy/login-startup.fc639ac0a5c9bc1be510.js` SHA-256
`5c2fa744029c9931b68aebf976f657938c7fdc68585528ef785aa5aea8c07a3a`.
Các symbol minified chỉ xác định trong snapshot này, không phải API stable.

## Những khoảng trống của chính lượt reverse engineering

Additional source coverage in the continuation: full direct reads of
`presence.rs:1–322`, `unread.rs:1–718`, `labels.rs:1–337` and
`avatar_history.rs:1–237`, plus worker presence delivery and composer typing
callers. Presence parsing preserves account self filtering, read/delivery IDs,
and the UI throttles typing at ten seconds. Fresh PC `sendIsTyping` confirms POST,
AES JSON params, IMEI, one leading `g` removal, and destination USER=3 / PAGE=5.
Fork currently sends USER=3; its UI gates unknown/stranger peers and self cloud.
An OA path sending USER has not been demonstrated, so this is a comparison limit,
not a confirmed defect. Labels retain unknown row fields/version and verify an
explicit edit by server readback; avatar history preserves wire photo ID type,
checks cursor progress and never sets `isPostSocial` on reuse. No new proven bug
was found from those reads; full concurrency/native/entitlement acceptance remains.

- Chưa giải nghĩa đầy đủ 118 JS entry theo module/caller chain. Một số renderer,
  sync/search worker, login, upload, archive và call flow đã được truy có mục tiêu.
- Chưa disassemble/reverse native `.node`/DLL; mới inventory family và đọc wrapper
  JS của một số native API. Không suy hành vi camera, codec, sqlite hay crypto từ
  tên thư viện.
- Chưa kiểm tra mọi UI route, từng menu condition, mọi API params/response, entitlement,
  edge/retry/error branch. 454 route literal và index 421 không thay thế việc đó.
- Không chạy PC tương tác/capture trong lượt này; UI parity ngoài note/screenshot
  là E2 hoặc user-observed evidence đã ghi trước. Không đo pixel, latency hoặc
  Windows ARM64 runtime.
- Chưa đối chiếu từng 159 file Rust hoặc mọi command/event end-to-end. Việc đó
  thuộc auditor code; bảng này là cross-domain reference map.

Vì vậy, đây là **audit parity trên toàn bộ miền sản phẩm với độ sâu không đồng đều**,
không phải “dịch ngược toàn bộ Zalo PC”. Finding transport bên dưới có đủ source
proof; không có schema/endpoint mới được suy ra. Các tính năng được triển khai cần được kiểm tra tiếp
theo cổng source → fixture → native → controlled live, từng MAP riêng biệt.

## Source đọc trực tiếp trong lượt mở rộng

Bảng phía trên là reference map; bảng này mới mô tả code thực sự đã đọc ở lượt
audit mở rộng, không coi tên module hoặc note lịch sử là proof đã review source.

| Source / vùng đọc | Điều đã xác minh từ code | Phần chưa đọc/chưa chứng minh |
| --- | --- | --- |
| `session.rs:1137–1145`, `socket.rs:158–180, 735–800` | Ingestion cho phép cả ws/wss; connection chỉ kiểm host rồi gắn credential; default/docs là wss. | Chưa chứng minh server thật trả ws hoặc có đường đầu độc config. Không tạo traffic thử. |
| `profiles.rs:1–174` | Registry/account-local session, mutex trong activate/remember/forget, logout xóa credential và temp account. | Toàn GUI switching/boot retry, IO failure atomicity và DPAPI/native acceptance chưa chứng nhận từ file này. |
| `privacy.rs:1–140` | Parse chỉ explicit setting trong value domain, read fresh trước set, không coi thiếu key là được phép, mutation một lần + readback. | Full UI/Worker event/runtime path và live permissions chưa review hết. |
| `account_metadata.rs:1–230` | Aliases/friends paged bounds, loop/duplicate detection, inbox classifier giữ explicit profile type, đọc independent snapshots với timeout. | Chưa đọc cache encode/decode/restore/save và mọi GUI consumer của snapshot. |
| `presence.rs:1–210, 234–257` | Typing/read receipts parser, self filter, declared 8s display/10s send throttle; typing request qua standard session. | Caller thực sự áp throttle, privacy read/seen ngoài parser và UI lifetime còn cần trace; chỉ const không chứng minh throttle hiệu lực. |
| `labels.rs:55–304` | Snapshot giữ raw fields/version; update chỉ intended label action; get/set dùng encoded params và service label; readback kiểm intent. | Chưa rà toàn UI selection/pending state và conflict native/live. |
| `server_media.rs:1–277` | Strict list-presence/nested error gate, group/direct request builders, cursor/page/count, media item projection. | Item malformed dùng filter_map có thể bị bỏ; chưa có capture schema thật cho mọi alias/IDs. Không dùng parser tests để khẳng định server archive complete. |
| `search.rs:1–180` | Account-local request model, generation/cursor identities, Vietnamese folding, projection loại call/recall/delete khỏi search. | Query/paging/cancel/hidden-policy sections và full GUI runtime guard chưa đọc trong nhánh parity. |
| `group_metadata.rs:1–127` | Cache chỉ dùng đúng server membership version; cached display không tái dùng send permission; invalidation không xóa history. | Permission freshness/mutations phía group APIs và UI chưa chứng nhận. |
| `quick_message.rs:1–100` | Quick-message store là local data + expansion/search/upsert/remove. | Không suy rằng server quickmessage/auto-reply đã parity; full worker/API và persistence chưa trace. |
| `zcloud.rs:1–140` | My Documents dùng viewer_key/enk riêng, key length checks/decrypt, positive explicit quota, runtime active check sau response. | Verify/download/decryption theo file, every cloud API consumer và paid entitlement còn cần review/live. |
| `gui/modals.rs:4340–4372`, `media_download.rs:845–870` | Album position counter chưa wire; range API explicit production-unwired. | Đây là scope gaps đã ghi từ audit 16/11, không phải bằng chứng dropped payload hoặc dead code; PC album counter và range behavior chưa có trace đủ để kết luận mismatch. |
| `gui.rs:870–995, 6330–6420`, `gui/modals.rs:1480–1548` | Local notification toggle/sound thực sự đi vào desktop_notice.configure, preparation worker, tray.configure; AppSettings chứa một số legacy privacy values nhưng UI server privacy dùng typed action riêng. | Không kết luận local legacy serialized fields đang là fake control. PC notification eligibility phong phú hơn; cần so mọi desktop_notice incoming-message policy branch trước khi tuyên bố parity. |

## Finding mới được phản biện độc lập

**P1 — WSS transport invariant chưa được cưỡng chế, trigger có điều kiện.**
Auditor code phát hiện và auditor parity xác nhận: `Session::apply_server_info`
cho phép `ws://` official shard. `socket::run_once` lấy URL (kể cả SavedSession),
kiểm tra host allowlist nhưng không bắt buộc scheme, rồi gắn `zpw_sek` vào Cookie
và header trước khi connect. Nếu config/persisted state chứa `ws://` hợp lệ theo
host allowlist, transport gửi credential qua cleartext. Điều này không có nghĩa
server đã trả cấu hình đó hoặc có exploit/config poisoning được chứng minh;
nguồn config bình thường qua TLS và SavedSession có DPAPI.

Parity reference: `re-notes/02_chat_data.md:161` và docs của `Domains::ws` chỉ ghi
`wss://`. Lần đọc E2 PC 26.11.12 cho thấy `_connectSocket` dùng configured URL;
`_pingEndpoint` thay `wss` bằng `https`. Không có bằng chứng cần hỗ trợ production
`ws://`. Sửa tối thiểu được hai auditor thống nhất là bắt buộc `wss` ngay tại
connection boundary trước credentials; ingestion filter là defense-in-depth,
không thay thế boundary vì session đã lưu có thể bỏ qua ingestion. Fixture loopback
cần ngoại lệ explicit test-only, không bật cho official remote host. Regression
phải offline, không gửi secret hay thực hiện network exposure.

Sửa WSS của repair agent đã được auditor parity đọc lại: discovery chỉ giữ WSS,
`secure_ws_request` kiểm scheme trong `run_once` trước DNS/header, không đổi
query/path/port/host guards. Hai regression offline đã thêm; kết quả thực thi
do parent chạy. Static review chấp nhận bản sửa, không nâng thành native/live.

**P2 — server-media tools dùng hợp đồng không khớp với wrapper PC.**
Lần trace fresh phát hiện `server_media.rs:205–277` gửi POST tới group/chat host,
payload `grid`/`fid`, numeric `type`, `page`, `count`, `cursor`. Trong PC renderer,
`getMediaFromConversation` byte 15.091.295 gửi GET qua service MEDIA_STORE hoặc
MEDIA_STORE_SEND2ME. Wrapper đặt `media_type`, optional `limit`, `last_id`,
`lastFetchId`, cùng `group_id`/IMEI cho mọi target không phải `sendToMeId` vào
JSON mã hóa AES trong outer query `params`; common params là query fields riêng.
`oneone/list` được chọn chỉ khi target đúng `sendToMeId`, không phải mọi chat 1-1.
`WorkerCmd::FetchServerMedia` ở
`gui/worker.rs:4401–4455` hiện đưa mọi non-group thread vào direct function, nên
đây là mismatch ở luồng tools đang được gọi, không chỉ helper bỏ hoang.

Response PC cũng khác: `_handleGetGroupMediaResponse` decode AES,
`preParse(owner_id,group_id)`, đọc `data.items`, `has_more`, `last_id`, map
`src_url`, `owner_id`, `group_id`, `type/sub_type` thành model; controller tiếp
tục cursor bằng lastId. Fork parser dùng `next_cursor`/`cursor` và các alias
id/url/type hiện tại chưa có fixture PC xác nhận. Không sửa bằng cách đoán thêm
alias. Cần guard path chưa chứng minh hoặc triển khai đúng wrapper sau khi
resolve service keys, enum và response/item fixtures có nguồn. Local archive
không bị coi là thiếu vì luồng này. Finding đã chuyển auditor code và parent
phản biện, chưa tuyên bố live failure.

Fresh service-key proof bổ sung: renderer byte 18.044.847 enum xác định
`MEDIA_STORE:"media_store"` và `MEDIA_STORE_SEND2ME:"media_store_send2me"`;
`setEndpoints` byte 15.078.230 nhận hai key riêng từ config. Mapper tiếp theo
byte 14.652.300 đọc id, `global_msg_id`, `client_msg_id`, `owner_id`, `group_id`,
`created_time`, `src_url`, `thumb_url`, `file_name`; type là chuỗi `media`, `file`,
`link`, media sub_type 1/2, không phải wire type numeric Photo1/Video2/File3/Link4
do fork tự khai báo. UI category numeric có thể là enum nội bộ nhưng phải map
sang contract client có bằng chứng trước network.

Final mapper re-read also pins `fileSize` to wire field `file_size` (not `size`).
PC uses `sub_type` for the media photo/video branch and does not require it to be
absent for file/link rows. Those two differences were found in the repair diff
and returned to the repair agent before acceptance; strictness is not parity if
it rejects a field that the native mapper simply ignores.

Finding backup/WAL do auditor code phát hiện cũng đã được phản biện về source:
`desktop_backup.rs:385–520` raw snapshot trước mở store và raw rollback trong
error branch trước drop transaction/store; `phone_sync/import.rs:327–355`
chỉ copy file chính. Điều đó không bảo toàn snapshot SQLite WAL và xung đột
với ghi chú “committed batches remain”. Auditor code có reproduction SQLite
cô lập cho thấy mất hàng trước import tùy checkpoint timing. Đây là consistency
bug độc lập với contract Zalo PC; nhánh parity đồng thuận cần sửa nhưng không
gọi reproduction đó là E1 Zalo hoặc bằng chứng native acceptance.

Additional cross-review acceptance: the original HTTP CONNECT parser checked its
header cap only after unbounded `read_line`, and accepted EOF after status 200 as
complete headers. Root's `proxy.rs` fix now caps reads with `take(remaining + 1)`
before allocation and requires a newline/nonzero read; the shared header budget
remains bounded. Regression source covers oversized unterminated status/header
with the writer open, and EOF after 200/partial header. Static review accepts the
fix; test execution belongs to parent. This is a conditional configured-proxy
resource/parser issue, not a Zalo contract or demonstrated remote compromise.

The code auditor's document-preview privacy finding was independently confirmed:
`DocPreviewLoaded` lacks owner/runtime/context, its GUI consumer opens plaintext
unconditionally, and restrictive hidden-chat relock/logout clear image viewers
but not the document viewer. Worker pre-enqueue checks cannot reject an event
already queued when the boundary changes. Fix/re-review must add receive-time
provenance/access checks, clear retained plaintext on relock/logout, and guard
the viewer context. This is a local privacy invariant, not a PC wire claim.
