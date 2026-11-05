# Bằng chứng: Hiển thị, flicker và điều hướng hội thoại

Ngày ghi nhận: 2026-11-05 (Asia/Saigon). Đây là baseline từ phiên trước, không phải
kết quả chạy lại cho thay đổi tài liệu/Spec Kit. [Bản ghi phiên](../../docs/releases/v0.007-arm-work.md)
báo 565 passed, 10 ignored; Clippy/fmt/diff check đạt. Log gốc ở workspace,
không được coi là artifact phát hành; cần chạy lại ở commit chuẩn bị tag.

## Mã và fixture đã nhận diện

`src/gui/theme.rs, src/gui/workspace.rs, src/gui.rs, src/group_metadata.rs, src/history_open.rs`

- `light_custom_white_foregrounds_remain_readable`
- `truncation_preserves_combining_accents_and_joined_emoji`
- `head_cache_survives_reopens_but_is_not_permanent_coverage`
- `zero_or_malformed_server_versions_are_never_trusted`
- `tests/group_metadata.rs`

## Kết luận theo bằng chứng

Có mã và fixture cho một phần yêu cầu. Chưa xác nhận toàn tính năng trên thiết bị.
Test tương phản và grapheme không thay kiểm chứng glyph thực tế hay quan sát flicker trên GPU Windows ARM64.

## Nhật ký lần kiểm chứng tiếp theo

| Ngày / commit hoặc định danh working tree | Yêu cầu / task | Lệnh hoặc kịch bản | Kết quả và phạm vi | Giới hạn / thiết bị |
| :--- | :--- | :--- | :--- | :--- |

Chỉ thêm kết quả đã chạy; ghi test failed/ignored. Không điền bảng bằng dự đoán.
Không ghi khóa, cookie, URL có chữ ký hay nội dung tin của người khác.
