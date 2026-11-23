# Sync2: origin, timeout và mốc hoàn tất preload

Ngày: 23/11/2026. Phạm vi: bốn phát hiện còn lại sau audit vòng 5, working tree trên nền `f33f43a`.

## Bằng chứng PC (E2)

Artifact PC 26.11.12 đã đọc trực tiếp, SHA-256:

- `compact-app-pc.0cd2e653e67efaa4bf4b.js`: `78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08`.
- `main-startup.41abf8e6af2a55551e3b.js`: `c29c9f0a7f0718c5b044764666b6fc5dec8b8eb65e178819b3cee3b1f93e3426`.

Offset dưới đây là chỉ số Unicode code point, bắt đầu từ 0; marker dùng để tìm lại khi bundle thay đổi.

| Marker / điểm neo | Hành vi xác minh |
| --- | --- |
| `isFeatureEnabledBySyncSource(e)`, main-startup CP 2519126 | Gate chung Sync2; `SETTING` dùng `enable_on_sync_source_setting`, `AUTO_SYNC` dùng `auto_sync.enable`, các source Sync2 còn lại dùng gate chung. |
| `getTimeoutConfigs(e)`, main-startup CP 2521607 trong thân hàm | AUTO dùng `auto_sync.awaiting_mobile_timeout_ms`, `wakeup_timeout`, `batch_timeout`; source khác dùng bộ `manual_sync.*`. |
| Object `auto_sync`, main-startup CP 2529533 | Defaults lần lượt 600000, 180000, 10000 ms; giống defaults manual nhưng là các trường cấu hình độc lập. |
| CTA `callback:()=>t.sync(rA.t.BANNER_SUGGEST_SYNC)`, compact CP 13203765 | Banner không phát nguồn `SETTING`. Vì vậy tắt gate Settings không được tự tắt thao tác banner. |

Không đổi route, frame ID, payload, ký số hay mã hóa. Parser giữ các giới hạn timeout cục bộ đã có; không suy diễn ý nghĩa các giá trị ngoài giới hạn này.

## Áp dụng trong fork

- `SyncOrigin` đi từ `WorkerCmd` đến entry point transfer. Transfer chụp một `SyncTimeoutConfig` theo origin khi operation bắt đầu, dùng cùng snapshot cho deadline chờ điện thoại, wake grace và batch receive trong các consumer tương ứng và khi reconnect. Round metadata đầu vẫn dành confirm budget cho người dùng xác nhận, không bị batch timeout cắt trước hạn. Thay config sau đó không đổi chính sách giữa chừng của operation đang chạy.
- Banner: parity với PC ở gate nguồn; quyền đăng nhập, DB, socket và chống chạy đồng thời vẫn do worker kiểm tra.
- **Fork deviation — Legacy V1:** khôi phục lựa chọn V1 chủ động, độc lập với cờ riêng `sync_2_pc.enable`, theo H-01 và handoff hiện hữu. Đây không phải khẳng định về entitlement V1 của PC. Không tự fallback, không bỏ kiểm tra quyền/DB/socket và không đổi giao thức V1.

## Hoàn tất startup

PC giữ loading theo lifecycle Start/PreProcessingSettle và cấu hình `o2o.keep_loading_during_preload` (nguồn E2 chi tiết trong báo cáo kiểm chứng ngày 22/11).

**Fork implementation:** sau DB/auth bắt buộc, worker phát `StartupPrerequisitesSettled`, rồi thực sự await tác vụ hydrate peer local qua `spawn_blocking`. Chỉ khi tác vụ trả về thành công/lỗi/skip mới phát `StartupPreloadSettled`. Lỗi optional được ghi log và cho settle; không biến thành lỗi DB/auth. Timeout UI có thể mở bốn zone khi optional còn chạy, nhưng không giả lập completion và không bỏ qua prerequisite.

Payload peer có owner/runtime; runtime đã đóng không được công bố completion hay cập nhật peer ở UI. Đã bỏ hai lần hydrate fire-and-forget trùng trong nhánh QR/password. Kiểm tra auth dùng clone session sau khi apply config mới.

## Kiểm chứng và giới hạn

Regression: fixture native có manual 10s / AUTO 45s; ma trận gate Banner/V1 cùng điều kiện DB/socket; producer optional bị giữ bằng oneshot barrier nối đến GUI; optional lỗi, runtime đóng, late completion và peer payload sai runtime.

Kết quả lệnh thực chạy được ghi tại audit dossier. Fixture/harness không phải E1 hoặc nghiệm thu native. `ChangeDevice`/`QueueEvict`, ZIP-08 và sync thực với điện thoại vẫn chưa được xác nhận trong đợt sửa này.


## 23/11/2026 — cache bắt buộc trước Ready và bảo toàn cập nhật trực tiếp

Đây là sửa lifecycle/cache nội bộ của fork; không thêm hợp đồng server hay thay wire/cipher. `StartupPrerequisitesSettled` mang cache hội thoại và UI merge trước khi mở Ready; `StartupPreloadSettled` chỉ xác nhận optional hydrate hoàn tất. Producer thật bị giữ bằng barrier để kiểm tra keep-loading tắt/bật, timeout 800/2000ms và completion trễ.

Snapshot cache đến sau cập nhật trực tiếp dùng merge riêng: giữ thông tin liên hệ/unread và trạng thái message đang có, bổ sung lịch sử hoặc plaintext local cho placeholder, không hồi sinh tin bị xóa cục bộ và không đảo recall. Intent chờ ACK được retire nếu channel worker đóng; không resend tác vụ có kết quả chưa rõ. Parser bỏ qua node sync sai kiểu.

Stress test cũng tìm thấy app bootstrap=false khởi chạy worker khi logout/profile switch. Cờ bootstrap được giữ qua transition để fixture không vô tình chạy network/chen lệnh sang test khác. Production bootstrap=true giữ nguyên.

Nguồn PC E2 ở trên vẫn chỉ hỗ trợ parity gate/timeout/loading lifecycle. Các quy tắc merge/cache và mất channel là biện pháp bảo toàn nội bộ, không tự nâng thành bằng chứng E1. Kết quả hai lượt audit cuối được ghi tại dossier.

Stress mở rộng còn phát hiện mock mailbox toàn cục bị nhiều app test truy cập. Dưới cfg(test), mailbox được tách thread-local theo UI test thread và có regression kiểm tra isolation; nhánh production vẫn dùng Mutex toàn cục như trước. Không giảm assertion chống gửi lặp hay chạy toàn suite với một thread để che race.
