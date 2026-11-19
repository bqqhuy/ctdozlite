# Đối chiếu tĩnh Zalo PC — 2026-11-19

Baseline fork: 50d6abe và diff audit hiện tại. Đây là kiểm kê/truy vết có mục tiêu,
không phải đã dịch ngược toàn bộ client. Không đọc kho tài khoản, cookie hoặc khóa;
không gọi API tài khoản thật, không chép bundle source vào repository.

## Nguồn và bằng chứng

Thư mục cài đặt có Zalo-26.10.20 và Zalo-26.11.12. Nguồn chính là
C:\Users\bqqhu\AppData\Local\Programs\Zalo\Zalo-26.11.12\resources\app.asar.
SHA-256 archive: 4C3A7AA9B8FB409A95C6739EF9DCCDC01127D0DC4F6383EE396400B7F94484C5.
Hash archive khác hash renderer entry ghi ở re-notes/45; không so hai hash như
cùng artifact. Đọc header JSON ASAR bằng .NET, duyệt entry/kích thước, tìm route
literal và đọc wrapper native unpacked. Không thực thi bundle. Kết luận là E2,
không chứng minh schema server, entitlement hay tương thích thiết bị thật.

## Kiểm kê

ASAR có 13.169 file entry: pc-dist 13.076, native 70, main-dist 20;
bootstrap.js, package.json, libs mỗi nhóm một. Đây không phải số Webpack module
hoặc số tính năng.

| Entry | Byte | Phạm vi |
| --- | ---: | --- |
| pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js | 21.691.083 | Renderer/UI/model |
| pc-dist/sync-v2-sub-worker.0cd2e653e67efaa4bf4b.js | 21.952.717 | Đồng bộ/control |
| pc-dist/search-worker.0cd2e653e67efaa4bf4b.js | 21.918.698 | Search worker |
| pc-dist/zd-worker.0cd2e653e67efaa4bf4b.js | 8.000.389 | Chưa giải mã toàn bộ |
| pc-dist/lazy/default-login-main-startup-shared-worker-znotification.82f32878081d885b6bdd.js | 17.618.606 | Shared/login/notification |
| main-dist/main.js | 4.370.778 | Electron main |
| main-dist/compact-app.js | 4.204.398 | App bridge |
| main-dist/preload-sqlite.js | 2.842.005 | SQLite bridge |
| main-dist/compact-app-preload.js | 2.260.257 | Preload bridge |

Native families: zcall, sqlite3, mp4thumb, file-utilities, file-utils, zwalker,
zlang, zimage, zjxl, zfile, win-utils, os-sys-info, v8-profiles, logger,
db-cross-v4 và zocr. Nhiều family có ia32/x64; zcall có zcall_ia32.node.
zjxl kèm libjxl/Brotli/OpenCV; zimage kèm libvips. Sự hiện diện DLL không chứng
minh mọi path đang hoạt động. Wrapper zcall nối getVideoFrameLocal/getVideoFrame
với local/remote renderer và bridge codec/hold/audio. Ngoài archive có Electron,
Chromium packs, ffmpeg.dll, EGL/GLES, SwiftShader/Vulkan và V8 snapshots.
Fork Rust không cần sao chép Electron để đạt parity hành vi.

## Route/service coverage

Quét literal /api/... toàn archive thấy 454 đường dẫn khác nhau. Đây là heuristic
chuỗi, có nhiều chunk/route cũ; không thay thế index 421 route có command mapping
và không phải số API đã dùng được. Nhóm literal lớn: group 81, message 69,
social 40, e2ee 31, board 30, voicecall 29, friend 24, login 24, prodcatalog 11,
conv 8, cm 6.

Config/accessor xác nhận các service riêng: chat, sticker, friend, profile,
group, group poll, file, gif, media store, voice call, conversation, group board,
alias, LAN friend, label, send-to-me media store, cloud message. Fork phải tiếp
tục resolve qua session service map/endpoints.rs; fallback literal không chứng
minh host live.

## Các luồng đã truy vết

- Call normalizer hỗ trợ calling/receivecall/misscall/calltime. Renderer parse
  params JSON string, ưu tiên params.isCaller/params.duration; misscall quyết định
  MISSCALL, các action khác CALLTIME. Reason 0–5 đã đối chiếu Rust; lỗi direction,
  duration và missing calltime được chuyển/sửa/tái kiểm tra tĩnh trong diff audit.
- Không xác nhận endpoint đọc call history riêng trong phạm vi quét. Call rows
  đi qua message normalizer/sync; logendcall không phải bằng chứng API đọc.
- Bundle bỏ call control quá 60 giây; baseline fork 50d6abe đã đổi từ 90 giây.
  Shape “nghe thiết bị khác” vẫn cần E1 để mở rộng status/uid classification.
- History-open từng nuốt lỗi fetch thành None và task cũ có thể publish qua phiên.
  Phản biện chéo với auditor code đã dẫn đến error surfacing/runtime guards.
- Native media bridge được xác nhận tĩnh; camera/codec/AEC/UDP của fork phải
  nghiệm thu thật, không suy parity từ tên library hoặc test fixture.

## Gap cần E1/native acceptance

- Gọi đến rồi nghe/từ chối ở thiết bị khác: payload, status, uid namespace,
  thứ tự WS/long-poll và chống replay.
- Thoại/video hai chiều, AEC/NS/AGC, đổi camera/mic, DPI/resize, reconnect,
  audio-video upgrade và group member lifecycle.
- Old 1:1 call history: nguồn phone sync/server queue, depth/cursor/coverage;
  không chế endpoint từ tên tính năng.
- UI pixel/layout/menu/notification nhiều kích thước/DPI trên ARM64.
- Mỗi route cần schema/entitlement/ACK/retry/wiring và nghiệm thu riêng.

## Giới hạn

Không chạy Cargo trong inventory này. Không reverse-engineer toàn bộ native
machine code, không chứng nhận toàn bộ 13.169 entry, không có live capture mới
hoặc benchmark native. Không tuyên bố toàn dự án không còn lỗi.

## Bảng coverage theo master action plan

Đối chiếu docs/ZALO_PC_MASTER_ACTION_PLAN_AND_GUIDE.md (tái-baseline 18/11,
SOURCE_AUDIT_CONSENSUS Phase 3/4) với ZALO_REFERENCE_ANALYSIS.md và source hiện tại.
Các câu “chưa triển khai” trong phần baseline lịch sử của hai tài liệu không được
dùng làm bằng chứng rằng source hiện nay vẫn thiếu tính năng.

| Miền / MAP | Code hiện có | Bằng chứng PC / mức | Giới hạn parity còn mở |
| --- | --- | --- | --- |
| Chat/send/reply/reaction, MAP-05/12/15 | chat.rs, outgoing.rs, service_resolver.rs, reactions.rs, gui/messages.rs | re-notes/29; send latency guide; E2 + fixture | Delivery/latency live, late echo, device privacy; compile không thay live |
| History/archive/call rows, MAP-23/26 | history_sync.rs, history_open.rs, store.rs, message_view.rs | PC normalizer/sync worker; E2 | Old 1:1 coverage/cursor và multi-device call record consistency chưa nghiệm thu |
| File/photo/video, MAP-09/20/25/27 | media.rs, media_download.rs, media_renew.rs, server_media.rs, gui/workspace.rs | re-notes/15/22/45 và người dùng quan sát UI | Per-file upload/chunk/renew đường thật, mọi extension/MIME/filter/page và native preview chưa chứng nhận |
| Group/community, MAP-06/07/08/22 | groups.rs, group_metadata.rs, group_state.rs, group_entry.rs, group_panel.rs | re-notes/30/31/34/45; E2 | Role/entitlement/permission drift cần server acceptance; group không đồng nhất community |
| Call/audio/video | call.rs, call_lifecycle.rs, windows_video.rs, gui overlay | voicecall route map, call renderer/enums, native zcall wrapper; E2 | Camera/audio two-way/AEC/reconnect/group/live call dismissal cần E1/native |
| Security/app lock/E2EE, MAP-00/10 | gui/auth_guard.rs, session.rs, e2ee_integration.rs, crypto/* | Reference authority security invariants, master UI requirements | E2EE UI ẩn không có nghĩa engine có interoperability; không suy E2EE từ HTTPS/ParamCipher |
| Settings, MAP-14 | gui/modals.rs, settings.rs, account_controls.rs, privacy.rs | Master 10-page UI requirement, existing verified adapters | Page có mặt không chứng minh mọi setting được server chấp nhận; mutation phải fail-closed |
| GUI/sidebar/naming, MAP-01/02/03/04/11/16/18/19/24/28/29 | gui/workspace.rs, gui/theme.rs, gui/modals.rs, message_view.rs, clock.rs | Master user screenshots/E1 UI, re-notes/45/E2 | DPI/font, modal click-through, menu target reorder và hover native còn cần nghiệm thu |
| Todo/poll | todo.rs, poll.rs, gui/tools_panel.rs, worker.rs | Group-board contracts; source response provenance | Runtime/thread isolation được sửa trong audit; server permission và live mutation chưa xác nhận |

### Lỗi có bằng chứng đã chuyển/sửa trong vòng audit này

- History fetch bị nuốt lỗi và result thiếu runtime provenance: worker.rs + GUI event.
- Call history thiếu calltime, sai params direction/duration precedence và missed reason:
  message_view.rs; kiểm tra tĩnh lại đã chấp nhận diff sửa.
- Link plaintext guard không fail-closed khi E2EE policy store lỗi: chat.rs dùng
  require_plaintext_send chung; fixture kiểm tra trước network.
- Todo/poll response thiếu runtime/thread identity: GUI/worker/tools_panel sửa provenance.
- Integration tests đụng storage root dùng chung: subprocess cô lập trước khởi tạo storage.

Đây là findings có source proof trong phạm vi audit, không phải bảng mọi bug dự án.
Không bổ sung “code rác” để xóa nếu chưa chứng minh không có consumer/feature contract.
Không tạo endpoint từ route hoặc đánh dấu gap lịch sử tài liệu là bug hiện tại.
