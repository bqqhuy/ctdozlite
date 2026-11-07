# Lộ trình hoàn thiện fork theo Zalo PC/Web

Ngày cập nhật: 07/11/2026 (Asia/Saigon).
Trạng thái tổng thể:
- **Giai đoạn 1 (v0.008-arm → v0.011-arm)**: Đã hoàn tất 100% bao phủ bề mặt API và tính năng (R0 → R15).
- **Cột mốc kiểm toán v0.012-arm**: Đã hoàn tất rà soát toàn diện, dọn 100% dead code, 0 lỗi Clippy, chuẩn hóa luồng dữ liệu 2 chiều. Xem [báo cáo v0.012](releases/v0.012-arm-release.md).
- **Giai đoạn 2 (từ v0.013-arm trở đi)**: Đang triển khai lộ trình Production Hardening, Realtime Engine & Experience Parity. Xem [Lộ trình Giai đoạn 2](ROADMAP_PHASE_2.md).

## Mục tiêu và phạm vi

Bao phủ 20 nhóm A, 12 nhóm B và các nhánh nghiên cứu C trong
[bảng gap](ZALO_FEATURE_API_GAPS.md). Các nhóm có thể chồng lấn; API numeric/legacy
không được tính như tính năng độc lập. 421 route tĩnh là chỉ mục bằng chứng, không
phải danh sách cần gọi hết.

### Giai đoạn 1: Bốn phiên bản bao phủ tính năng (Đã hoàn tất)

| Phiên bản | Khối công việc giữ lại | Mục tiêu bàn giao | Trạng thái |
| :--- | :--- | :--- | :--- |
| **[v0.008-arm](releases/v0.008-arm-plan.md)** | R0, R8, R9, R10 | Sửa cuộc gọi nhận ở điện thoại; UI PR #105; người lạ/privacy; tổ chức hội thoại/danh bạ/hồ sơ; quản trị nhóm/cộng đồng | **ĐÃ HOÀN TẤT** |
| **[v0.009-arm](releases/v0.009-arm-plan.md)** | R11, R12, R13 | Voice/card/batch, sticker/GIF/viewer/screen-share; to-do/nhắc hẹn/automation server; bảo mật tài khoản và chat E2EE | **ĐÃ HOÀN TẤT** |
| **[v0.010-arm](releases/v0.010-arm-plan.md)** | R14 | Lịch sử, server media library và zCloud; hoàn thiện V1/BinNet/import/khôi phục | **ĐÃ HOÀN TẤT** |
| **[v0.011-arm](releases/v0.011-arm-plan.md)** | R15 | Business/OA/enterprise/catalog, phần tài khoản đặc thù, LAN/transfer theo capability xác minh | **ĐÃ HOÀN TẤT** |

### Cột mốc Kiểm toán & Hợp nhất chất lượng

| Phiên bản | Khối công việc | Mục tiêu bàn giao | Trạng thái |
| :--- | :--- | :--- | :--- |
| **[v0.012-arm](releases/v0.012-arm-release.md)** | Hardening & Quality Audit | Dọn sạch 100% dead code (`AtomicRestoreSession`, v.v.), sửa toàn bộ 16 cảnh báo Clippy, tích hợp luồng 2 chiều (No UI Stubs / No Dropped Payloads), chuẩn hóa domain `*.chat.zalo.me` cho `ServiceResolver` | **ĐÃ HOÀN TẤT** (Commit `c958d7c`, CI Run `#42344064020` Success) |

### Giai đoạn 2: Lộ trình Gia cố Sản phẩm & Trải nghiệm Thời gian thực (Từ v0.013-arm trở đi)

Chi tiết kỹ thuật từng gói công việc cho AI và lập trình viên: xem [ROADMAP_PHASE_2.md](ROADMAP_PHASE_2.md).

| Phiên bản | Khối công việc | Trọng tâm bàn giao kỹ thuật | Kế hoạch chi tiết |
| :--- | :--- | :--- | :--- |
| **v0.013-arm** | R16 | **Realtime Push Engine & Zero-Delay Sync**: Thay thế HTTP Polling bằng WebSocket Full-Duplex; Typing Indicator; Read Receipts tức thời; Tối ưu pin ARM64 Modern Standby | [Kế hoạch v0.013](releases/v0.013-arm-plan.md) |
| **v0.014-arm** | R17 | **WebRTC Audio DSP & VoIP Hardening**: Bộ xử lý âm thanh WebRTC APM (AEC3 chống vọng, Noise Suppression, AGC); Software Video Fallback (OpenH264); Screen-Share 30fps | [Kế hoạch v0.014](releases/v0.014-arm-plan.md) |
| **v0.015-arm** | R18 | **In-App Rich Media & Large File Transfers**: Multi-window Photo Gallery; Range-request Video Streaming (RFC 7233); In-app Document Preview (PDF/Doc/Code); Chunked Resumable Upload (>1GB) | [Kế hoạch v0.015](releases/v0.015-arm-plan.md) |
| **v0.016-arm** | R19 | **Full E2EE Double Ratchet & Multi-Account**: Bắt tay trao đổi Prekey Signal Protocol thật; Mã hóa kho khóa bằng Windows DPAPI; Đa tài khoản Zalo chuyển đổi trong 0.5s; Khóa app Windows Hello | [Kế hoạch v0.016](releases/v0.016-arm-plan.md) |

Các mốc R là thứ tự triển khai/nghiệm thu trong cùng phiên bản, không tự tách thành
release có số khác. Các khả năng phụ thuộc server/key/entitlement vẫn cần bằng chứng
và giữ blocker thật; việc gộp phiên bản không tự đóng task hoặc xác nhận thiết bị.

R0 là việc sửa đầu tiên, có thể tạo bản chạy thử trước khi hoàn thành UI mới.
Các việc tồn v0.007 vẫn giữ task ID/bằng chứng; được phân lại vào các đợt trên chứ không bị đóng.

## Các mốc của v0.008

1. **M0 — tái hiện cuộc gọi:** chính xác ca người dùng xác nhận: cá nhân, nhận trên điện thoại.
2. **M1 — sửa call lifecycle:** normalize control, terminal/replay, identity biên GUI, dọn local;
   không end/reject cuộc gọi đang diễn ra trên điện thoại. Có test âm tính và bản thử.
3. **M2 — hộp người lạ:** xác minh setting 59 và state nguồn, lưu/refresh/reconnect;
   phân biệt chưa kết bạn với hộp người lạ, Unknown không bị suy thành StrangerBox.
4. **M3 — privacy/block/report:** đọc trạng thái server, mutation rõ ý nghĩa, ACK và
   hạn chế chattable/media; quyền nhận tin khác quyền hiển thị trực tiếp. Không tự bật seen.
5. **M4 — UI PR #105:** font/scale/màu, preview/reset, rà picker/reply/forward; giữ
   source protection và quy tắc không gửi lại sau mất ACK.
6. **M5 — hội thoại/danh bạ/hồ sơ (R9):** alias/label/archive/hidden-PIN, quản lý
   danh bạ/profile, mark-unread và presence theo privacy; nghiệm thu đồng bộ nhiều thiết bị.
7. **M6 — nhóm/cộng đồng (R10):** role/control realtime, rename/avatar/settings, link,
   duyệt/ban và invitation/quota; kiểm tra thay đổi quyền khi hội thoại đang mở.
8. **M7 — nghiệm thu/phát hành v0.008:** full tests, audit, headless/native DPI/theme,
   dữ liệu cũ và nhiều thiết bị. Release notes phân biệt hoàn thiện, thử nghiệm và blocker.

[UI spec 005](../specs/005-ui-customization/spec.md) ·
[Người lạ spec 006](../specs/006-stranger-inbox/spec.md) ·
[Call spec 007](../specs/007-call-multidevice/spec.md).

## Tách việc trong từng đợt

### R9 — hội thoại/danh bạ (v0.008-arm)

- Alias, label và archive: snapshot/version, CRUD, control cập nhật nhiều thiết bị,
  tìm kiếm/filter theo UID; không dùng tên làm khóa hoặc trộn local/server preference.
- Hidden PIN: xác minh contract và trạng thái unlock; rà search/export/notification/
  cache khi đang khóa, không coi hide local là bảo mật PIN hay mã hóa DB.
- Danh bạ/profile: remove/close-friend/QR/lookup, sửa profile/avatar/lịch sử avatar;
  mapping identity và thay đổi tên không mất tin. Presence tuân thủ privacy và có Unknown.
- Mark-unread là state server riêng, không sửa giả last-seen cursor chỉ để tăng badge.

### R10 — nhóm/cộng đồng (v0.008-arm)

- Hợp nhất role/membership/approval/ban/disband và control invalidation vào mô hình quyền.
- Rename/avatar/settings, link detail/new/disable/join, review pending, blocked members.
- Invitation box/community upgrade/quota theo capability; phản hồi lỗi không biến thành success.
- Nghiệm thu trưởng/admin/member/pending/banned, nhóm lớn và thay đổi quyền từ thiết bị khác.

### R11 — nội dung và media/call (v0.009-arm)

- Recorder voice: capture/playback preview/cancel, encode/upload/send, thiết bị bị rút,
  permission thất bại, ACK và identity; khác microphone trong cuộc gọi.
- Contact card: payload/preview/open/share xác minh; không thay danh thiếp bằng text giả.
- Batch forward/recall: UI multi-select, nguồn/quyền từng tin, giới hạn batch, kết quả
  từng phần; không gửi lại toàn batch khi chỉ mất một ACK.
- Sticker/GIF/effect: search/recommended/personalized theo contract; cache/budget và fallback.
- Screen-share: device/window selection, capture, encode, signaling, truyền/nhận frame,
  dừng chia sẻ giữ audio, group/multi-device và reconnect. Signaling có sẵn chỉ là nền.
- Viewer: rà streaming/range/seek/thumbnail/điều hướng/multi-window theo source và CDN;
  khả năng Range phải được kiểm chứng, không giả định mọi signed URL hỗ trợ.

### R12 — cộng tác/tự động hóa (v0.009-arm)

- To-do: tạo/sửa/giao việc/trạng thái/nhắc, ownership, danh sách/phân trang và control.
- Poll/share/voter; rich note; RSVP/event response và lịch lặp theo timezone/server rule.
- Quickmessage/auto-reply server: đọc/sửa/xóa, capability, opt-in và scope; không cho
  rule local cùng trả lời một event đã xử lý ở server hoặc lịch sử replay.
- Giữ phân biệt reminder, to-do, auto-reply và scheduled send; không tự dùng endpoint
  này thay chức năng khác chỉ vì payload tương tự.

### R13 — bảo mật/E2EE (v0.009-arm)

- Account management: session/device list/revoke, password/2FA và các challenge có bằng chứng.
- E2EE: identity/prekey/session/device roster, secure storage, ratchet/rekey/recovery,
  message/media/quote/forward/delete và nhóm theo capability thực.
- Mốc PoC bắt buộc: hai thiết bị trao đổi text đã mã hóa và giải mã đúng, sau đó media,
  reconnect/rekey/thiết bị đổi; có failure tests, review crypto/dependency và device acceptance.
- HTTPS/ParamCipher hoặc khóa truyền CSDL không được coi là PoC E2EE chat.
- Không mở E2EE mặc định trước khi đủ contract/keys và nghiệm thu; thiếu key/entitlement
  được ghi blocker thay vì downgrade âm thầm hoặc báo gửi thành công.

### R14 — lịch sử/zCloud (v0.010-arm)

- PoC trước: một file tài khoản được phép mở trên Zalo chính thức → xác thực viewer →
  download/unwrap/decrypt bằng khóa thiết bị hợp lệ → so đúng nội dung/hash.
- Sau PoC: list/search/page/restore, quota, URL renewal, account isolation, cancel,
  cache và kiểm soát dung lượng; không cam kết mọi ảnh cũ còn tồn tại.
- V1/BinNet: decode có fixture, sender/group mapping; giữ raw/Unknown phần chưa biết;
  không hardcode self UID hay suy từ sender phổ biến.
- Desktop V2: file thay giữa dry-run/import, batch rollback, assets/shared refs,
  cancellation/replay/delete marker; bảo toàn bản backup gốc.
- Server media library và local FTS tách nguồn, hiển thị phạm vi/cờ bị lọc; dữ liệu
  thiếu/expired không đồng nghĩa người dùng không có file ở thiết bị/zCloud khác.

### R15 — phần đặc thù (v0.011-arm)

- OA/Business/enterprise/catalog: xác minh đúng loại tài khoản, scope và entitlement,
  đọc trước mutation; dành tài khoản thử cho từng capability.
- Register/recovery/mode: chỉ hiện workflow PC/Web còn được hỗ trợ và challenge hợp lệ.
- LAN/transfer: đọc callsite/native flow và xác minh ý nghĩa trước; chưa suy là ZaloPay
  hoặc truyền LAN hoàn chỉnh. Bàn giao theo workflow xác minh, không theo tên API.
- Legacy/v2/server-map compatibility thuộc từng đợt: một hành vi có nhiều route không
  trở thành nhiều task tính năng; API bị bỏ cần quyết định có bằng chứng và phương án tương đương.

## Phụ thuộc và nghiên cứu sớm

- R0 độc lập: không chờ framework lớn, màu/font hay nghiên cứu cloud.
- R8 tạo state/permission nền; R9/R10 dùng chung nhưng vẫn bàn giao từng capability.
- Quyền nhóm R10 là điều kiện cho các thao tác nhóm nâng cao trong R11/R12.
- R13 key/device model là phụ thuộc cần kiểm tra cho phần E2EE của R14, không khẳng
  định zCloud dùng cùng khóa/giao thức với chat E2EE.
- Có thể nghiên cứu PoC E2EE/zCloud/Business sớm để lộ blocker; không để nghiên cứu
  chưa có kết quả chặn các bản thông thường hoặc tự bật nhiều agent/công cụ.

## Công việc nền xuyên suốt, làm khi cần cho từng đợt

1. API contract: service/method/params/response/error/ACK/cursor/capability + source và
   fixture; không lấy route trong bundle làm đủ bằng chứng để gọi mutation.
2. State/event: owner/account/epoch/generation/call-thread identity; sự kiện đến muộn,
   duplicate, reconnect và concurrent device. Unknown khác false/success.
3. DB/cache: migration forward-only, backup, bounded backfill/transactions, tombstone
   chống hồi sinh và quan hệ asset; local/server fields không nhập nhằng.
4. Diagnostics: metadata tối thiểu, rate-limit và correlation rút gọn; không full payload,
   token, cookie, signed URL hoặc nội dung riêng tư. Không mở telemetry mặc định.
5. API lỗi chưa rõ ACK: đối soát state, không tự gửi lại mutation. HTTP failover chỉ
   áp dụng nơi an toàn và không làm duplicate thao tác đã đến server.

## Định nghĩa hoàn tất và phát hành

Mỗi capability đi qua: bằng chứng → fixture/PoC → mã → integration/headless → native/
thiết bị đúng triệu chứng → đối chiếu nhiều thiết bị → changelog. Task có thể đã có mã
nhưng chưa đạt nghiệm thu; giữ status đó, không đổi nhãn để đủ release.

Bắt buộc kiểm thử âm tính: sai tài khoản/UID, stale response, thiếu quyền, timeout,
ACK không rõ, replay, cancel, offline/reconnect, DB cũ, cache/file expired và UI/DPI.
Thử tác vụ nhạy cảm qua UI/tài khoản thử được phép, không thực hiện trong CI tự động.
Release chỉ dùng artifact CI đúng commit, checksum/PE/audit và giới hạn đã công bố.

## Ước lượng và điều chỉnh

Đây là kế hoạch **4 phiên bản + các mốc công việc nội bộ, bắt đầu bằng sửa P0**, không cam kết ngày hoàn thành
khi chưa có contract/capture/key và tài khoản thử cho E2EE/zCloud/Business. Sau mốc
bằng chứng/PoC của từng đợt mới ước lượng lịch và tách commit/bản thử. R8/R9 dựa nhiều
trên nền hiện có; R10…R12 có rủi ro tích hợp/media cao hơn; R13/R14 là nghiên cứu mật mã/
khóa/dữ liệu, R15 phụ thuộc capability ngoài mã nguồn. Các khối này được gộp theo
bốn phiên bản đã chốt; dùng mốc nội bộ để kiểm thử tích hợp, giữ nguyên điều kiện nghiệm thu.

## Ma trận truy vết không bỏ sót phạm vi

ID tham chiếu trực tiếp [bảng gap](ZALO_FEATURE_API_GAPS.md).

| Đợt chính | Nhóm phải được bàn giao hoặc ghi blocker có bằng chứng |
| :--- | :--- |
| R8 — v0.008 | A01, A02, A03, A08, B02 |
| R9 — v0.008 | A04, A05, A06, A07, A10, A11, B01, B10 |
| R10 — v0.008 | A12, A13, A14, A15, B03 |
| R11 — v0.009 | A16, A17, B06, B08, B09, B12 |
| R12 — v0.009 | A18, B04, B07 |
| R13 — v0.009 | A09, A19 |
| R14 — v0.010 | A20, B05, B11 |
| R15 — v0.011 / nhánh đặc thù | C01 Business/OA/enterprise; C02 phần account còn lại; C03 LAN/transfer |
| Xuyên suốt | C04 legacy/service compatibility; B02 E2EE tiếp tục ở R13 |
| Ngoài 32 nhóm gap | R0: lỗi cuộc gọi spec 007; R8: UI PR #105 spec 005 |

Ma trận bao phủ đủ A01…A20 và B01…B12. Hành vi bị server loại bỏ hoặc tài khoản chưa có
quyền không được giả triển khai; cần quyết định/nguồn/blocker rõ và giữ truy vết.

## Quy tắc gộp phiên bản đã được người dùng chốt

- Phạm vi v0.008, v0.009, v0.010 trong phương án cũ → **v0.008-arm**.
- Phạm vi v0.011, v0.012, v0.013 trong phương án cũ → **v0.009-arm**.
- Phạm vi v0.014 trong phương án cũ → **v0.010-arm**.
- Phạm vi v0.015 trong phương án cũ → **v0.011-arm**.

Đây chỉ là bảng chuyển đổi từ phương án cũ, không phải các release còn dự kiến.

## Tra cứu roadmap bằng Graphify

Đồ thị phát triển đã bao gồm cả bốn release, gap, đặc tả/task và bindings mã/test.
Xem [hướng dẫn](GRAPHIFY.md#roadmap-và-truy-vết-yêu-cầu). Tái tạo bằng
`scripts/graphify.sh`; báo cáo ở `graphify-out/ROADMAP_REPORT.md` (không commit).
Đồ thị là chỉ mục từ tài liệu gốc, không tự đóng task hay thay nghiệm thu thiết bị.
