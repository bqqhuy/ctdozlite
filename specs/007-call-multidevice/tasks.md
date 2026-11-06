# Tasks — P0 cuộc gọi đa thiết bị

- [x] [C-T001] [C-01…C-07] Ghi báo cáo và baseline helper/GUI; chỉ hoàn tất assessment ban đầu.
- [ ] [C-T002] [C-01,C-05,C-07] Tái hiện và xác minh control trên hai thiết bị; lưu fixture đã làm sạch — re-notes/, src/call.rs; trạng thái: chưa đủ bằng chứng nguyên nhân.
- [x] [C-T003] [C-03,C-04,C-05] Normalize ID/actor/envelope, transition và replay/epoch guard — src/call.rs, src/call_lifecycle.rs; trạng thái: đã có hardening và test mô hình; còn xác minh envelope thật.
- [x] [C-T004] [C-01,C-02,C-04,C-06] Biên GUI có identity, stop ring/popup/local resources, không end remote call — src/gui.rs, src/call.rs, src/gui/modals.rs; trạng thái: một phần đã có mã/test; chưa có xác nhận thiết bị.
- [x] [C-T005] [C-01…C-07] Test caller/callee/group, event lệch thứ tự, duplicate, account switch và GUI bridge — src/call_lifecycle.rs, tests/; trạng thái: một phần đã có mã/test; chưa có xác nhận thiết bị.
- [ ] [C-T090] [C-01…C-07] Nghiệm thu Windows ARM64 + thiết bị khác; xác nhận cuộc gọi remote vẫn tiếp tục — verification.md; trạng thái: chưa xác nhận trên thiết bị.

06/11: C-T003–005 xác nhận mã/model regression, gồm reject owner/call ID và terminal-before-invite; không đóng capture C-T002 hoặc nghiệm thu C-T090.
