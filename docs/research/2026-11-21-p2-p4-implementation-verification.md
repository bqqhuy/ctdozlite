# Triển khai P2–P4 và nghiệm thu Windows ARM64

Ngày: 2026-11-21. Working tree chưa commit, giữ baseline P1 và các sửa dở có sẵn. Không push/tag/release.

## Phạm vi và audit

- P2: bỏ full snapshot khỏi từng outgoing Pending/ACK; lưu metadata một thread có owner/runtime; SQLite hydrate preview/ACK/tên và sticky responded_by_me không phụ thuộc preload window.
- P3: bounded socket relay + control/typing priority; name lookup và reaction drain tách khỏi ordered page; cursor yêu cầu messages + durable reaction enqueue + receipts thành công; overflow kết thúc epoch để replay từ cursor bền vững. Reconnect refresh toàn bộ account metadata cho native control không có message cursor.
- P5a liên quan: composer đọc modifier từng event và phân ranh Enter trong burst; buffer hủy khi đổi focus/context/modal, không replay pointer; banner budget/popup anchor/avatar chung/call-card; native picker STA thread.
- P4 instrumentation: opt-in CTDOZ_PERF=1, fixed numeric metric labels, 512 samples/metric, không serialize nội dung/ID/tag/URL. GuiFrameWork đo thời gian update CPU, không phải present/display frame latency. DispatchToFirstOutcome đo khi apply app event đầu tiên, không hứa Pending/ACK.
- Hai subnet audit độc lập A/B đã cùng Accept mã sau sửa cache-only human name loss, pointer-tail replay, receipt durability, control/typing priority và metric label. Audit không chứng nhận native interoperability.

## Kiểm tra thực chạy

- cargo check --tests --locked -j 1: qua.
- cargo test --locked --lib -j 1: 1.021 passed, 0 failed, 11 ignored. Lần đầu 1 regression mới sai kỳ vọng Pending sau reload; sửa assertion theo P1 restore Pending→DeliveryUnknown, không sửa product.
- cargo clippy --all-targets --locked -j 1 -- -D warnings: qua sau dọn needless borrows.
- cargo fmt --all -- --check và git diff --check: qua.

## Benchmark fixture đã chạy

Windows ARM64, aarch64, SQLite :memory:, 300 threads / 6.001 messages, 10 samples. So cùng source đường cũ clone+SQL+JSON full snapshot và đường targeted row. Không gồm DB-open, SQLite busy, disk flush; không suy thành tỷ lệ cải thiện network/native.

| task | min_ms | avg_ms | p50_ms | p95_ms | p99_ms |
|---|---:|---:|---:|---:|---:|
| full_snapshot_clone_sql_json | 7275.4325 | 7673.3928 | 7284.0121 | 11037.8777 | 11037.8777 |
| single_outgoing_row_sql | 1.7013 | 1.90921 | 1.8093 | 2.4439 | 2.4439 |

Lệnh: cargo test --locked --lib benchmark_full_snapshot_versus_single_outgoing_row -j 1 -- --ignored --nocapture — passed.

## Đối chiếu attachment với Zalo PC

- **Zalo PC/Web (E2, `re-notes/17-zalo-pc-sync-config-and-reconnect.md`, `re-notes/18-typing-receipts-and-pr78.md`)**: file/video đi qua upload bất đồng bộ (`asyncfile/upload` hoặc video chunk upload), chờ control `file_done`/URL rồi mới gọi request gửi (`asyncfile/msg`); ảnh dùng cặp `photo_original/upload` → `photo_original/send`. Upload không được chặn vòng render/chat.
- **Clipboard**: ảnh dán được giữ như attachment draft trong composer và chỉ chuyển sang upload sau Enter. File tạm là file nội bộ của ứng dụng, không được coi là file người dùng để cleanup.
- **Drag & drop**: thả vào composer là prepare/preview rồi người dùng xác nhận; thả vào vùng timeline là intent gửi ngay. Fork hiện có hai nhánh UX này.
- **Fork**: picker menu dùng `PickAttachmentDraft` rồi `SendDroppedFiles`; clipboard dùng `PickAndSendAttachment`; cả hai cuối cùng gọi `send_attachment_batch`. Vì vậy lỗi native cần phân biệt picker/draft, clipboard hoặc upload/send. Chưa có E1 chứng minh crash thuộc endpoint upload.
- **UI parity mới từ ảnh native người dùng cung cấp (E1 UI)**: nút ảnh và nút attachment của Zalo PC chọn xong gửi trực tiếp; drag/drop vào composer mới giữ draft để xác nhận. Draft hiển thị tile ngang, thumbnail/icon loại file, tên rút gọn, dung lượng, xoá từng mục, xoá tất cả và ô `+` thêm file. Ảnh chọn qua attachment có thể được gửi như file ảnh với icon file ở timeline; không suy thành photo bubble chỉ từ phần mở rộng.
- Fork đã đổi picker của hai nút toolbar sang `PickAndSendAttachment` có owner/runtime fence; drag/drop composer vẫn dùng draft. Preview draft đã chuyển từ danh sách dọc sang tile ngang tương ứng ảnh tham chiếu. Giới hạn cục bộ vẫn là 16 file/hội thoại, 64 file toàn bộ; **chưa có bằng chứng E1/E2 đủ để nâng lên 50**, nên không đổi mù.
- Cập nhật theo E1 người dùng: giới hạn một lần và tổng draft đều là **50 file**, áp dụng ảnh/video/file. Nút ảnh/attachment chọn xong gửi trực tiếp; drag/drop vào composer vẫn preview để xác nhận.

## P5 và voice trong lượt hiện tại

- Preview attachment draft đã chuyển sang tile ngang, có thumbnail/icon, tên rút gọn, dung lượng/trạng thái, xoá từng mục, xoá tất cả và tile thêm file.
- Banner sync dùng nền palette đặc, tránh dải đen do màu trong suốt trên clear surface.
- Voice bubble có nút `AI` trực tiếp cạnh player để gọi flow voice-to-text đã có consent/owner guard. Voice upload native vẫn **BLOCKED-EVIDENCE**: source hiện ghi WAV và gửi qua attachment; route `voice/upload`/`voicemp3` có trong route index nhưng chưa đủ schema/ACK live để chuyển mù sang native protocol.
- P5 audit delta: reaction emoji không nhận diện được nay giữ glyph gốc thay vì rơi sai thành Like (`src/gui/theme.rs`), có regression test riêng.

## Native test checklist cho build tiếp theo

1. Chọn nút ảnh: chọn 1 ảnh, 1 video, 1 file; xác nhận gửi ngay, bubble Pending xuất hiện tức thì, timeline hiển thị icon file đúng như Zalo gốc.
2. Chọn 50 file một lần; chọn file thứ 51; xác nhận 50 được nhận và 51 bị chặn rõ ràng, không crash.
3. Drag/drop vào composer: preview tile ngang, xoá từng file, `Xóa tất cả`, `+ Thêm`, nhập caption rồi Enter.
4. Drag/drop vào timeline: gửi ngay; thử nhiều file và kiểm tra từng bubble/outcome.
5. Clipboard ảnh: preview cùng composer, huỷ bằng Esc, gửi bằng Enter; không xoá file người dùng.
6. Gửi liên tiếp file/ảnh/video trong lúc history sync; bubble Pending phải xuất hiện trước upload và không làm mất tin text.
7. Ghi âm: gửi một voice, kiểm tra nó hiện là voice bubble, nút play/AI nằm trong timeline; bấm AI và kiểm tra consent, loading, transcript hoặc lỗi rõ ràng.
8. Kiểm tra server error 204/timeout: trạng thái Failed/DeliveryUnknown đúng, không tự gửi lại; kiểm tra hội thoại trước retry thủ công.

## Native retest findings from build `(152)`

- UI capture still showed a dark/black strip between the conversation list and chat while sync status was visible. The prior banner-only fix was insufficient; the latest patch also sets egui window spacing to zero and must be rebuilt before acceptance.
- Dropping more than 50 files previously produced no visible feedback because the rejection only wrote `status_msg`; it now uses the toast path.
- Clipboard send still needs a reproducible crash trace. The latest patch validates the temporary path before enqueue and keeps the preview when the path is missing.
- File uploads still report server rejection code 204 on the user's account. The code now records only the typed rejection code; native retest must identify whether it occurs during chunk upload or `asyncfile/msg` before any protocol change.
- Voice remains the known fork deviation: generic WAV attachment plus media fallback player. Native voice upload/player parity is not claimed.

## Build và native

Release ARM64 đã build thành công:

- Binary: `target/aarch64-pc-windows-msvc/release/ctdozlite.exe`
- Size: 43,747,840 bytes; PE machine `0xAA64`
- SHA-256 (final rebuild after P5/audit delta): `342EE3E938C6D0AA43524D82E0CD101F5A97083E2A414D085ABB669AEC3E0EC5`
- Chạy được GUI, gửi liên tiếp hai tin và kiểm tra Shift+Enter/Control+Shift+S.
- Native P4 **không đạt**: khi thử luồng attachment, người dùng báo ứng dụng văng. Process không còn sau lần thử; Windows Application log không có event `ctdozlite`, và Computer Use bị dừng bằng phím Escape trước khi có capture chọn-file hoàn chỉnh. Chưa thể quy trách nhiệm chính xác cho picker, draft hay upload/send.

## Giới hạn còn theo dõi

Queue giới hạn theo số event (512 relay + 32 downstream/priority), chưa byte budget. Cursor write fail giữ mốc cũ; restart có thể replay. Invalidation metadata vẫn có task DB nền theo control; chưa có workload flood chứng minh fairness toàn hệ thống. Tác vụ history phải được đo cạnh realtime trên binary mới. Những kịch bản media/video/call/phone approval không được đóng bằng unit tests.
