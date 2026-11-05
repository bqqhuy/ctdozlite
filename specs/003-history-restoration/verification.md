# Bằng chứng: Khôi phục lịch sử V1 và backup Desktop V2

Ngày ghi nhận: 2026-11-05 (Asia/Saigon). Đây là baseline từ phiên trước, không phải
kết quả chạy lại cho thay đổi tài liệu/Spec Kit. [Bản ghi phiên](../../docs/releases/v0.007-arm-work.md)
báo 565 passed, 10 ignored; Clippy/fmt/diff check đạt. Log gốc ở workspace,
không được coi là artifact phát hành; cần chạy lại ở commit chuẩn bị tag.

## Mã và fixture đã nhận diện

`src/phone_sync/legacy.rs, src/phone_sync/assets.rs, src/desktop_backup.rs, src/desktop_backup_format.rs, tests/desktop_backup_v2.rs`

- `v1_key_and_segment_decryption_match_an_openssl_known_answer`
- `xz_stream_respects_the_decoder_memory_budget_and_rejects_truncation`
- `old_public_key_and_other_computer_controls_cannot_finish_current_session`
- `tests/desktop_backup_v2.rs`
- `restores_jxl_without_a_remote_url`

## Kết luận theo bằng chứng

Có mã và fixture cho một phần yêu cầu. Chưa xác nhận toàn tính năng trên thiết bị.
V1 chưa nghiệm thu iPhone và chưa decode đủ BinNet. CBC/tar hợp lệ không phải bằng chứng xác thực mật mã của file backup.

## Nhật ký lần kiểm chứng tiếp theo

| Ngày / commit hoặc định danh working tree | Yêu cầu / task | Lệnh hoặc kịch bản | Kết quả và phạm vi | Giới hạn / thiết bị |
| :--- | :--- | :--- | :--- | :--- |

Chỉ thêm kết quả đã chạy; ghi test failed/ignored. Không điền bảng bằng dự đoán.
Không ghi khóa, cookie, URL có chữ ký hay nội dung tin của người khác.
