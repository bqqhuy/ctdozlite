# Đối chiếu và xử lý audit Claude — 09/11/2026

Baseline: `a610cbf3c314450f7b6b0de513e244685876e0ec`, bao gồm ba thay đổi
chưa commit của Claude trong doh_resolver.rs, media.rs và media_download.rs.
Bản đề xuất gốc có 20 mục nếu tính cả mục fail-open đã loại; nhiều mục chỉ là
nhóm checklist, một mục để reserved. Không phải 20 lỗi đã tái hiện.

## Kết quả năm bước được giao

1. **Compiler:** sửa bản thay thế log gọi sai chữ ký `voip_log::line`, tham chiếu
   `profile_id`, `peer_id` và variant `Via::Local` không tồn tại. Dùng đường `report_media_error`
   hiện hữu; không thêm dependency log. Bản trước đó dùng `log::error!` cũng không
   compile vì crate chưa được khai báo.
2. **Mutex:** cache DNS/client là dữ liệu có thể dựng lại: xóa mọi entry sau poison,
   clear poison dưới lock rồi mới dùng lại. Không nuốt lỗi invalidate hoặc làm DNS
   hỏng vĩnh viễn. Tạo client ngoài lock; dùng chung resolver khởi tạo có Result,
   bỏ hai expect trên đường tải. Buffer âm thanh không phục hồi trạng thái sau panic:
   trả lỗi, báo GUI, thoát loop; StopOnExit hiện hữu dừng tác vụ media liên quan.
3. **Preview:** bỏ exists/read; thử đọc trực tiếp, fallback khi cache mất/không đọc
   được/rỗng. Dùng cùng key hội thoại + message như attachment download thay tên
   file đơn thuần. Cache lẫn mạng đều giới hạn 1 MiB. Mạng dùng transport CDN đang
   có (allowlist, redirect, timeout, HTTP status); lỗi giữa stream không thành công
   giả. Worker kiểm tra session, quyền xem hội thoại và expiry trước/sau tải.
4. **Clippy:** HIGH-1 và HIGH-2 đã sửa trong commit f1b152f; không áp dụng lại.
5. **Bằng chứng:** phân loại toàn bộ mục bên dưới. Không thay TLS, crypto hay sửa
   hàng loạt theo ví dụ không có trong source. Giữ feature experimental-e2ee,
   runtime --e2ee-experimental và log đã che dữ liệu.

## Phân loại toàn bộ nhận định gốc

| Mục | Kết luận / bằng chứng | Xử lý |
| :--- | :--- | :--- |
| CRITICAL-1 fail-open | Sai: is_thread_e2ee trả true khi lỗi để chặn plaintext | Giữ fail-closed |
| CRITICAL-2 pinning | Không có exploit được tái hiện; HTTPS/WSS cấu hình Rustls/WebPKI roots, không phải OS roots như mô tả | Không thêm pin cho server bên thứ ba chưa có chính sách xoay khóa |
| CRITICAL-3 mutex | HEAD có unwrap ở cache DNS/client và buffer audio; các tên WEBSOCKET_EPOCH/APP_STATE/MESSAGE_CACHE nêu trong mẫu không tồn tại; chưa chứng minh cascading DoS từ xa | Hardening cụ thể như bước 2, test poison; unwrap trong test không phải lỗi runtime |
| HIGH-1 items_after_test_module | is_placeholder đứng trước module tests | Đã sửa từ f1b152f |
| HIGH-2 needless_update | AccountSwitcher chỉ khởi tạo expanded, không còn update thừa | Đã sửa từ f1b152f |
| HIGH-3 TOCTOU | Ví dụ crypto DB không khớp source; worker preview có exists/read, cache đọc lỗi không fallback | Sửa bước 3; phân loại lỗi phục hồi local, không có bằng chứng High security |
| MEDIUM-1 FFI null | src/crypto/ffi.rs không tồn tại; không chỉ ra hàm/đầu vào gây UB; DPAPI/clipboard có check ở đường đã đối chiếu | Chưa xác minh; không chèn null check hàng loạt |
| MEDIUM-2 leakage | Không có log line chứa secret hoặc fixture tái hiện | Chưa xác minh; không dùng regex xóa nội dung lỗi chung |
| MEDIUM-3 O(n²) | Không có loop/dataset cụ thể; merge lookup trước đó đã sửa trong f1b152f | Chưa xác minh lỗi mới |
| MEDIUM-4 magic numbers | Không có hằng số cụ thể gây sai hành vi | Checklist, chưa phải bug |
| MEDIUM-5 TODO | Không có TODO/caller chứng minh feature hỏng | Checklist, chưa phải bug |
| MEDIUM-6 unsafe comments | Không có site cụ thể trong báo cáo; thiếu comment không tự chứng minh UB | Cần site + invariant; không đánh dấu toàn repo đã an toàn |
| MEDIUM-7 reserved | Không có nội dung | Bỏ khỏi số lỗi |
| LOW imports | Không có import cụ thể | Đối chiếu compiler/Clippy |
| LOW dead code | Không có symbol/caller cụ thể | Đối chiếu compiler/Clippy |
| LOW signatures | Không có function cụ thể | Đề xuất phong cách chưa xác minh |
| LOW public docs | Không có API cụ thể | Checklist chưa xác minh |
| LOW error types | anyhow ở boundary là chuẩn repo; thiếu trường hợp sai cụ thể | Không chuẩn hóa hàng loạt |
| LOW i18n | Không có string/caller cụ thể | Checklist chưa xác minh |
| LOW coverage | Không có invariant chưa được kiểm tra cụ thể | Thêm hồi quy trực tiếp cho các thay đổi được xác minh |

Một phát hiện mới chỉ được nhận vào danh sách sửa khi có: SHA/working tree, vị trí,
luồng gọi, điều kiện kích hoạt, expected/actual, bằng chứng tái hiện và mức nghiêm
trọng theo tác động thực tế. Không coi mục chưa chứng minh là đã sửa hoặc không
thể tồn tại lỗi; cần bổ sung bằng chứng rồi mở lại.

## Kiểm tra và giới hạn

- Hồi quy poison: positive/negative DNS cache dựng lại được, stale entries bị xóa;
  audio poison trả lỗi và không tái dùng sample.
- Preview: cache lớn bị giới hạn, cache mất/không đọc được/rỗng được tải lại;
  HTTP lỗi, stream thiếu và scheme sai bị từ chối; download lớn chỉ giữ prefix.
- Benchmark `media_download::tests::measure_document_preview_cache`: bảy vòng,
  cache 8 MiB, so sánh cách đọc cũ và prefix 1 MiB; không đại diện throughput mạng.
- Test dùng temp dir và loopback giả, không dùng session hoặc dữ liệu thật.
- Chạy cargo test --locked và cargo test --locked --features experimental-e2ee;
  tên feature e2ee_trial trong tài liệu gốc là sai.
- Chạy strict Clippy toàn target/feature, fmt, audit-prepush, ARM64 check, cargo audit.
  Kết quả được ghi sau khi hoàn tất; native build GitHub dùng đúng SHA trên main.
- Không tạo tag/release. Build không chứng minh E2EE tương thích Zalo hoặc nghiệm
  thu cuộc gọi thật. Không đổi schema crypto hoặc trạng thái opt-in của người dùng.

Tham chiếu: re-notes/42-claude-review-followup.md; docs/E2EE_TRIAL.md.

### Kết quả trước commit

- Bộ mặc định: 819 passed, 0 failed, 13 ignored.
- Feature experimental-e2ee: 822 passed, 0 failed, 13 ignored.
- Sau đó sửa assertion số byte đọc trong HTTP fixture theo Clippy; không đổi logic app.
- Clippy --all-targets --all-features -D warnings, audit-prepush, ARM64 all-target/
  all-feature check, fmt/diff check: đạt.
- cargo audit: 0 vulnerability; 4 cảnh báo unmaintained và 1 unsound transitive
  (audiopus_sys, paste, proc-macro-error, ttf-parser, glib) vẫn còn như baseline.
- Heuristic UI-stub của audit-prepush vẫn nhắc nhánh từ chối gửi tệp kèm mention;
  nhánh hợp lệ thực sự dispatch WorkerCmd. Đây không phải control giả.
- Native test/build, artifact và SHA cuối được xác nhận trong workflow GitHub
  tương ứng commit; không suy kết quả build từ việc dispatch workflow.
