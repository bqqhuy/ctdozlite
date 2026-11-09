# Bằng chứng: Vòng đời tin nhắn, media và reaction

Ngày ghi nhận: 2026-11-05 (Asia/Saigon). Đây là baseline từ phiên trước, không phải
kết quả chạy lại cho thay đổi tài liệu/Spec Kit. [Bản ghi phiên](../../docs/releases/v0.007-arm-work.md)
báo 565 passed, 10 ignored; Clippy/fmt/diff check đạt. Log gốc ở workspace,
không được coi là artifact phát hành; cần chạy lại ở commit chuẩn bị tag.

## Mã và fixture đã nhận diện

`src/message_lifecycle.rs, src/conversation_policy.rs, src/reaction_pipeline.rs, src/desktop_backup.rs, src/gui/worker.rs`

- `history_limit_is_applied_after_expiry_even_when_purge_batch_is_exhausted`
- `expiry_deletes_search_projection_and_rejects_replay`
- `queued_previews_cannot_reappear_after_expiry_or_account_change`
- `a_drain_preserves_distinct_targets_and_keeps_latest_state_per_target`
- `restores_jxl_without_a_remote_url`

## Kết luận theo bằng chứng

Có mã và fixture cho một phần yêu cầu. Chưa xác nhận toàn tính năng trên thiết bị.
Bộ kiểm thử cục bộ chưa chứng minh đầy đủ cleanup cache thường/native Windows; danh sách reactor chỉ gồm UID thật đã nhận.

## Nhật ký lần kiểm chứng tiếp theo

| Ngày / commit hoặc định danh working tree | Yêu cầu / task | Lệnh hoặc kịch bản | Kết quả và phạm vi | Giới hạn / thiết bị |
| :--- | :--- | :--- | :--- | :--- |

Chỉ thêm kết quả đã chạy; ghi test failed/ignored. Không điền bảng bằng dự đoán.
Không ghi khóa, cookie, URL có chữ ký hay nội dung tin của người khác.


## Kiểm chứng bổ sung 09/11/2026

Working tree trên `f842dfeb84fd94ff7ea79dc8d6b88dc9ad9d94bc`, theo yêu cầu sửa audit.
M-01/M-03: account, persistence và media có thay đổi liên quan được mô tả trong
[bản ghi khắc phục](../../docs/audits/2026-11-09-remediation.md).
Suite đầy đủ: 803 passed, 11 ignored; Clippy và check ARM64 đạt. Ignored được
chạy riêng; lỗi H.264 ban đầu đã có bản sửa và test native x64 chạy lại đạt.
UI smoke có PNG ở hai kích thước/theme và đã kiểm tra trực quan.
Không đóng task thiết bị hoặc chứng nhận Zalo interoperability từ các kết quả này.
