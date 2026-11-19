# Audit code và logic — 2026-11-19

Auditor độc lập: code_quality_audit. Snapshot bắt đầu: main 50d6abe; các sửa
history/call parser trong workspace được review chéo riêng. Đây là audit tĩnh có
giới hạn, không phải chứng nhận toàn bộ dự án không lỗi. Không chạy Cargo trong
vòng audit rộng này vì phát hiện test logout chưa cô lập.

## Finding xác minh

### P1 — test logout có thể xóa phiên thật

- tests/audit_regressions.rs:773–799 ghi fake account credentials bằng storage
  mặc định và gọi profiles::perform_account_logout, nhưng test binary không thiết
  lập CTDOZ_DATA_DIR.
- src/profiles.rs:167–172 gọi storage::remove_session() trên phiên global.
- Tên test có chữ isolated không tạo isolation. Default cargo test có thể xóa
  session.dat thật và ghi tài khoản giả vào registry thật.
- Sửa: chạy body trong child process được cấp CTDOZ_DATA_DIR từ trước khi khởi
  tạo storage; không đổi biến môi trường chung từng test trong binary chạy song song.
- Trạng thái: history_fix đã đặt mọi audit_regressions fixture trong child process
  có root tạm từ startup; auditor đã review helper và synthetic global credential
  assertion. Chưa ghi nhận kết quả Cargo trong báo cáo này.

### P1 — low-level link send dùng E2EE policy fail-open

- src/chat.rs:3171 gọi e2ee_integration::is_thread_e2ee.
- src/e2ee_integration.rs:125–126 chuyển lỗi đọc policy thành false bằng
  check_thread_e2ee(...).unwrap_or(false).
- src/chat.rs:3256 có thể gửi endpoint link thường nếu DB policy lỗi. Public
  send_link_message gọi trực tiếp helper này; kiểm tra ở outgoing UI không bảo vệ
  mọi caller hoặc lỗi DB trong lần kiểm tra tiếp theo.
- Các send boundary khác dùng require_plaintext_send tại src/chat.rs:1847,
  giữ lỗi DB bằng Result. Dùng helper này cho link là sửa nhỏ, không đổi schema API.
- Kiểm chứng cần có: policy store lỗi phải không phát request HTTP.
- Trạng thái: history_fix đã thay guard bằng require_plaintext_send và thêm
  loopback regression cho policy DB không đọc được; code review đạt, chờ parent
  chạy test.

### P2 — danh sách to-do không scope theo hội thoại

- GuiEvent::GroupTodosLoaded ở src/gui.rs:705 chỉ mang danh sách, handler
  src/gui.rs:6228 ghi thẳng self.tools.group_todos.
- WorkerCmd::FetchTodos ở src/gui/worker.rs:4532 chạy task nền theo thread_id,
  nhưng kết quả không mang thread/request identity.
- src/gui/tools_panel.rs:607 gửi thread đang chọn, rồi hiển thị group_todos tại
  dòng 615. Đổi A sang B khi request A đang chạy có thể hiển thị việc của A trong
  công cụ của B. Cần scope thread/runtime ở event và verify tại GUI receive.
- GroupPollsLoaded có hình dạng tương tự; cần kiểm tra panel/call sites trước sửa.
- Trạng thái: code_quality_audit đã thêm runtime/thread identity vào cả hai event
  và cả bốn emit sites; GUI reject response khác phiên/hội thoại trước mutate,
  panel xóa danh sách cũ khi đổi group. Thêm regression GUI consumer; parent
  điều phối format/test và auditor parity kiểm tra chéo.

## Finding đã xử lý trong diff được review chéo

- Parser lịch sử thiếu recommened.calltime; các params string/numeric-string
  không được đọc đầy đủ.
- Phân biệt MISSCALL/CALLTIME và reason caller cancel khác callee reject.
- params.video=false/null không được coi là video chỉ vì tồn tại key.
- history_open::fetch lỗi bị unwrap_or_default nuốt; local history vẫn được render
  nhưng trạng thái cuối phải giữ việc server fetch thất bại.
- ConversationHistoryLoaded mang runtime_id và GUI kiểm tra trước mutate status
  hoặc normalize thành ThreadsUpdated. Đây là kiểm tra ở consumer, không chỉ producer.

## Kiểm tra không xác nhận lại lỗi cũ

- src/outgoing.rs:645 chỉ xóa HD copy khi hd_source tồn tại; không còn heuristic
  tên clipboard_ để xóa file user trong outgoing::send.
- src/gui/worker.rs:6610–6616 chỉ cleanup attachment ephemeral trong account temp
  hoặc root temp; nguồn user thông thường không được xóa khi batch.ephemeral=false.
- Open download folder trong src/gui/modals.rs:1954 nằm trong std::thread::spawn;
  không coi đây là disk I/O trên UI thread.
- MediaKey trong src/media_resources.rs bao gồm account/thread/message/quality;
  key hash có length prefix. Reader/cache có budget, không có bằng chứng từ mẫu
  đọc này cho việc media cache trộn tài khoản.
- Logout và SwitchProfile thay worker/event channel; không kết luận mọi unscoped
  event có thể xuyên tài khoản chỉ từ việc thiếu owner trong struct.

## Phạm vi và giới hạn

Đã đọc/rà soát tập trung outgoing send/cleanup, E2EE low-level guards, logout
test, profiles/session cleanup, media cache budget/key, history worker, các task
detached, tools to-do/poll events và mẫu filesystem call trong GUI.

Chưa review từng dòng của 159 Rust source files; chưa xác minh tất cả native
unsafe/Media Foundation paths, queue ordering, server response contracts hoặc
dead code toàn dự án. Không chạy tài khoản thật, không gửi tin/gọi điện, không
khẳng định capture E1 hoặc parity thiết bị thật từ phân tích tĩnh.

Đối chiếu E2 bundle Zalo PC 26.11.12: recommended call renderer đọc params JSON,
status từ action misscall, subject từ isCaller và fallback action, audio/video
từ calltype, reason enums 0..5. Contract server mới và fetch endpoint lịch sử
độc lập chưa được chứng minh.

## Bảng phủ rà soát cuối

Đã liệt kê 159 file Rust dưới `src/`. Bảng này ghi nhận phạm vi đọc, không phải
chứng nhận toàn bộ module an toàn hoặc không còn lỗi.

| Nhóm | Bằng chứng đã xem | Giới hạn còn lại |
| :--- | :--- | :--- |
| Storage / account isolation | `storage.rs`: root override, account hash, temp cleanup, atomic replace, DPAPI current-user | Chưa thử crash/Windows filesystem race thực tế |
| Attachment cache | `attachment_cache.rs`: key, symlink-root rejection, reservation và budget/pruning | Chưa kiểm thử đồng thời mọi downloader |
| Desktop backup | `desktop_backup_format.rs`: header, giới hạn TAR, path traversal/reserved names/duplicate destinations | Chưa phủ toàn bộ extraction, TOCTOU và native path |
| Hidden chat | `hidden_chat.rs`: owner-bound snapshot, fail-closed readiness, Zeroizing, PIN throttling | Không phải mã hóa DB; chưa đọc toàn bộ mutation/API |
| Diagnostics | `logger.rs`, đoạn dump exchange trong `session.rs`: preview/redaction, bounded log | Chưa chứng minh mọi call site và toàn bộ redactor |
| Updater | URL/checksum, bounded metadata/download, canonical launch path và hash kiểm lại | Chưa thử native launch/race thực tế |
| E2EE key persistence | Call sites `storage::protect/unprotect` trong `crypto/store.rs` | Ratchet/transaction crash state chưa audit toàn bộ |
| Native integration | Đã lập danh sách nơi có `unsafe`/Win32/COM | `windows_video`, `video_player_windows`, clipboard ownership, tray/notifications/auth guard cần vòng chuyên sâu |
| Phone sync / import | Đã lập danh sách filesystem/deletion surfaces | Chưa phủ toàn bộ transfer/import/rollback và dữ liệu thiết bị |
| Stub / dead code scan | Không tìm thấy `todo!` hoặc `unimplemented!` trong `src/` | Không chứng minh không có UI stub, ignored payload hoặc code chết |

Không có thêm P1/P2 được xác nhận trong mẫu security/storage của lượt cuối.
Các điểm chưa phủ cần được giữ là khoảng trống kiểm chứng, không chuyển thành
finding suy đoán và không tuyên bố đồng thuận “không còn lỗi” toàn dự án.
