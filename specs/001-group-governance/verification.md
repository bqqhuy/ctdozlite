# Bằng chứng: Quyền và quản trị nhóm

Ngày ghi nhận: 2026-11-05 (Asia/Saigon). Đây là baseline từ phiên trước, không phải
kết quả chạy lại cho thay đổi tài liệu/Spec Kit. [Bản ghi phiên](../../docs/releases/v0.007-arm-work.md)
báo 565 passed, 10 ignored; Clippy/fmt/diff check đạt. Log gốc ở workspace,
không được coi là artifact phát hành; cần chạy lại ở commit chuẩn bị tag.

## Mã và fixture đã nhận diện

`src/group_state.rs, src/groups.rs, src/group_content.rs, src/gui/group_panel.rs, src/gui/content_panel.rs`

- `missing_and_removed_group_never_grant_send_permission`
- `manager_can_send_locked_group_but_roster_order_does_not_grant_role`
- `destructive_paths_and_fields_are_separate_from_delete_conversation`
- `anonymous_poll_does_not_invent_voter_identities`

## Kết luận theo bằng chứng

Có mã và fixture cho một phần yêu cầu. Chưa xác nhận toàn tính năng trên thiết bị.
Chưa xác nhận control tức thời và đầy đủ mô hình quyền cộng đồng; không coi lockSendMsg là toàn bộ mô hình quyền.

## Nhật ký lần kiểm chứng tiếp theo

| Ngày / commit hoặc định danh working tree | Yêu cầu / task | Lệnh hoặc kịch bản | Kết quả và phạm vi | Giới hạn / thiết bị |
| :--- | :--- | :--- | :--- | :--- |

Chỉ thêm kết quả đã chạy; ghi test failed/ignored. Không điền bảng bằng dự đoán.
Không ghi khóa, cookie, URL có chữ ký hay nội dung tin của người khác.
