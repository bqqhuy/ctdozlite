# Zalo Desktop PC 26.11.12 — Windows ARM audit baseline

Ngày audit: 2026-11-21 (Asia/Saigon)

## Phạm vi và phương pháp

Audit này dùng bản Zalo Desktop đang cài tại C:\Users\bqqhu\AppData\Local\Programs\Zalo và đang chạy dưới tài khoản Windows hiện tại. Phân tích tĩnh đọc resources/app.asar theo định dạng Electron ASAR, chỉ trích xuất các bundle JavaScript cần thiết vào thư mục tạm; không sao chép hoặc đọc nội dung tin nhắn, cookie, khóa phiên hay cơ sở dữ liệu 317 MB của Zalo.

Nguồn bằng chứng:

- E1-runtime/process: danh sách process, command line, thư mục dữ liệu, config.json, DevToolsActivePort và file version trên máy.
- E2-bundle: package.json, pc-dist/compact-app-pc.*.js, sync-v2-sub-worker.*.js, shared-worker.*.js, dal-worker.*.js, mainless-worker.*.js, stast.json trong ASAR.
- E2-repository: đối chiếu với src/socket.rs, src/session.rs, src/endpoints.rs, các re-note và docs/ZALO_REFERENCE_ANALYSIS.md.
- REA (morluto/rea) được dùng làm chuẩn phương pháp: phân tích JavaScript/Electron nên trả về module, IPC, route, native add-on và bằng chứng; lần này các thao tác ASAR/route inventory được thực hiện cục bộ vì REA chưa được cài vào workspace.

Không có network capture, không hook TLS, không đọc DevTools live: cổng 9222 trong DevToolsActivePort là stale/không lắng nghe khi kiểm tra. Do đó mọi kết luận về payload thực tế, entitlement và frame bytes vẫn phải đánh dấu E1 chưa có.

## Baseline cài đặt và runtime

| Quan sát | Kết quả | Ý nghĩa |
|---|---|---|
| Phiên bản bundle | 26.11.12 trong package.json | Baseline PC khác với snapshot 26.11.10 đang được một số re-note tham chiếu. |
| ASAR | 181,103,630 bytes; SHA-256 4C3A7AA9B8FB409A95C6739EF9DCCDC01127D0DC4F6383EE396400B7F94484C5 | Cho phép tái lập đúng artifact audit. |
| Zalo main executable | SHA-256 7D5E8900760F998AE965294AE86C434D27B254FCDBA668A84C3D7EC176890641 | Dùng cùng build với ASAR. |
| Process model | Electron nhiều process: browser, GPU, network utility, Node utility, renderer, capture | Không thể so sánh trực tiếp memory/latency của PC với một process Rust duy nhất. |
| User data | C:\Users\bqqhu\AppData\Roaming\ZaloData; ftsm_*.db khoảng 317 MB | Local storage lớn và account-scoped; không đưa dữ liệu này vào Git hoặc fixture. |
| Display | device-scale-factor=1.5; config ghi os_machine: arm64 | Native UI test phải bao phủ DPI 150% và màn hình xoay hiện tại. |
| Remote debug | DevToolsActivePort=9222, nhưng 127.0.0.1:9222 refused | Không được xem port file là bằng chứng runtime inspection thành công. |

### Kiến trúc Windows ARM

Đọc PE header của 139 file exe/dll/node trong bản cài cho thấy: 125 image là machine 0x14c (x86/IA-32), 14 image là machine 0x8664 (x64), và 0 image là machine 0xaa64 (ARM64). Zalo.exe, ZaloCap.exe và ZaloCall.exe đều có machine 0x14c. Command line runtime cho thấy Chromium renderer/network/GPU process và app-path trỏ vào resources/app.asar.

Kết luận: Zalo PC này chạy qua Windows ARM emulation, không phải ARM64 native. ctdozlite cần được test theo hai trục riêng: correctness/parity với PC emulated; hiệu năng và crash behavior native ARM64 của fork.

## Bằng chứng từ bundle PC 26.11.12

### Transport, sync và cursor

Bundle chứa các lớp worker riêng (sync-v2-sub-worker, shared-worker, dal-worker, mainless-worker) và lớp WebSocket persistent. Các literal/đường dẫn quan sát được gồm push/pull message commands 501, 521, 510, 511 và các pull bổ sung 513, 514, 515, 517, 518; control commands 601–604; heartbeat/ping path với eventId echo và reconnect/retry state; cursor state lastId, preIds, queue identifiers 510_0/511_0, miss/overflow QoS và sync-v2 checkpoint/noise ID.

Điều này nâng các nhận định hiện có về realtime lên E2 trên đúng build 26.11.12, nhưng chưa nâng framing bytes, heartbeat interval cụ thể hoặc server payload schema lên E1.

### Route inventory và tính năng

compact-app-pc chứa các nhóm route thực tế: board/pin/topic/reminder (/api/board/topic/*, /api/board/oneone/*, /api/board/pin/*); E2EE 1-1 và nhóm (/api/e2ee/pc/message/* và /api/e2ee/pc/group/*); cloud/media (/api/cm/*, cloudmedia); conversation state (unread mark, auto-delete, block-sharing, labels, archived chat); AI translation/voice-to-text, linked devices và backup.

Route presence chỉ chứng minh code client có nhánh gọi route. Không được suy ra entitlement, server schema, hay khả năng hoạt động của tài khoản từ route index. Các mutation vẫn cần capture/fixture và test ACK riêng.

### Build health signal

pc-dist/stast.json báo 323 warnings, chủ yếu là attempted import errors/type exports (ví dụ IResizerOptions, IDatabaseStateStorage, ISyncQueueRepository, AESKey). Đây là tín hiệu chất lượng bundle cần theo dõi, không tự động chứng minh lỗi runtime; cần correlate với entrypoint và đường chạy thực tế.

### Native/plugin surface

ASAR unpacked và thư mục plugin có capture (ZaloCap.exe, ZaloCall.exe, ZaviMeet.exe), OCR/ONNX, SQLite, JXL/image, media/file utilities và zcall native modules. Đây là khác biệt kiến trúc lớn với fork Rust: parity của UI/API không bao hàm parity của capture, OCR, media codec, audio APM hay SQLite behavior.

## Đối chiếu với ctdozlite

### Đã có cơ sở parity

- src/param_cipher.rs và src/crypto.rs có implementation ParamCipher v2/signkey; các re-note có E1 login evidence.
- src/socket.rs đã có command constants cho push/pull/control, ping, signal và phone sync; có epoch, bounded write, DNS/host guard, reconnect và frame size guard.
- src/endpoints.rs/src/session.rs đã mô hình hóa service map và session routing.
- Các invariants fail-closed E2EE, account scoping và không log secret đã được ghi rõ trong repository policy.

### Khoảng cách cần ưu tiên audit

1. P0 — Realtime interop evidence: capture trên tài khoản test để chốt frame header, auth response, ping interval, command 501/510/511/521 và cursor commit ordering. Mã hiện có nhiều giả định đã được harden nhưng chưa có E1 của build 26.11.12.
2. P0 — E2EE production boundary: route E2EE được xác nhận trong bundle, nhưng handshake/prekey/identity/receive decrypt và payload coverage phải được đối chiếu với capture thật. Không dùng route existence để mở cờ production hoặc fallback plaintext.
3. P1 — ARM native test matrix: build aarch64-pc-windows-msvc, kiểm tra camera/audio/SQLite/HTTP/TLS/WebSocket trên ARM64 native; so sánh với Zalo PC x86-emulated bằng latency/CPU/RSS, không dùng raw process count.
4. P1 — Media and VoIP: PC có native capture/OCR/codec plugin surface. Cần test device thật cho camera, microphone, screen share, AEC/NS/AGC, video playback/range và upload resume. Signaling route không chứng minh media pipeline hoạt động.
5. P1 — Cursor/sync durability: đối chiếu lastId, preIds, miss/overflow, isLast, duplicate delivery và logout/account switch với src/socket.rs, history_sync.rs, store transaction boundaries.
6. P2 — UI/DPI: test 150% DPI, rotation, multi-window viewer, tray/capture và account switching; Zalo runtime đang dùng Chromium renderer + separate capture processes.
7. P2 — Build warnings: lập inventory warning theo entrypoint; chỉ sửa khi warning chạm module được load. Không coi 323 warnings của official bundle là bằng chứng trực tiếp về lỗi fork.

## Verification state

- git status sạch sau khi xóa helper tạm; không có file audit tự động nào ngoài tài liệu này.
- scripts/audit-prepush.ps1: FAILED vì máy hiện không có cargo.exe trong PATH hoặc C:\Users\bqqhu\.cargo\bin; heuristic script cũng báo một possible UI stub ở src/gui/workspace.rs, cần review riêng.
- Chưa chạy cargo check/test/clippy; đây là blocker môi trường, không phải kết luận compile failure của source.
- Chưa tạo traffic capture, chưa đọc cookie/session/DB, chưa thực hiện thao tác gửi tin hoặc mutation trên tài khoản thật.

## Kế hoạch E1 tiếp theo

1. Chuẩn bị tài khoản Zalo test và peer test riêng, bật logging không chứa cookie/key.
2. Ghi lại flow tối thiểu: login/refresh, mở 1-1, group, gửi text, reaction, recall, pin, media, reconnect và logout.
3. Capture ở lớp HTTP/WebSocket hợp pháp trên thiết bị test; redact zpw_*, IMEI, user IDs và message content trước khi lưu.
4. Lưu fixture vào re-notes/fixtures/ với timestamp/build hash; cập nhật docs/ZALO_REFERENCE_ANALYSIS.md từ E2 → E1 chỉ cho trường đã quan sát thật.
5. Dùng fixture để kiểm tra ctdozlite, sau đó chạy build/test ARM64 native và nghiệm thu UI/device riêng.

Các kết luận protocol trong tài liệu này đều ghi rõ mức bằng chứng; không có kết luận nào dựa riêng vào route index hoặc suy luận E3.
